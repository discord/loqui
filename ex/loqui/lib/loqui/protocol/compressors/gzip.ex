defmodule Loqui.Protocol.Compressors.Gzip do
  @moduledoc """
  A compressor that uses Gzip

  The name used during negotiation is "gzip"
  """

  @behaviour Loqui.Protocol.Compressor

  @doc false
  def name,
    do: "gzip"

  @doc false
  def compress(iodata),
    do: :zlib.gzip(iodata)

  @doc false
  def decompress(iodata),
    do: :zlib.gunzip(iodata)
end
