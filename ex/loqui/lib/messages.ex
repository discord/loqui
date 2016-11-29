defmodule Loqui.Messages do
  # TODO: how to share these opcodes
  @opcode_hello 1
  @opcode_hello_ack 2
  @opcode_ping 3
  @opcode_pong 4
  @opcode_request 5
  @opcode_response 6
  @opcode_push 7
  @opcode_goaway 8
  @opcode_error 9

  # TODO: how to share these macros
  defmacro uint8 do
    quote do: unsigned-integer-size(8)
  end
  defmacro uint16 do
    quote do: unsigned-integer-size(16)
  end
  defmacro uint32 do
    quote do: unsigned-integer-size(32)
  end

  def make_hello_ack(flags, ping_interval, settings_payload) do
    <<@opcode_hello_ack, flags, ping_interval :: uint32, byte_size(settings_payload) :: uint32, settings_payload :: binary>>
  end

  def make_pong(flags, seq) do
    <<@opcode_pong, flags, seq :: uint32>>
  end

  def make_response(flags, seq, response) do
    <<@opcode_response, flags, seq :: uint32, byte_size(response) :: uint32, response :: binary>>
  end

  def make_error(flags, code, seq, reason) do
    <<@opcode_error, flags, code :: uint8, seq :: uint32, byte_size(reason) :: uint32, reason :: binary>>
  end

  def make_goaway(flags, code, reason) do
    <<@opcode_goaway, flags, code :: uint8, byte_size(reason) :: uint32, reason :: binary>>
  end

end