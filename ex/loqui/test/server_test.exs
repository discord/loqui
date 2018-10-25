defmodule Loqui.ServerTest do
  use ExUnit.Case

  defmodule Handler do
    @behaviour Loqui.Handler

    def loqui_init(_, _) do
      {:ok, %{supported_encodings: ["erlpack"], supported_compressions: []}}
    end

    def loqui_request(_, _),
      do: ""

    def loqui_terminate(_),
      do: :ok
  end

  @port 38419
  setup do
    {:ok, handler} = Loqui.Server.start_link(@port, "/_rpc", Handler, server_name: :server_test)

    :inets.start()

    on_exit(fn ->
      ref = Process.monitor(handler)
      Loqui.Server.stop(:server_test)

      receive do
        {:DOWN, ^ref, _, _, _} ->
          :ok
      end
    end)

    :ok
  end

  def test_url(path \\ "/") do
    'http://localhost:#{@port}#{path}'
  end

  test "it should turn away clients not requesting the right path" do
    {:ok, r} = :httpc.request(test_url())

    assert {{'HTTP/1.1', 404, 'Not Found'}, _, _} = r
  end

  test "it should handle clients not setting the upgrade header" do
    {:ok, r} = :httpc.request(test_url("/_rpc"))

    assert {{'HTTP/1.1', 404, 'Not Found'}, _, _} = r
  end

  test "it should throw out HTTP requests that are too big" do
    big_header_value =
      "Haxor"
      |> String.duplicate(100)
      |> String.to_charlist()

    headers =
      for n <- 1..10_000 do
        {'bogus-header-#{n}', big_header_value}
      end

    # httpc can die for two reasons here, both are fine; as long as we stop processing
    # the request
    case :httpc.request(:get, {test_url("/_rpc"), headers}, [], []) do
      {:ok, rsp} ->
        assert {{'HTTP/1.1', 413, 'Request Entity Too Large'}, _, _} = rsp

      {:error, reason} ->
        assert reason == :socket_closed_remotely
    end
  end
end
