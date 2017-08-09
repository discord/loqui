if Code.ensure_compiled?(Msgpax) do
  defmodule Loqui.Protocol.Codecs.Msgpack do
    @moduledoc """
    The Msgpack codec.

    Encodes / decodes data in the (http://msgpack.org/index.html)
    format. Like JSON, not all terms can be represented in msgpack,
    so it's important to ensure that your data can be serialized
    correctly.

    The name used during negotiation is "msgpack"
    """
    @behaviour Loqui.Protocol.Codec

    @doc false
    def name,
      do: "msgpack"

    @doc false
    def encode(term),
      do: Msgpax.pack!(term)

    @doc false
    def decode(binary),
      do: Msgpax.unpack!(binary)
  end
end
