defmodule Loqui.Protocol.Compressor do
  @moduledoc """
  A compressor

  A compressor is concerned with compressing
  and decompressing iodata.
  """

  @type t :: module

  @doc """
  Compresses encoded data before sending it across
  the network.
  """
  @callback compress(iodata) :: iodata

  @doc """
  Decompresses data retrieved from the network.
  """
  @callback decompress(iodata) :: iodata

  @doc """
  The name of the compressor.

  This name is sent during protocol negotiation between
  the client and the server.
  """
  @callback name() :: String.t()
end
