defmodule Loqui.Messages do
  use Loqui.{Opcodes, Types}

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