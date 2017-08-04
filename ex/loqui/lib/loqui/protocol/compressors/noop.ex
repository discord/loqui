defmodule Loqui.Protocol.Compressors.NoOp do
  @behaviour Loqui.Protocol.Compressor

  def name,
    do: " "

  def compress(binary),
    do: binary

  def decompress(binary),
    do: binary
end
