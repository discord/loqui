defmodule Loqui.Client do
  @moduledoc """
  # A high performance client for the Loqui protocol

  This client exposes five public functions, `start_link`, `request`, `push`, `ping` and `close`.
  In loqui parlance, `request` is a synchronous call and `push` is asynchrounous. Be careful with push
  calls, as they can overwhelm the remote server.

  ## Codecs and Compressors

  A `codec` changes data structures into `iodata` and a `compressor` applies a compression algorithm
  to iodata. They're applied thusly:

  ```
  term
  |> Codec.encode()
  |> Compressor.compress()
  ```

  When connecting, a Loqui client and server negotiate which compressors and codecs they
  should use to communicate.

  The Loqui client comes with several preinstalled codecs and compressors. They are:

  ### Codecs

    * `Loqui.Protocol.Codecs.Erlpack`: Uses `:erlang.term_to_binary` to encode terms and `:erlang.binary_to_term` to
       decode. All terms can be represented in `Erlpack`.
    * `Loqui.Protocol.Codecs.Json`: Uses the `jiffy` JSON library to encode and decode terms. Not all terms are
       able to be represented in JSON.
    * `Loqui.Protocol.Codecs.Msgpack`: Uses `Msgpax` to encode data in MessagePack format. Not all terms can
       be represented in MessagePack.

  ### Compressors

   * `Loqui.Protocol.Compressors.NoOp`: The default compressor; does nothing
   * `Loqui.Protocol.Compressors.Gzip`: Compresses data via `:zlib.gzip` and decompresses data via `:zlib.gunzip`

  The dependencies for the JSON and Msgpack codecs are **not** automatically resolved. If you wish to
  use one of them, you need to update your app's `mix.exs`.

  ```
  # in your mix.exs
  {:jiffy, "~> 0.14.11"}, # adding this line downloads jiffy and enables the Json codec
  {:msgpax, "~> 2.0"}, # adding this line downloads msgpax and enables the Msgpack codec

  ```

  If Loqui can detect the presence of the modules, then the codecs will be enabled.

  ### Defining your own

  If you need to define your own codec or compressor, you'll need to implement a module and then
  tell the client about it. In order to define one, implement the behaviour:

  ```elixir
  defmodule Useless do
    @behaviour Loqui.Protocol.Codec

    def name(), do: "useless"
    def encode(term), do: "1"
    def decode(binary), do: 1
  end
  ```

  Then you need to tell the client to use your codec.

  ```elixir
  {:ok, client} = Loqui.Client.start_link("localhost", 1234, "/_rpc", loqui_opts: [codecs: [Useless]])
  ```

  During negotiation, codecs and compressors defined by the user take precedence over the defaults,
  so if the server supports the `Useless` codec, it will be the one selected. If not, the precedence
  is: `Erlpack`, `Msgpack`, `Json`.
  You can override this ordering by re-specifying the default compressors:

  ```
  alias Loqui.Protocol.Codecs.{Erlpack, Json, Msgpack}
  # Starts a Loqui client on port 4450 favoring the Json codec, then the Msgpack codec, then the Erlpack codec.
  {:ok, json_client} = Loqui.Client.start_link("localhost", 4450, "/_rpc", loqui_opts: [codecs: [Json, Msgpack, Erlpack]])
  ```

  """

  defmodule State do
    @moduledoc false
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
      host: charlist,
      transport: Loqui.Client.transport,
      transport_opts: Loqui.Client.transport_opts,
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
      transport: nil,
      transport_opts: nil,
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

    def add_waiter(%State{waiters: waiters}=state, sequence, waiter) do
      %State{state | waiters: Map.put(waiters, sequence, waiter)}
    end

    def remove_waiter(%State{waiters: waiters}=state, sequence) do
      {waiter, new_waiters} = Map.pop(waiters, sequence)
      {%State{state | waiters: new_waiters}, waiter}
    end

    def next_sequence(%State{sequence: seq}=state) when seq >= @max_sequence do
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
  @default_compressors Enum.into(Compressors.all(), %{}, &{&1.name, &1})
  @default_codecs Enum.into(Codecs.all(), %{}, &{&1.name(), &1})
  @go_away_timeout 1000

  @type sequence :: 1..unquote(@max_sequence)

  @type transport :: :gen_tcp | :ssl
  @type transport_opt :: {:recv_timeout, pos_integer} | {:send_timeout, pos_integer}
  @type transport_opts :: [transport_opt]

  @type loqui_opt :: {:codecs, [Codec.t]} | {:compressors, [Compressor.t]}
  @type loqui_opts :: [loqui_opt]

  @type opts :: [
    loqui_opts: loqui_opts,
    transport: transport,
    transport_opts: transport_opts
  ]

  @doc """
  Creates a connection to a Loqui server.
  """
  @spec start_link(String.t, pos_integer, String.t, opts) :: {:ok, pid} | {:error, term}
  def start_link(host, port, loqui_path, options) do
    Connection.start_link(__MODULE__, {host, port, loqui_path, options})
  end

  @doc false
  def init({host, port, loqui_path, opts}) do
    transport_opts = opts
      |> Keyword.get(:transport_opts, [])
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

    {transport, _opts} = Keyword.pop(opts, :transport, :gen_tcp)
    {connect_timeout, transport_opts} = Keyword.pop(transport_opts, :connect_timeout, @default_timeout)

    state = %State{
      host: to_host(host),
      port: port,
      transport: transport,
      transport_opts: transport_opts,
      loqui_path: loqui_path,
      connect_timeout: connect_timeout,
      sequence: 1,
      registered_compressors: registered_compressors,
      registered_codecs: registered_codecs,
      buffer: <<>>}

    {:connect, :init, state}
  end


  @doc """
  Closes the connection.
  """
  @spec close(pid, pos_integer) :: :ok | {:error, term}
  def close(conn, timeout \\ 5000),
    do: Connection.call(conn, :close, timeout)

  @doc """
  Synchronously pings the server.
  """
  @spec ping(pid, pos_integer) :: :ok | {:error, term}
  def ping(conn, timeout \\ 5000) do
    Connection.call(conn, :ping, timeout)
  end

  @doc """
  Makes a synchronous request to the server.

  Sends the payload to the server and awaits a response.
  """
  @spec request(pid, term, pos_integer) :: {:ok, term} | {:error, term}
  def request(conn, payload, timeout \\ 5000) do
    Connection.call(conn, {:request, payload}, timeout)
  end

  @doc """
  Makes an asynchronous request to the server.

  `push` sends a request to the server, but doesn't wait for a reply
  before returning. This is good for one-off messages, but it's
  it's important to be cognizant that you don't overwhelm the server
  with requests, as there's no backpressure.
  """
  @spec push(pid, term) :: :ok
  def push(conn, payload) do
    Connection.cast(conn, {:push, payload})
  end

  @doc false
  def connect(_info, %{sock: nil, host: host, port: port, transport: transport, transport_opts: transport_opts, connect_timeout: connect_timeout}=state) do
    case transport.connect(host, port, transport_opts, connect_timeout) do
      {:ok, sock} ->
        # this is out of the with statement because we need to bind
        # the socket to the state, and if the with statement doesn't
        # complete, the state won't have the socket in it.
        state = %{state | sock: sock}

        with {:ok, _sock} <- update_socket_opts(sock, transport),
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

  defp update_socket_opts(socket, transport) do
    {:ok, opts} = getopts(transport, socket, [:sndbuf, :recbuf])
    send_buffer_size = opts[:sndbuf]
    recieve_buffer_size = opts[:recbuf]
    buffer_size = max(send_buffer_size, recieve_buffer_size)
    setopts(transport, socket, [sndbuf: send_buffer_size,
                           recbuf: recieve_buffer_size,
                           buffer: buffer_size])
    {:ok, socket}
  end

  @doc false
  def disconnect(info, %State{sock: sock, transport: transport}=state) do
    :ok = transport.close(sock)
    case info do
      {:close, from} ->
        Connection.reply(from, :ok)
        {:stop, :normal, %{state | sock: nil}}

      {:error, reason}  ->
        {:stop, reason, %{state | sock: nil}}
    end
  end

  # Connection callbacks

  def handle_call(:ping, caller, %{sock: sock, transport: transport}=state) do
    {next_seq, state} = State.next_sequence(state)
    transport.send(sock, Frames.ping(0, next_seq))

    {:noreply, State.add_waiter(state, next_seq, caller)}
  end

  def handle_call(_, _from, %State{sequence: :go_away}=state) do
    {:reply, {:error, :remote_went_away}, state}
  end

  def handle_call(:close, from, %State{sock: sock, transport: transport}=s) do
    go_away_packet = Frames.goaway(0, 0, "Closing")
    transport.send(sock, go_away_packet)

    {:disconnect, {:close, from}, s}
  end

  def handle_call({:request, payload}, caller, %State{codec: codec, compressor: compressor}=state) do
    {next_seq, state} = State.next_sequence(state)
    in_flight_payload = payload
      |> codec.encode()
      |> compressor.compress()

    state = state
      |> buffer_packet(Frames.request(0, next_seq, in_flight_payload))
      |> State.add_waiter(next_seq, caller)

    {:noreply, state}
  end

  def handle_cast(_, %State{sequence: :go_away}=state),
    do: {:noreply, state}

  def handle_cast({:push, payload}, %State{codec: codec, compressor: compressor}=state) do
    in_flight_payload = payload
      |> codec.encode()
      |> compressor.compress()

    push_frame = Frames.push(0, in_flight_payload)

    {:noreply, buffer_packet(state, push_frame)}
  end

  def handle_info({type, socket, data}, %State{sock: socket}=state) when type in [:tcp, :ssl] do
    make_active_once(state)
    with {:ok, parsed_packets, leftover_data} <- Parser.parse(state.buffer, data) do
      state = handle_packets(parsed_packets, state)

      {:noreply, %State{state | buffer: leftover_data}}
    else

      {:error, {:need_more_data, data}} ->
        {:noreply, %State{state | buffer: data}}
    end
  end

  def handle_info(:flush_packets, %State{packet_buffer: []}=state) do
    {:noreply, state}
  end

  def handle_info(:flush_packets, %State{packet_buffer: packets, sock: socket, transport: transport}=state) do
    transport.send(socket, packets)

    {:noreply, %State{state | packet_buffer: []}}
  end

  def handle_info({type, _socket}, _state) when type in [:tcp_closed, :ssl_closed] do
    {:stop, type, nil}
  end

  def handle_info({type, _, _}, _state) when type in [:tcp_error, :ssl_error] do
    {:stop, type, nil}
  end

  def handle_info({:close_go_away, go_away_code, go_away_data}, %State{sequence: :go_away, waiters: waiters}=state) do
    # tell the waiters that the remote end went away, then close.
    err = {:error, {:remote_went_away, go_away_code, go_away_data}}
    Enum.each(waiters, fn {_, waiter} ->
      Connection.reply(waiter, err)
    end)

    {:disconnect, err, %State{state | waiters: %{}}}
  end

  def handle_info(:send_ping, %State{sock: sock, last_ping: nil, transport: transport}=state) do
    {next_seq, state} = State.next_sequence(state)

    transport.send(sock, Frames.ping(0, next_seq))
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

  defp handle_packet({:ping, _flags, seq}, %State{sock: sock, transport: transport}=state) do
    transport.send(sock, Frames.pong(0, seq))
    state
  end

  defp handle_packet({:pong, _flags, seq}, %State{last_ping: {seq, _ts}}=state) do
    %State{state | last_ping: nil}
  end

  defp handle_packet({:pong, _flags, seq}, %State{last_ping: nil}=state) do
    {state, waiter} = State.remove_waiter(state, seq)

    case waiter do
      nil ->
        Logger.error("Got an unknown pong for sequence #{inspect seq}")

      reply_to ->
        Connection.reply(reply_to, :pong)
    end

    state
  end

  defp handle_packet({:hello_ack, _flags, ping_interval, data}, %State{}=state) do
    [encoding, compression] = data
      |> String.split("|")
      |> Enum.map(&String.trim/1)

    codec = Map.get(state.registered_codecs, encoding)
    compressor = Map.get(state.registered_compressors, compression)


    %State{state | codec: codec, compressor: compressor, ping_interval: ping_interval}
      |> schedule_ping()
  end

  defp handle_packet({:response, _flags, sequence, payload}, %State{codec: codec, compressor: compressor}=state) do
    {state, waiter} = State.remove_waiter(state, sequence)
    decoded_data = payload
      |> compressor.decompress()
      |> codec.decode()

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

  defp getopts(:ssl, socket, opts), do: :ssl.getopts(socket, opts)

  defp getopts(:gen_tcp, socket, opts), do: :inet.getopts(socket, opts)

  defp setopts(:ssl, socket, opts), do: :ssl.setopts(socket, opts)

  defp setopts(:gen_tcp, socket, opts), do: :inet.setopts(socket, opts)

  defp make_active_once(%State{sock: sock, transport: transport}=state) do
    :ok = setopts(transport, sock, [active: :once])
    state
  end

  defp do_upgrade(%State{sock: sock, transport_opts: transport_opts, transport: transport}=state) do
    recv_timeout = Keyword.get(transport_opts, :recv_timeout, @default_timeout)
    with :ok         <- transport.send(sock, upgrade_request(state)),
         {:ok, data} <- transport.recv(sock, 0, recv_timeout),
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

  defp do_loqui_connect(%State{sock: sock, registered_codecs: codecs, registered_compressors: compressors, transport: transport}=state) do
    extract_names = fn(m) ->
      m
        |> Map.keys
        |> Enum.join(",")
    end

    codecs = extract_names.(codecs)
    compressors = extract_names.(compressors)
    transport.send(sock, Frames.hello(0, "#{codecs}|#{compressors}"))

    {:ok, state}
  end
end
