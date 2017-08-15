defmodule Loqui.Protocol.Compressors do
  @moduledoc false

  @enabled_compressors [NoOp, Gzip]
  |> Enum.map(&Module.concat(__MODULE__, &1))
  |> Enum.filter(&Code.ensure_compiled?/1)

  def all(),
    do: @enabled_compressors
end
