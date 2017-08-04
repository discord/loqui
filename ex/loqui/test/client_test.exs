defmodule ClientTest do
  use ExUnit.Case

  defmodule Server do
    def init(transport, req, opts) do
      case :cowboy_req.header("upgrade", req) do
        {"loqui", _req} ->
          {:upgrade, :protocol, Loqui.CowboyProtocol}

        {:undefined, req} ->
          {:ok, req, nil}
      end
    end

    def handle(req, _state),
      do: :cowboy_req.reply(401, [], "", req)

    def terminate(_, _, _),
      do: :ok

    def loqui_init(_transport, req, opts) do
      opts = %{
        supported_encodings: ["erlpack"],
        supported_compressions: [" "],
      }
      {:ok, req, opts}
    end

    def loqui_request(request, encoding) do
      decoded =
        case encoding do
          "erlpack" ->
            :erlang.binary_to_term(request)

          _ ->
            request
        end
      Process.send(Test, {:request, decoded}, [])
      request
    end

    def loqui_push(push, _encoding),
      do: :ok

    def loqui_terminate(_reason, _req),
      do: :ok
  end

  setup_all do
    routes = :cowboy_router.compile([
      {:_, [
          {"/_rpc", Server, []}
        ]}
    ])
    :cowboy.start_http(Server, 1, [port: 8080], [env: [dispatch: routes]])
    :ok
  end

  setup do
    Process.register(self(), Test)
    {:ok, client_pid} = Loqui.Client.start_link("localhost", 8080, "/_rpc",
      loqui_opts: [
      ])

    {:ok, client: client_pid}
  end

  test "it should be able to ping the server", ctx do
    assert :pong = Loqui.Client.ping(ctx.client)
  end

  test "it should be able to send an RPC call", ctx do
    assert {:foo, 3, "hello"} == Loqui.Client.request(ctx.client, {:foo, 3, "hello"})
    assert_receive {:request, {:foo, 3, "hello"}}
  end

  test "it should be able to send a push", ctx do
    assert :ok = Loqui.Client.push(ctx.client, {:foo, 3, "push!"})
    assert_receive {:request, {:foo, 3, "push!"}}
  end

  test "it should be able to close", ctx do
    assert :ok = Loqui.Client.close(ctx.client)
  end

end
