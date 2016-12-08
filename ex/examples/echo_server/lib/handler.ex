defmodule EchoServer.Handler do
   # HTTP Behavior

  def init(_transport, req, _opts) do
    case :cowboy_req.header("upgrade", req) do
      {"loqui", _req} -> {:upgrade, :protocol, Loqui.CowboyProtocol}
      {:undefined, req} -> {:ok, req, nil}
    end
  end

  def handle(req, _state) do
    :cowboy_req.reply(401, [], "", req)
  end

  def terminate(_reason, _req, _state), do: :ok

  ## Loqui Callbacks

  def loqui_init(_transport, req, _opts) do
    opts = %{supported_encodings: ["msgpack"], supported_compressions: []}
    {:ok, req, opts}
  end

  def loqui_request(request, _encoding), do: request

  def loqui_push(_push, _encoding), do: :ok

  def loqui_terminate(_reason, _req), do: :ok
end