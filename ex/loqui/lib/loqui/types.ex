defmodule Loqui.Types do
  defmacro __using__(_) do
    quote do
      defmacro uint8 do
        quote do: unsigned-integer-size(8)
      end
      defmacro uint16 do
        quote do: unsigned-integer-size(16)
      end
      defmacro uint32 do
        quote do: unsigned-integer-size(32)
      end
    end
  end
end
