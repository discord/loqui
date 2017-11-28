defmodule ClientTest do
  use ExUnit.Case

  defmodule Server do
    @behaviour Loqui.Handler

    def loqui_init(_transport, _opts) do
      opts = %{
        supported_encodings: ["erlpack"],
        supported_compressions: [],
      }
      {:ok, opts}
    end

    def loqui_request(request, encoding) do
      Process.send(Test, {:request, request}, [])

      case request do
        "go away" ->
          {:go_away, 32, "Leave me alone!"}

        _ ->
          request
      end
    end

    def loqui_terminate(_reason),
      do: :ok
  end

  setup_all do
    {:ok, _server} = Loqui.Server.start_link(8080, "/_rpc", handler: Server)
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

  test "it should respond to go away packets", ctx do
    assert {:error, {:remote_went_away, 32, "Leave me alone!"}} = Loqui.Client.request(ctx.client, "go away")
  end

  test "it should handle really big requests and responses", ctx do
    req = String.duplicate("hello", 100_000)
    assert {:request, ^req} = Loqui.Client.request(ctx.client, {:request, req})
  end

end
