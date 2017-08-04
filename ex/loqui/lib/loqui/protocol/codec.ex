defmodule Loqui.Protocol.Codec do
  @type t :: module

  @callback encode(term) :: iodata
  @callback decode(iodata) :: term
  @callback name() :: String.t
end
