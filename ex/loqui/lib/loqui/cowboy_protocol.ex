defmodule Loqui.CowboyProtocol do
  use Loqui.Opcodes
  alias Loqui.{Protocol, Frames, Worker}
  require Logger

  @type req :: Map.t
  @type env :: Keyword.t
  @type state :: %__MODULE__{}

  @default_ping_interval 30_000
  @supported_versions [1]
  @empty_flags 0
  @flag_compressed 1

  defstruct socket_pid: nil,
            transport: nil,
            env: nil,
            req: nil,
            handler: nil,
            handler_opts: nil,
            ping_interval: nil,
            supported_encodings: nil,
            supported_compressions: nil,
            worker_pool: nil,
            version: nil,
            encoding: nil,
            compression: nil,
            monitor_refs: %{},
            pong_received: true,
            next_seq: 1

  def upgrade(req, env, handler, handler_opts) do
    :ranch.remove_connection(env[:listener])
    [socket_pid, transport] = :cowboy_req.get([:socket, :transport], req)

    state = %__MODULE__{
      socket_pid: socket_pid,
      transport: transport,
      req: req,
      env: env,
      handler: handler,
      handler_opts: handler_opts,
      worker_pool: Loqui.pool_name,
    }

    {host, _} = :cowboy_req.host(req)
    Logger.info "[loqui] upgrade. host=#{inspect host} socket_pid=#{inspect socket_pid}"

    handler_init(state)
  end

  @spec handler_init(state) :: {:ok, req, env}
  def handler_init(%__MODULE__{transport: transport, req: req, handler: handler, handler_opts: handler_opts, env: env}=state) do
    case handler.loqui_init(transport, req, handler_opts) do
      {:ok, req, opts} -> %{state | req: req} |> set_opts(opts) |> loqui_handshake
      {:shutdown, req} -> {:ok, req, Keyword.put(env, :result, :closed)}
    end
  end

  @spec loqui_handshake(state) :: :ok
  def loqui_handshake(%{req: req}=state) do
    :cowboy_req.upgrade_reply(101, [{"Upgrade", "loqui"}], req)
    receive do
      {:cowboy_req, :resp_sent} -> :ok
    after
      0 -> :ok
    end

    ping(state) |> handler_loop(<<>>)
  end

  @spec handler_loop(state, binary) :: {:ok, req, env}
  def handler_loop(%{socket_pid: socket_pid, transport: transport}=state, so_far) do
    transport.setopts(socket_pid, [active: :once])
    receive do
      :send_ping ->
        ping(state) |> handler_loop(so_far)
      {:response, seq, response} ->
        handle_response(state, seq, response, []) |> handle_socket_data(so_far)
      {:tcp, ^socket_pid, data} ->
        handle_socket_data(state, <<so_far :: binary, data :: binary>>)
      {:tcp_closed, ^socket_pid} ->
        Logger.info "[loqui] tcp_closed. socket_pid=#{inspect socket_pid}"
        close(state, :tcp_closed)
      {:tcp_error, ^socket_pid, reason} ->
        goaway(state, reason)
      {:DOWN, ref, :process, _pid, reason} ->
        handle_down(state, ref, reason) |> handler_loop(so_far)
      other ->
        Logger.info "[loqui] unknown message. message=#{inspect other}"
        handler_loop(state, so_far)
    end
  end

  @spec ping(state) :: state | {:ok, req, env}
  defp ping(%{pong_recieved: false}=state), do: goaway(state, :ping_timeout)
  defp ping(%{ping_interval: ping_interval}=state) do
    {seq, state} = next_seq(state)
    do_send(state, Frames.ping(0, seq))
    Process.send_after(self, :send_ping, ping_interval)
    %{state | pong_received: false}
  end

  @spec next_seq(state) :: {integer, state}
  defp next_seq(%{next_seq: next_seq}=state) do
    {next_seq, %{state | next_seq: next_seq + 1}}
  end

  @spec handle_response(state, integer, any, []) :: state
  defp handle_response(%{monitor_refs: monitor_refs}=state, seq, response, responses) do
    %{state | monitor_refs: Map.delete(monitor_refs, seq)}
      |> flush_responses([response_frame(response, seq) | responses])
  end

  @spec flush_responses(state, [binary]) :: state
  defp flush_responses(state, responses) do
    receive do
      {:response, seq, response} ->
        handle_response(state, seq, response, responses)
    after
      0 ->
        do_send(state, responses)
    end
  end

  @spec response_frame({atom, binary} | binary, integer) :: binary
  defp response_frame({:compressed, payload}, seq), do: Frames.response(@flag_compressed, seq, payload)
  defp response_frame(payload, seq), do: Frames.response(@empty_flags, seq, payload)

  @spec handle_socket_data(state, binary) :: {:ok, req, env}
  defp handle_socket_data(state, data) do
    case Protocol.handle_data(data) do
      {:ok, request, extra} ->
        case handle_request(request, state) do
          {:ok, state} -> handle_socket_data(state, extra)
          {:shutdown, reason} -> close(state, reason)
        end
      {:continue, extra} -> handler_loop(state, extra)
      {:error, reason} -> goaway(state, reason)
    end
  end

  @spec handle_request(tuple, state) :: {:ok, state} | {:shutdown, atom}
  defp handle_request({:hello, _flags, version, encodings, compressions}, %{ping_interval: ping_interval, supported_encodings: supported_encodings, supported_compressions: supported_compressions}=state) do
    encoding = choose_encoding(supported_encodings, encodings)
    compression = choose_compression(supported_compressions, compressions)

    cond do
      !Enum.member?(@supported_versions, version) ->
        goaway(state, :unsupported_version)
        {:shutdown, :unsupported_version}
      is_nil(encoding) ->
        goaway(state, :no_common_encoding)
        {:shutdown, :no_common_encoding}
      true ->
        settings_payload = "#{encoding}|#{compression}"
        do_send(state, Frames.hello_ack(@empty_flags, ping_interval, settings_payload))
        {:ok, %{state | version: version, encoding: encoding, compression: compression}}
    end
  end
  defp handle_request({:ping, _flags, seq}, state) do
    do_send(state, Frames.pong(@empty_flags, seq))
    {:ok, state}
  end
  defp handle_request({:pong, _flags, seq}, state) do
    {:ok, %{state | pong_received: true}}
  end
  defp handle_request({:request, _flags, seq, request}, state) do
    {:ok, handler_request(state, seq, request)}
  end
  defp handle_request({:push, _flags, request}, state) do
    {:ok, handler_push(state, request)}
  end
  defp handle_request(request, state) do
    Logger.info "unknown request. request=#{inspect request}"
    {:ok, state}
  end

  @spec handle_down(state, reference, {atom, any} | atom) :: state
  defp handle_down(state, ref, {reason, _trace}), do: handle_down(state, ref, reason)
  defp handle_down(%{monitor_refs: monitor_refs}=state, ref, reason) do
    case Enum.find(monitor_refs, &match?({_, ^ref}, &1)) do
      {seq, ^ref} ->
        send_error(state, seq, :internal_server_error, reason)
        %{state | monitor_refs: Map.delete(monitor_refs, seq)}
      nil -> state
    end
  end

  @spec send_error(state, integer, integer, atom) ::  {:ok, req, env}
  defp send_error(state, seq, :internal_server_error, reason), do: send_error(state, seq, 7, reason)
  defp send_error(state, seq, code, reason) do
    reason = encode(state, reason)
    do_send(state, Frames.error(@empty_flags, code, seq, reason))
  end

  @spec goaway(state, atom) :: {:ok, req, env}
  def goaway(state, :normal), do: goaway(state, 0, "Normal")
  def goaway(state, :invalid_op), do: goaway(state, 1, "InvalidOp")
  def goaway(state, :unsupported_version), do: goaway(state, 2, "UnsupportedVersion")
  def goaway(state, :no_common_encoding), do: goaway(state, 3, "NoCommonEncoding")
  def goaway(state, :invalid_encoding), do: goaway(state, 4, "InvalidEncoding")
  def goaway(state, :invalid_compression), do: goaway(state, 5, "InvalidCompression")
  def goaway(state, :ping_timeout), do: goaway(state, 6, "PingTimeout")
  def goaway(state, :internal_server_error), do: goaway(state, 7, "InternalServerError")
  def goaway(state, :not_enough_options), do: goaway(state, 8, "NotEnoughOptions")

  @spec goaway(state, integer, atom) :: {:ok, req, env}
  def goaway(%{socket_pid: socket_pid}=state, code, reason) do
    Logger.info "[loqui] goaway. socket_pid=#{inspect socket_pid} code=#{inspect code} reason=#{inspect reason}"
    do_send(state, Frames.goaway(@empty_flags, code, reason))
    close(state, reason)
  end

  @spec do_send(state, binary | [binary]) :: state
  defp do_send(%{transport: transport, socket_pid: socket_pid}=state, msg) do
    transport.send(socket_pid, msg)
    state
  end

  @spec handler_request(state, integer, binary) :: :ok
  defp handler_request(%{handler: handler, worker_pool: worker_pool, encoding: encoding, monitor_refs: monitor_refs}=state, seq, request) do
    ref = :poolboy.transaction(worker_pool, fn pid ->
      ref = Process.monitor(pid)
      Worker.request(pid, {handler, :loqui_request, [request, encoding]}, seq, self)
      ref
    end)
    monitor_refs = Map.put(monitor_refs, seq, ref)
    %{state | monitor_refs: monitor_refs}
  end

  @spec handler_push(state, binary) :: :ok
  defp handler_push(%{handler: handler, worker_pool: worker_pool, encoding: encoding}=state, request) do
    :poolboy.transaction(worker_pool, &Worker.push(&1, {handler, :loqui_request, [request, encoding]}))
    state
  end

  @spec handler_terminate(state, atom) :: :ok
  defp handler_terminate(%{handler: handler, req: req}=state, reason) do
    handler.loqui_terminate(reason, req)
    state
  end

  @spec close(state, atom) :: {:ok, req, env}
  defp close(%{transport: transport, socket_pid: socket_pid, env: env, req: req}=state, reason) do
    handler_terminate(state, reason)
    transport.close(socket_pid)
    {:ok, req, Keyword.put(env, :result, :closed)}
  end

  @spec set_opts(state, Map.t) :: state
  defp set_opts(state, opts) do
    %{supported_encodings: supported_encodings, supported_compressions: supported_compressions} = opts
    ping_interval = Map.get(opts, :ping_interval, @default_ping_interval)
    %{state |
      ping_interval: ping_interval,
      supported_encodings: supported_encodings,
      supported_compressions: supported_compressions,
    }
  end

  @spec encode(state, any) :: binary
  def encode(%{encoding: "erlpack"}, msg), do: :erlang.term_to_binary(msg)
  @spec decode(state, binary) :: any
  def decode(%{encoding: "erlpack"}, msg), do: :erlang.binary_to_term(msg)

  @spec choose_encoding(list, list) :: nil | String.t
  defp choose_encoding(_supported_encodings, []), do: nil
  defp choose_encoding(supported_encodings, [encoding | encodings]) do
    if Enum.member?(supported_encodings, encoding), do: encoding, else: choose_encoding(supported_encodings, encodings)
  end

  @spec choose_compression(list, list) :: nil | String.t
  defp choose_compression(_supported_compressions, []), do: nil
  defp choose_compression(supported_compressions, [compression | compressions]) do
    if Enum.member?(supported_compressions, compression), do: compression, else: choose_compression(supported_compressions, compressions)
  end
end
