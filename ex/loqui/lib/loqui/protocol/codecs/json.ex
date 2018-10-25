if Code.ensure_compiled?(:jiffy) do
  defmodule Loqui.Protocol.Codecs.Json do
    @moduledoc """
    The Json codec.

    This codec represents many erlang terms in JavaScript object
    notation (JSON) format. Not all Erlang terms can be represented
    in JSON, so it's important to ensure that any data you wish to serialize
    can indeed be represented as JSON Before sending data into this codec.

    The name used during negotiation is "json"
    """

    @behaviour Loqui.Protocol.Codec
    @jiffy_encode_opts [:use_nil, bytes_per_iter: 4096]
    @jiffy_decode_opts [:use_nil, :return_maps, bytes_per_iter: 4096]

    @doc false
    def name,
      do: "json"

    @doc false
    def encode(term),
      do: :jiffy.encode(term, @jiffy_encode_opts)

    @doc false
    def decode(json),
      do: :jiffy.decode(json, @jiffy_decode_opts)
  end
end
