defmodule Loqui.Protocol.Compressors.NoOp do
  @moduledoc """
  The default compressor, which does nothing.

  The name used during negotiation is "" (empty string)
  """

  @behaviour Loqui.Protocol.Compressor

  @doc false
  def name,
    do: ""

  @doc false
  def compress(binary),
    do: binary

  @doc false
  def decompress(binary),
    do: binary
end
