defmodule Loqui.Client do
  defmodule State do
    @max_sequence round(:math.pow(2, 32) - 1)

    alias Loqui.Protocol.{
      Codec,
      Codecs,
      Compressor,
      Compressors
    }

    @type timestamp :: pos_integer
    @type last_ping :: {Loqui.Client.sequence, timestamp}
    @type t :: %__MODULE__{
      port: pos_integer,
      host: char_list,
      tcp_opts: Loqui.Client.tcp_opts,
      recv_timeout: pos_integer,
      loqui_path: String.t,
      buffer: binary,
      packet_buffer: iodata,
      sequence: :go_away | Loqui.Client.sequence,
      ping_interval: pos_integer,
      registered_compressors: %{String.t => Compressor.t},
      registered_codecs: %{String.t => Codec.t},
      compressor: Compressor.t,
      codec: Codec.t,
      waiters: %{Loqui.Client.sequence => pid},
      last_ping: last_ping
    }

    defstruct host: nil,
      port: nil,
      sock: nil,
      tcp_opts: nil,
      connect_timeout: nil,
      recv_timeout: nil,
      loqui_path: nil,
      buffer: <<>>,
      packet_buffer: [],
      sequence: 1,
      ping_interval: nil,
      registered_compressors: %{},
      registered_codecs: %{},
      compressor: Compressors.NoOp,
      codec: Codecs.Erlpack,
      waiters: %{},
      last_ping: nil

    def add_packet(%State{packet_buffer: buffer}=state, packet) do
      %State{state | packet_buffer: [packet | buffer]}
    end

    def push_waiter(%State{waiters: waiters}=state, sequence, waiter) do
      %State{state | waiters: Map.put(waiters, sequence, waiter)}
    end

    def pop_waiter(%State{waiters: waiters}=state, sequence) do
      {waiter, new_waiters} = Map.pop(waiters, sequence)
      {%State{state | waiters: new_waiters}, waiter}
    end

    def next_sequence(%State{sequence: seq}=state) when seq > @max_sequence do
      {1, %State{state | sequence: 2}}
    end

    def next_sequence(%State{sequence: seq}=state) do
      {seq, %State{state | sequence: seq + 1}}
    end
  end

  alias Loqui.Protocol.{Codec, Codecs, Compressor, Compressors, Frames, Parser}
  use Connection
  use Loqui.{Opcodes, Types}
  require Logger

  @default_timeout 5000
  @max_sequence round(:math.pow(2, 32) - 1)
  @default_compressors %{Compressors.NoOp.name() => Compressors.NoOp}
  @default_codecs %{Codecs.Erlpack.name() => Codecs.Erlpack}
  @go_away_timeout 1000

  @type sequence :: 1..unquote(@max_sequence)

  @type tcp_opt :: {:recv_timeout, pos_integer} | {:send_timeout, pos_integer}
  @type tcp_opts :: [tcp_opt]

  @type loqui_opt :: {:codecs, [Codec.t]} | {:compressors, [Compressor.t]}
  @type loqui_opts :: [loqui_opt]

  @type opts :: [
    loqui_opts: loqui_opts,
    tcp_opts: tcp_opts
  ]


  def start_link(host, port, loqui_path, options) do
    Connection.start_link(__MODULE__, {host, port, loqui_path, options})
  end

  def init({host, port, loqui_path, opts}) do
    tcp_opts = opts
      |> Keyword.get(:tcp_opts, [])
      |> Keyword.put(:active, :false)
      |> Keyword.put(:mode, :binary)
      |> Keyword.put(:send_timeout_close, true)

    {loqui_opts, _opts} = Keyword.pop(opts, :loqui_opts, [])

    extract_option = fn(opt_name) ->
      loqui_opts
        |> Keyword.get(opt_name, [])
        |> Enum.into(%{}, &{&1.name, &1})
    end

    registered_codecs = Map.merge(@default_codecs, extract_option.(:codecs))
    registered_compressors = Map.merge(@default_compressors, extract_option.(:compressors))

    {connect_timeout, tcp_opts} = Keyword.pop(tcp_opts, :connect_timeout, @default_timeout)

    state = %State{
      host: to_host(host),
      port: port,
      tcp_opts: tcp_opts,
      loqui_path: loqui_path,
      connect_timeout: connect_timeout,
      sequence: 1,
      registered_compressors: registered_compressors,
      registered_codecs: registered_codecs,
      buffer: <<>>}

    {:connect, :init, state}
  end

  def close(conn, timeout \\ 5000),
    do: Connection.call(conn, :close, timeout)

  def ping(conn, timeout \\ 5000) do
    Connection.call(conn, :ping, timeout)
  end

  def request(conn, payload, timeout \\ 5000) do
    Connection.call(conn, {:request, payload}, timeout)
  end

  def push(conn, payload) do
    Connection.cast(conn, {:push, payload})
  end

  def connect(_info, %{sock: nil, host: host, port: port, tcp_opts: tcp_opts, connect_timeout: connect_timeout}=state) do
    case :gen_tcp.connect(host, port, tcp_opts, connect_timeout) do
      {:ok, sock} ->
        # this is out of the with statement because we need to bind
        # the socket to the state, and if the with statement doesn't
        # complete, the state won't have the socket in it.
        state = %{state | sock: sock}

        with {:ok, _sock}  <- update_socket_opts(sock),
             {:ok, state} <- do_upgrade(state),
             state        <- make_active_once(state),
             {:ok, state} <- do_loqui_connect(state) do

          {:ok, state}
        else
          {:error, _} = error ->
            Logger.error("Loqui upgrade failed because of #{inspect error}")
            {:stop, error, state}
        end

      {:error, _} = error ->
        Logger.error("Couldn't connect to #{host}:#{port} because #{inspect error}")
        {:stop, error, state}
    end
  end

  defp update_socket_opts(socket) do
    {:ok, opts} = :inet.getopts(socket, [:sndbuf, :recbuf])
    send_buffer_size = opts[:sndbuf]
    recieve_buffer_size = opts[:recbuf]
    buffer_size = max(send_buffer_size, recieve_buffer_size)
    :inet.setopts(socket, [sndbuf: send_buffer_size,
                           recbuf: recieve_buffer_size,
                           buffer: buffer_size])
    {:ok, socket}
  end

  def disconnect(info, %State{sock: sock}=state) do
    :ok = :gen_tcp.close(sock)
    case info do
      {:close, from} ->
        Connection.reply(from, :ok)
        {:stop, :normal, %{state | sock: nil}}

      {:error, reason}  ->
        {:stop, reason, %{state | sock: nil}}
    end
  end

  # Connection callbacks
  def handle_call(:ping, caller, %{sock: sock}=state) do
    {next_seq, state} = State.next_sequence(state)
    :gen_tcp.send(sock, Frames.ping(0, next_seq))

    {:noreply, State.push_waiter(state, next_seq, caller)}
  end

  def handle_call(_, _from, %State{sequence: :go_away}=state) do
    {:reply, {:error, :remote_went_away}, state}
  end

  def handle_call(:close, from, %State{sock: sock}=s) do
    go_away_packet = Frames.goaway(0, 0, "Closing")
    :gen_tcp.send(sock, go_away_packet)

    {:disconnect, {:close, from}, s}
  end

  def handle_call({:request, payload}, caller, %State{codec: codec}=state) do
    {next_seq, state} = State.next_sequence(state)
    encoded_payload = codec.encode(payload)

    state = state
      |> buffer_packet(Frames.request(0, next_seq, encoded_payload))
      |> State.push_waiter(next_seq, caller)

    {:noreply, state}
  end

  def handle_cast(_, %State{sequence: :go_away}=state),
    do: {:noreply, state}

  def handle_cast({:push, payload}, %State{codec: codec}=state) do
    encoded_payload = codec.encode(payload)

    push_frame = Frames.push(0, encoded_payload)

    {:noreply, buffer_packet(state, push_frame)}
  end

  def handle_info({:tcp, socket, data}, %State{sock: socket}=state) do
    make_active_once(state)
    with {:ok, parsed_packets, leftover_data} <- Parser.parse(state.buffer, data) do
      state = handle_packets(parsed_packets, state)

      {:noreply, %State{state | buffer: leftover_data}}
    else

      {:error, {:need_more_data, data}} ->
        {:noreply, %State{buffer: data}}
    end
  end

  def handle_info(:flush_packets, %State{packet_buffer: []}=state) do
    {:noreply, state}
  end

  def handle_info(:flush_packets, %State{packet_buffer: packets, sock: socket}=state) do
    :gen_tcp.send(socket, packets)

    {:noreply, %State{state | packet_buffer: []}}
  end

  def handle_info({:tcp_closed, _socket}, _state) do
    {:stop, :tcp_closed, nil}
  end

  def handle_info({:tcp_error, _, _}, _state) do
    {:stop, :tcp_closed, nil}
  end

  def handle_info({:close_go_away, go_away_code, go_away_data}, %State{sequence: :go_away, waiters: waiters}=state) do
    # tell the waiters that the remote end went away, then close.
    err = {:error, {:remote_went_away, go_away_code, go_away_data}}
    Enum.each(waiters, fn {_, waiter} ->
      Connection.reply(waiter, err)
    end)

    {:disconnect, err, state}
  end

  def handle_info(:send_ping, %State{sock: sock, last_ping: nil}=state) do
    {next_seq, state} = State.next_sequence(state)

    :gen_tcp.send(sock, Frames.ping(0, next_seq))
    new_state = state
      |> schedule_ping
      |> Map.put(:last_ping, {next_seq, :erlang.system_time(:milli_seconds)})

    {:noreply, new_state}
  end
  def handle_info(:send_ping, %State{last_ping: {sequence, _ts}}=state) do
    {:stop, {:error, {:pong_not_received, sequence}}, state}
  end

  # Private

  defp buffer_packet(%State{}=state, packet) do
    # for a client with messages in its mailbox, this will buffer
    # all subsequent requests and pushes until the :flush_packets message
    # is recieved. Then all the packets will be sent in one call to
    # gen_tcp.
    send(self(), :flush_packets)
    State.add_packet(state, packet)
  end

  defp handle_packets([], state),
    do: state
  defp handle_packets([packet | rest], state) do
    state = handle_packet(packet, state)
    handle_packets(rest, state)
  end

  defp handle_packet({:ping, _flags, seq}, %State{sock: sock}=state) do
    :gen_tcp.send(sock, Frames.pong(0, seq))
    state
  end

  defp handle_packet({:pong, _flags, seq}, %State{last_ping: {seq, _ts}}=state) do
    %State{state | last_ping: nil}
  end

  defp handle_packet({:pong, _flags, seq}, %State{last_ping: nil}=state) do
    {state, waiter} = State.pop_waiter(state, seq)

    case waiter do
      nil ->
        Logger.error("Got an unknown pong for sequence #{inspect seq}")

      reply_to ->
        Connection.reply(reply_to, :pong)
    end

    state
  end

  defp handle_packet({:hello_ack, _flags, ping_interval, data}, %State{}=state) do
    [encoding, compression] = String.split(data, "|")
    codec = Map.get(state.registered_codecs, encoding)
    compressor = Map.get(state.registered_compressors, compression)

    %State{state | codec: codec, compressor: compressor, ping_interval: ping_interval}
      |> schedule_ping()
  end

  defp handle_packet({:response, _flags, sequence, payload}, %State{codec: codec}=state) do
    {state, waiter} = State.pop_waiter(state, sequence)
    decoded_data = codec.decode(payload)
    Connection.reply(waiter, decoded_data)

    state
  end

  defp handle_packet({:go_away, _flags, close_code, payload_data}, %State{}=state) do
    Process.send_after(self(), {:close_go_away, close_code, payload_data}, @go_away_timeout)
    %State{state | sequence: :go_away}
  end

  defp handle_packet(packet, state) do
    Logger.error("Received unknown packet, opcode #{inspect elem(packet, 0)} #{inspect packet}")
    state
  end

  defp schedule_ping(%State{ping_interval: ping_interval}=state) do
    jitter = round(1000 * :rand.uniform())

    Process.send_after(self(), :send_ping, ping_interval - jitter)
    state
  end

  defp make_active_once(%State{sock: sock}=state) do
    :ok = :inet.setopts(sock, [active: :once])
    state
  end

  defp do_upgrade(%State{sock: sock, tcp_opts: tcp_opts}=state) do
    recv_timeout = Keyword.get(tcp_opts, :recv_timeout, @default_timeout)
    with :ok         <- :gen_tcp.send(sock, upgrade_request(state)),
         {:ok, data} <- :gen_tcp.recv(sock, 0, recv_timeout),
         :upgraded   <- parse_upgrade_response(data) do

      {:ok, state}
    else
      {:error, reason} ->
        Logger.error("Upgrade failed #{inspect reason}")
        {:error, {:upgrade_failed, reason}}
    end
  end

  defp to_host(host) when is_bitstring(host),
    do: String.to_charlist(host)

  defp to_host(host) when is_list(host),
    do: host

  defp upgrade_request(%State{loqui_path: loqui_path, host: host}) do
    "GET #{loqui_path} HTTP/1.1\r\nHost: #{host}\r\nUpgrade: loqui\r\nConnection: upgrade\r\n\r\n"
  end

  defp parse_upgrade_response(<<"HTTP/1.1 101 Switching Protocols", _rest :: binary>>) do
    :upgraded
  end

  defp parse_upgrade_response(invalid_response) do
    {:error, {:upgrade_failed, invalid_response}}
  end

  defp do_loqui_connect(%State{sock: sock, registered_codecs: codecs, registered_compressors: compressors}=state) do
    extract_names = fn(m) ->
      m
        |> Map.keys
        |> Enum.join(",")
    end

    codecs = extract_names.(codecs)
    compressors = extract_names.(compressors)
    :gen_tcp.send(sock, Frames.hello(0, "#{codecs}|#{compressors}"))

    {:ok, state}
  end
end
