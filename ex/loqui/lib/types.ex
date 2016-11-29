defmodule Loqui.Types do
  defmacro __using__(_) do
    quote do
      @opcode_hello 1
      @opcode_hello_ack 2
      @opcode_ping 3
      @opcode_pong 4
      @opcode_request 5
      @opcode_response 6
      @opcode_push 7
      @opcode_goaway 8
      @opcode_error 9

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
