defmodule Loqui.Protocol.Frames do
  @moduledoc false
  use Loqui.{Opcodes, Types}

  def hello(flags, payload_data) do
    [@opcode_hello, flags, 1, <<size_of(payload_data)::uint32>> | payload_data]
  end

  def hello_ack(flags, ping_interval, settings_payload) do
    [
      @opcode_hello_ack,
      flags,
      <<ping_interval::uint32>>,
      <<size_of(settings_payload)::uint32>> | settings_payload
    ]
  end

  def ping(flags, seq) do
    [@opcode_ping, flags, <<seq::uint32>>]
  end

  def pong(flags, seq) do
    [@opcode_pong, flags, <<seq::uint32>>]
  end

  def push(flags, payload) do
    [@opcode_push, flags, <<size_of(payload)::uint32>> | payload]
  end

  def request(flags, seq, payload) do
    [@opcode_request, flags, <<seq::uint32>>, <<size_of(payload)::uint32>> | payload]
  end

  def response(flags, seq, response) do
    [@opcode_response, flags, <<seq::uint32>>, <<size_of(response)::uint32>> | response]
  end

  def error(flags, code, seq, reason) do
    [
      @opcode_error,
      flags,
      <<seq::uint32>>,
      <<code::uint16>>,
      <<size_of(reason)::uint32>> | reason
    ]
  end

  def goaway(flags, code, reason) do
    [@opcode_goaway, flags, <<code::uint16>>, <<size_of(reason)::uint32>> | reason]
  end

  def size_of(bin) when is_bitstring(bin),
    do: byte_size(bin)

  def size_of(io_data) when is_list(io_data),
    do: IO.iodata_length(io_data)
end
