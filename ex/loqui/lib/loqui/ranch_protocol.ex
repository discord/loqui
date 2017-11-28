defmodule Loqui.RanchProtocol do
  @moduledoc false

  use Loqui.Opcodes
  alias Loqui.{Protocol, Protocol.Frames}
  alias Loqui.Protocol.{Codecs, Compressors}
  require Logger

  @type state :: %__MODULE__{}

  @default_ping_interval 30_000
  @max_sequence round(:math.pow(2, 32) - 1)
  @supported_versions [1]
  @empty_flags 0
  @flag_compressed 1
  @default_compressors Enum.into(Compressors.all(), %{}, &{&1.name(), &1})
  @default_codecs Enum.into(Codecs.all(), %{}, &{&1.name(), &1})


  defstruct socket_pid: nil,
            transport: nil,
            handler: nil,
            handler_opts: nil,
            ping_interval: nil,
            supported_encodings: nil,
            supported_compressions: nil,
            version: nil,
            codec: nil,
            compressor: nil,
            monitor_refs: %{},
            pong_received: true,
            next_seq: 1

  def upgrade(socket_pid, transport, handler, handler_opts) do
    state = %__MODULE__{
      socket_pid: socket_pid,
      transport: transport,
      handler: handler,
      handler_opts: handler_opts,
    }

    Logger.info "[loqui] upgrade. address=#{inspect address(state)} socket_pid=#{inspect socket_pid}"

    handler_init(state)
  end

  @spec handler_init(state) :: :ok
  def handler_init(%__MODULE__{transport: transport, handler: handler, handler_opts: handler_opts}=state) do
    case handler.loqui_init(transport, handler_opts) do
      {:ok, opts} ->
        state
          |> set_opts(opts)
          |> loqui_handshake

      :shutdown ->
        :ok
    end
  end

  @spec loqui_handshake(state) :: :ok
  def loqui_handshake(%__MODULE__{}=state) do
    state
      |> schedule_ping()
      |> handler_loop(<<>>)
  end

  @spec handler_loop(state, binary) :: :ok
  def handler_loop(%{socket_pid: socket_pid, transport: transport}=state, so_far) do
    transport.setopts(socket_pid, [active: :once])

    receive do
      :send_ping ->
        state
          |> ping
          |> handler_loop(so_far)

      {:response, seq, response} ->
        state
          |> handle_response(seq, response, [])
          |> handler_loop(so_far)

      {:tcp, ^socket_pid, data} ->
        handle_socket_data(state, <<so_far::binary, data::binary>>)

      {:tcp_closed, ^socket_pid} ->
        Logger.info "[loqui] tcp_closed. socket_pid=#{inspect socket_pid}"
        close(state, :tcp_closed)

      {:tcp_error, ^socket_pid, reason} ->
        goaway(state, reason)

      {:DOWN, ref, :process, _pid, reason} ->
        state
          |> handle_down(ref, reason)
          |> handler_loop(so_far)

      other ->
        Logger.info "[loqui] unknown message. message=#{inspect other}"
        handler_loop(state, so_far)
    end
  end

  @spec ping(state) :: state | :ok
  defp ping(%{pong_received: false}=state),
    do: goaway(state, :ping_timeout)
  defp ping(state) do
    {seq, state} = next_seq(state)

    state
      |> schedule_ping()
      |> do_send(Frames.ping(0, seq))
      |> Map.put(:pong_received, false)
  end

  defp schedule_ping(%{ping_interval: ping_interval}=state) do
    Process.send_after(self(), :send_ping, ping_interval)
    state
  end

  @spec next_seq(state) :: {integer, state}
  defp next_seq(%{next_seq: next_seq}=state) when next_seq >= @max_sequence do
    {1, %{state | next_seq: 2}}
  end
  defp next_seq(%{next_seq: next_seq}=state) do
    {next_seq, %{state | next_seq: next_seq + 1}}
  end

  @spec handle_response(state, integer, any, []) :: state
  defp handle_response(%{compressor: compressor}=state, seq, response, responses) do
    compression_flag =
      case compressor do
        Protocol.Compressors.NoOp ->
          @empty_flags

        _ ->
          @flag_compressed
      end

    response_frame =
      case response do
        {:go_away, code, reason} ->
          Frames.goaway(compression_flag, code, reason)

        rsp ->
          payload = to_wire_format(state, rsp)

          Frames.response(compression_flag, seq, payload)
      end
    flush_responses(state, [response_frame | responses])
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

  defp handle_socket_data(state, data) do
    case Protocol.Parser.parse(data) do
      {:ok, requests, extra} ->
        case handle_requests(requests, state) do
          {:ok, state} ->
            handler_loop(state, extra)

          {:shutdown, reason} ->
            close(state, reason)
        end
      {:error, {:need_more_data, extra}} ->
        handler_loop(state, extra)

      {:error, reason} ->
        goaway(state, reason)
    end
  end

  defp handle_requests([request | rest], state)do
    case handle_request(request, state) do
      {:ok, state} ->
        handle_requests(rest, state)

      {:shutdown, _reason} = err ->
        err
    end
  end
  defp handle_requests([], state) do
    {:ok, state}
  end

  @spec handle_request(tuple, state) :: {:ok, state} | {:shutdown, atom}
  defp handle_request({:hello, _flags, version, encodings, compressions}, %{ping_interval: ping_interval, supported_encodings: supported_encodings, supported_compressions: supported_compressions}=state) do
    codec = choose_codec(supported_encodings, encodings)
    compressor = choose_compressor(supported_compressions, compressions)
    cond do
      !Enum.member?(@supported_versions, version) ->
        goaway(state, :unsupported_version)
        {:shutdown, :unsupported_version}

      is_nil(codec) ->
        goaway(state, :no_common_encoding)
        {:shutdown, :no_common_encoding}

      true ->
        settings_payload = "#{codec.name()}|#{compressor.name()}"
        do_send(state, Frames.hello_ack(@empty_flags, ping_interval, settings_payload))
        {:ok, %{state | version: version, codec: codec, compressor: compressor}}
    end
  end
  defp handle_request({:ping, _flags, seq}, state) do
    do_send(state, Frames.pong(@empty_flags, seq))
    {:ok, state}
  end
  defp handle_request({:pong, _flags, _seq}, state) do
    {:ok, %{state | pong_received: true}}
  end
  defp handle_request({:request, _flags, seq, request}, state) do
    decoded_request = from_wire_format(state, request)

    {:ok, handler_request(state, seq, decoded_request)}
  end
  defp handle_request({:push, _flags, request}, state) do
    decoded_push = from_wire_format(state, request)

    {:ok, handler_push(state, decoded_push)}
  end
  defp handle_request(request, state) do
    Logger.info "unknown request. request=#{inspect request}"
    {:ok, state}
  end

  @spec handle_down(state, reference, {atom, any} | atom) :: state
  defp handle_down(state, ref, {reason, _trace}), do: handle_down(state, ref, reason)
  defp handle_down(%{monitor_refs: monitor_refs}=state, ref, :normal) do
    %{state | monitor_refs: Map.delete(monitor_refs, ref)}
  end
  defp handle_down(%{monitor_refs: monitor_refs}=state, ref, reason) do
    case Map.pop(monitor_refs, ref) do
      {nil, monitor_refs} -> %{state | monitor_refs: monitor_refs}
      {seq, monitor_refs} ->
        state = send_error(state, seq, :internal_server_error, reason)
        %{state | monitor_refs: monitor_refs}
    end
  end

  @spec send_error(state, integer, integer, atom) :: %__MODULE__{}
  defp send_error(state, seq, :internal_server_error, reason), do: send_error(state, seq, 7, reason)
  defp send_error(state, seq, code, reason) do
    reason = to_wire_format(state, reason)
    do_send(state, Frames.error(@empty_flags, code, seq, reason))
  end

  @spec goaway(state, atom) :: :ok
  def goaway(state, :normal),
    do: goaway(state, 0, "Normal")
  def goaway(state, :invalid_op),
    do: goaway(state, 1, "InvalidOp")
  def goaway(state, :unsupported_version),
    do: goaway(state, 2, "UnsupportedVersion")
  def goaway(state, :no_common_encoding),
    do: goaway(state, 3, "NoCommonEncoding")
  def goaway(state, :invalid_encoding),
    do: goaway(state, 4, "InvalidEncoding")
  def goaway(state, :invalid_compression),
    do: goaway(state, 5, "InvalidCompression")
  def goaway(state, :ping_timeout),
    do: goaway(state, 6, "PingTimeout")
  def goaway(state, :internal_server_error),
    do: goaway(state, 7, "InternalServerError")
  def goaway(state, :not_enough_options),
    do: goaway(state, 8, "NotEnoughOptions")

  @spec goaway(state, integer, atom) :: :ok
  def goaway(%{socket_pid: socket_pid}=state, code, reason) do
    Logger.info "[loqui] goaway. address=#{inspect address(state)} socket_pid=#{inspect socket_pid} code=#{inspect code} reason=#{inspect reason}"
    do_send(state, Frames.goaway(@empty_flags, code, reason))
    close(state, reason)
  end

  @spec do_send(state, binary | [binary]) :: state
  defp do_send(%{transport: transport, socket_pid: socket_pid}=state, msg) do
    transport.send(socket_pid, msg)
    state
  end

  @spec handler_request(state, integer, binary) :: :ok
  defp handler_request(%{handler: handler, codec: codec, monitor_refs: monitor_refs}=state, seq, request) do
    from = self()
    {_, ref} = spawn_monitor(fn ->
      response = handler.loqui_request(request, codec)
      send(from, {:response, seq, response})
    end)
    %{state | monitor_refs: Map.put(monitor_refs, ref, seq)}
  end

  @spec handler_push(state, binary) :: :ok
  defp handler_push(%{handler: handler, codec: codec}=state, request) do
    spawn(handler, :loqui_request, [request, codec])
    state
  end

  @spec handler_terminate(state, atom) :: :ok
  defp handler_terminate(%{handler: handler}=state, reason) do
    handler.loqui_terminate(reason)
    state
  end

  @spec close(state, atom) :: :ok
  defp close(%{transport: transport, socket_pid: socket_pid}=state, reason) do
    handler_terminate(state, reason)
    transport.close(socket_pid)
    :ok
  end

  @spec set_opts(state, Map.t) :: state
  defp set_opts(state, %{supported_encodings: supported_encodings, supported_compressions: supported_compressions}=opts) do
    select_enabled = fn(supported, defaults) ->
      Enum.map(supported, fn
        name when is_bitstring(name) ->
          Map.get(defaults, name)

        module when is_atom(module) ->
          module
      end)
      |> Enum.reject(&is_nil/1)
      |> Enum.into(%{}, &{&1.name(), &1})
    end

    enabled_encodings = select_enabled.(supported_encodings, @default_codecs)

    enabled_compressors = select_enabled.(supported_compressions, @default_compressors)
      |> Map.put(Protocol.Compressors.NoOp.name(), Protocol.Compressors.NoOp)

    ping_interval = Map.get(opts, :ping_interval, @default_ping_interval)

    %{state |
      ping_interval: ping_interval,
      supported_encodings: enabled_encodings,
      supported_compressions: enabled_compressors,
    }

  end

  defp to_wire_format(%{codec: encoder, compressor: compressor}, data) do
    data
      |> encoder.encode()
      |> compressor.compress()
  end

  defp from_wire_format(%{codec: encoder, compressor: compressor}, data) do
    data
      |> compressor.decompress()
      |> encoder.decode()
  end

  @spec choose_codec(list, list) :: nil | String.t
  defp choose_codec(_supported_encodings, []), do: nil
  defp choose_codec(supported_encodings, [encoding | encodings]) do
    case Map.get(supported_encodings, encoding) do
      nil ->
        choose_codec(supported_encodings, encodings)

      codec ->
        codec
    end
  end

  @spec choose_compressor(list, list) :: nil | String.t
  defp choose_compressor(_supported_compressions, []), do: nil
  defp choose_compressor(supported_compressions, [compression | compressions]) do
    case Map.get(supported_compressions, compression) do
      nil ->
        choose_compressor(supported_compressions, compressions)

      compressor ->
        compressor
    end
  end

  @spec address(map) :: String.t
  defp address(state) do
    {:ok, {address, _port}} = state.transport.peername(state.socket_pid)
    :inet_parse.ntoa(address)
  end
end
