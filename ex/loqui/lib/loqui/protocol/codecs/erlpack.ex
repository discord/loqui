defmodule Loqui.Protocol.Codecs.Erlpack do
  @behaviour Loqui.Protocol.Codec

  def name,
    do: "erlpack"

  def encode(term),
    do: :erlang.term_to_binary(term)

  def decode(binary),
    do: :erlang.binary_to_term(binary)
end
