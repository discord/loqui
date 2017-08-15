defmodule Loqui.Protocol.Codecs do
  @moduledoc false

  @enabled_codecs [Erlpack, Msgpack, Json]
    |> Enum.map(&Module.concat(__MODULE__, &1))
    |> Enum.filter(&Code.ensure_compiled?/1)

  def all() do
    @enabled_codecs
  end
end
