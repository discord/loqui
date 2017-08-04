defmodule Loqui.Protocol.Frames do
  use Loqui.{Opcodes, Types}

  def hello(flags, payload_data) do
    <<@opcode_hello, flags :: uint8, 1::uint8, byte_size(payload_data) :: uint32, payload_data :: binary>>
  end

  def hello_ack(flags, ping_interval, settings_payload) do
    <<@opcode_hello_ack, flags, ping_interval :: uint32, byte_size(settings_payload) :: uint32, settings_payload :: binary>>
  end

  def ping(flags, seq) do
    <<@opcode_ping, flags, seq :: uint32>>
  end

  def pong(flags, seq) do
    <<@opcode_pong, flags, seq :: uint32>>
  end

  def push(flags, payload) do
    <<@opcode_push, flags::uint8, byte_size(payload)::uint32, payload::binary>>
  end

  def request(flags, seq, payload) do
    <<@opcode_request, flags, seq::uint32, byte_size(payload)::uint32, payload::binary>>
  end

  def response(flags, seq, response) do
    <<@opcode_response, flags, seq :: uint32, byte_size(response) :: uint32, response :: binary>>
  end

  def error(flags, code, seq, reason) do
    <<@opcode_error, flags, seq :: uint32, code :: uint16, byte_size(reason) :: uint32, reason :: binary>>
  end

  def goaway(flags, code, reason) do
    <<@opcode_goaway, flags, code :: uint8, byte_size(reason) :: uint32, reason :: binary>>
  end

end
