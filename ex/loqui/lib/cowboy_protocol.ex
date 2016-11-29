defmodule Loqui.CowboyProtocol do
  use Loqui.Opcodes
  alias Loqui.{Parser, Messages}
  require Logger

  @supported_versions [1]
  @supported_encodings MapSet.new(["erlpack"])
  @supported_compressions MapSet.new()


  defstruct socket_pid: nil,
            transport: nil,
            env: nil,
            req: nil,
            handler: nil,
            handler_opts: nil,
            handler_state: nil,
            ping_interval: nil,
            version: nil,
            encoding: nil,
            timeout: :infinity,
            timeout_ref: nil,
            hibernate: false

  def upgrade(req, env, handler, handler_opts) do
    {_, ref} = :lists.keyfind(:listener, 1, env)
    :ranch.remove_connection(ref)
    [socket_pid, transport] = :cowboy_req.get([:socket, :transport], req)

    state = %__MODULE__{
      socket_pid: socket_pid,
      transport: transport,
      req: req,
      env: env,
      handler: handler,
      handler_opts: handler_opts,
    }

    handler_init(state)
  end

  def handler_init(%{transport: transport, req: req, handler: handler, handler_opts: handler_opts, env: env}=state) do
    case handler.loqui_init(transport, req, handler_opts) do
      {:ok, req, ping_interval, handler_state} ->
        state = %{state | req: req, handler_state: handler_state, ping_interval: ping_interval}
        loqui_handshake(state)
      {:ok, req, ping_interval, handler_state, :hibernate} ->
        state = %{state | req: req, handler_state: handler_state, ping_interval: ping_interval, hibnerate: true}
        loqui_handshake(state)
      {:ok, req, ping_interval, handler_state, timeout} ->
        state = %{state | req: req, handler_state: handler_state, ping_interval: ping_interval, timeout: timeout}
        loqui_handshake(state)
      {:ok, req, ping_interval, handler_state, timeout, :hibernate} ->
        state = %{state | req: req, handler_state: handler_state, ping_interval: ping_interval, timeout: timeout, hibernate: true}
        loqui_handshake(state)
      {:shutdown, req} ->
        {:ok, req, Keyword.put(env, :result, :closed)}
    end
  end

  def loqui_handshake(%{req: req}=state) do
    :cowboy_req.upgrade_reply(101, [{"Upgrade", "loqui"}], req)
    receive do
      {:cowboy_req, :resp_sent} -> :ok
    after
      0 -> :ok
    end

    handler_loop_timeout(state) |> handler_before_loop(<<>>)
  end

  def handler_before_loop(%{socket_pid: socket_pid, transport: transport, hibnerate: true}=state, so_far) do
    transport.setopts(socket_pid, [active: :once])
    {:suspend, __MODULE__, :handler_loop, [%{state | hibernate: false}, so_far]}
  end
  def handler_before_loop(%{socket_pid: socket_pid, transport: transport}=state, so_far) do
    transport.setopts(socket_pid, [active: :once])
    handler_loop(state, so_far)
  end

  def handler_loop(%{socket_pid: socket_pid, timeout_ref: timeout_ref}=state, so_far) do
    receive do
      {:tcp, ^socket_pid, data} ->
        state = handler_loop_timeout(state)
        socket_data(state, <<so_far :: binary, data :: binary>>)
      {:tcp_closed, ^socket_pid} -> close(state, :tcp_closed)
      {:tcp_error, ^socket_pid, reason} -> goaway(state, reason)
      :timeout ->
        if Process.read_timer(timeout_ref) == false do
          goaway(state, :timeout)
        else
          handler_loop(state, so_far)
        end
      other ->
        Logger.info "[loqui] unknown message. message=#{inspect other}"
        handler_loop(state, so_far)
    end
  end

  defp socket_data(state, data) do
    case Parser.handle_data(data) do
      {:ok, request, extra} ->
        case handle_request(request, state) do
          {:ok, state} -> socket_data(state, extra)
          {:shutdown, reason} -> close(state, reason)
        end
      {:continue, extra} -> handler_before_loop(state, extra)
    end
  end

  defp handle_request({:hello, _flags, version, encodings, _compressions}, %{ping_interval: ping_interval}=state) do
    common_encodings = MapSet.intersection(encodings, @supported_encodings) |> MapSet.to_list()
    cond do
      !Enum.member?(@supported_versions, version) ->
        goaway(state, :unsupported_version)
        {:shutdown, :unsupported_version}
      common_encodings == [] ->
        goaway(state, :no_common_encoding)
        {:shutdown, :no_common_encoding}
      true ->
        flags = 0
        encoding = List.first(common_encodings)
        settings_payload = "#{encoding}|"
        do_send(state, Messages.make_hello_ack(flags, ping_interval, settings_payload))
        {:ok, %{state | version: version, encoding: encoding}}
    end
  end
  defp handle_request({:ping, _flags, seq}, state) do
    do_send(state, Messages.make_pong(0, seq))
    {:ok, state}
  end
  defp handle_request({:request, _flags, seq, request}, state) do
    request = decode(state, request)
    {response, handler_state} = handler_request(state, request)
    response = encode(state, response)
    do_send(state, Messages.make_response(0, seq, response))
    {:ok, %{state | handler_state: handler_state}}
  end
  defp handle_request({:push, _flags, request}, state) do
    request = decode(state, request)
    handler_state = handler_push(state, request)
    {:ok, %{state | handler_state: handler_state}}
  end
  defp handle_request(request, state) do
    Logger.info "unknown request. request=#{inspect request}"
    {:ok, state}
  end

  defp send_error(state, seq, :handler_error, reason), do: send_error(state, seq, 1, reason)
  defp send_error(state, seq, code, reason) do
    reason = encode(state, reason)
    do_send(state, Messages.make_error(0, code, seq, reason))
  end

  def goaway(state, :unsupported_version), do: goaway(state, 2, "UnsupportedVersion")
  def goaway(state, :no_common_encoding), do: goaway(state, 3, "NoCommonEncoding")
  def goaway(state, :timeout), do: goaway(state, 4, "Timeout")

  def goaway(state, code, reason) do
    do_send(state, Messages.make_goaway(0, code, reason))
    close(state, reason)
  end

  defp do_send(%{transport: transport, socket_pid: socket_pid}=state, msg) do
    transport.send(socket_pid, msg)
    state
  end

  defp handler_request(%{handler: handler, handler_state: handler_state}, request) do
    handler.handle_request(request, handler_state)
  end
  defp handler_push(%{handler: handler, handler_state: handler_state}, request) do
    handler.handle_push(request, handler_state)
  end
  defp handler_terminate(%{handler: handler, req: req, handler_state: handler_state}, reason) do
    handler.loqui_terminate(reason, req, handler_state)
  end

  defp close(%{transport: transport, socket_pid: socket_pid, env: env, req: req}=state, reason) do
    handler_terminate(state, reason)
    transport.close(socket_pid)
    {:ok, req, Keyword.put(env, :result, :closed)}
  end

  def encode(%{encoding: "erlpack"}, msg), do: :erlang.term_to_binary(msg)
  def decode(%{encoding: "erlpack"}, msg), do: :erlang.binary_to_term(msg)

  defp handler_loop_timeout(%{timeout: :infinity}=state), do: %{state | timeout_ref: nil}
  defp handler_loop_timeout(%{timeout: timeout, timeout_ref: prev_ref}=state) do
    if prev_ref do
      Process.cancel_timer(prev_ref)
    end
    ref = Process.send_after(self, :timeout, timeout)
    %{state | timeout_ref: ref}
  end
end
