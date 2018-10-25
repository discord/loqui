defmodule Loqui.Protocol.Codec do
  @moduledoc """
  A Codec behaviour

  Codecs turn terms into IOdata and IOdata into terms.

  """

  @type t :: module

  @doc """
  Turns the given term into IOdata
  """
  @callback encode(term) :: iodata

  @doc """
  Decodes the IOdata into a term
  """
  @callback decode(iodata) :: term

  @doc """
  The name of the codec used in the negotiation
  phase of the loqui protocol
  """
  @callback name() :: String.t()
end
