defmodule Loqui.Types do
  @moduledoc false

  defmacro __using__(_) do
    quote do
      @doc false
      defmacro uint8 do
        quote do: unsigned - integer - size(8)
      end

      @doc false
      defmacro uint16 do
        quote do: unsigned - integer - size(16)
      end

      @doc false
      defmacro uint32 do
        quote do: unsigned - integer - size(32)
      end
    end
  end
end
