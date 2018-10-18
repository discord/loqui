defmodule Loqui.Server do

  defmodule Http do
    defmodule State do
      defstruct [:handler, :path, :transport, :socket, :buffer, :handler_opts]

      def new(socket, transport, opts) do
        %__MODULE__{
          path: Keyword.fetch!(opts, :loqui_path),
          handler: Keyword.fetch!(opts, :handler),
          socket: socket,
          transport: transport,
          buffer: "",
          handler_opts: Keyword.get(opts, :handler_opts, %{})
        }
      end
    end

    require Logger
    alias Loqui.RanchProtocol

    @behaviour :ranch_protocol
    @max_payload 1024 * 100

    def start_link(ref, socket, transport, opts) do
      pid = spawn_link(__MODULE__, :init, [{ref, socket, transport, opts}])
      {:ok, pid}
    end

    def init({ref, socket, transport, opts}) do
      state = State.new(socket, transport, opts)
      transport_opts = opts
        |> Keyword.get(:transport_opts, [])
        |> Keyword.put(:active, true)

      # This will fail if the path hasn't been set.
      _loqui_path = Keyword.fetch!(opts, :loqui_path)
      :ok = :ranch.accept_ack(ref)
      transport.setopts(socket, transport_opts)

      loop(state)
    end

    def loop(%{socket: sock} = state) do
      receive do
        {type, ^sock, data} when type in [:tcp, :ssl] ->
          case handle_tcp_data(data, state) do
            :ok ->
              exit(:normal)

            {:error, {:not_complete, request}} ->
              loop(%State{state | buffer: request})

            {:error, reason} ->
              Logger.error("Client #{inspect ip_address(sock)} caused #{inspect reason}. Exiting.")
              exit(reason)
          end

        {type, ^sock, reason} when type in [:tcp_error, :ssl_error] ->
          Logger.warn("TCP error #{inspect reason} from client #{inspect ip_address(sock)}. Closing")
          exit(reason)

        {type, ^sock} when type in [:tcp_closed, :ssl_closed]  ->
          Logger.info("Client #{inspect ip_address(sock)} closed.")
          exit(:normal)
      end
    end

    defp handle_tcp_data(extra_data, %{socket: sock, path: loqui_path, transport: transport, buffer: buffer} = state) do
      with {:ok, ^loqui_path, {headers, _}} <- try_parse_request(buffer <> extra_data),
           header_val when is_bitstring(header_val) <- :proplists.get_value("upgrade", headers),
           "loqui" <- String.downcase(header_val) do

        upgrade_headers = [{"connection", "Upgrade"},{"upgrade", "loqui"}]
        response = :cow_http.response(101, :"HTTP/1.1", upgrade_headers)
        transport.send(sock, response)
        RanchProtocol.upgrade(sock, transport, state.handler, state.handler_opts)
      else
        {:error, :payload_too_large} ->
          response = :cow_http.response(413, :"HTTP/1.1", [{"connection", "close"}])
          transport.send(sock, response)
          {:error, :payload_too_large}

        {:error, _} = err ->
          err

        _ ->
          response = :cow_http.response(404, :"HTTP/1.1", [{"connection", "close"}])
          transport.send(sock, response)
          :ok
      end
    end

    defp try_parse_request(data) when byte_size(data) >= @max_payload,
      do: {:error, :payload_too_large}

    defp try_parse_request(data) do
      with true <- String.ends_with?(data, "\r\n\r\n"),
           ["GET " <> rest_of_line, rest] <- String.split(data, "\r\n", parts: 2),
           [path, _version] <- String.split(rest_of_line, " ", parts: 2) do

        {:ok, path, :cow_http.parse_headers(rest)}
      else
        _ ->
          {:error, {:not_complete, data}}
      end
    end

    defp ip_address(socket) do
      with {:ok, {ip, _port}} <- :inet.peername(socket) do
        :inet_parse.ntoa(ip)
      end
    end
  end

  @type transport :: :ranch_tcp | :ranch_ssl
  @type transport_option :: :gen_tcp.option | :ranch.opt
  @type option :: {:loqui_path, String.t}
  | {:transport_opts, [transport_option]}
  | {:handler_opts, Keyword.t}
  | {:transport, transport}
  @type options :: [{:handler, module} | [option]]

  @type tcp_port :: (0..65535)
  @type path :: String.t

  @spec start_link(tcp_port, path, module, options) :: {:ok, pid} | {:error, any}
  def start_link(port, path, handler, opts \\ []) do
    server_name = Keyword.get(opts, :server_name, :loqui)
    transport = Keyword.get(opts, :transport, :ranch_tcp)
    opts = opts
      |> Keyword.put(:handler, handler)
      |> Keyword.put(:loqui_path, path)

    {transport_opts, opts} = Keyword.pop(opts, :transport_opts, [])
    transport_opts = Keyword.put(transport_opts, :port, port)

    :ranch.start_listener(server_name, transport, transport_opts, Http, opts)
  end

  defdelegate stop(listener_name), to: :ranch, as: :stop_listener

end
