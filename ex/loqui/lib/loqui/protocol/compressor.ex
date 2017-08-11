defmodule Loqui.Protocol.Compressor do
  @type t :: module

  @callback compress(iodata) :: iodata
  @callback decompress(iodata) :: iodata
  @callback name() :: String.t
end
