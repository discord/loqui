defmodule EchoServer.Handler do

  alias Loqui.Protocol.Codecs.Msgpack

  ## Loqui Callbacks

  def loqui_init(_transport, _opts) do
    opts = %{supported_encodings: [Msgpack], supported_compressions: []}
    {:ok, opts}
  end

  def loqui_request(request, _encoding), do: request

  def loqui_push(_push, _encoding), do: :ok

  def loqui_terminate(_reason), do: :ok
end
