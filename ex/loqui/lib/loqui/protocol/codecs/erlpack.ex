defmodule Loqui.Protocol.Codecs.Erlpack do
  @moduledoc """
  The Erlpack codec.

  This codec uses `:erlang.term_to_binary` and `:erlang.binary_to_term`
  to encode and decode data. It's possible to represent all erlang terms
  in this format, even functions.

  The name used during negotiation is "erlpack"
  """
  @behaviour Loqui.Protocol.Codec

  @doc false
  def name,
    do: "erlpack"

  @doc false
  def encode(term),
    do: :erlang.term_to_binary(term)

  @doc false
  def decode(binary),
    do: :erlang.binary_to_term(binary)
end
