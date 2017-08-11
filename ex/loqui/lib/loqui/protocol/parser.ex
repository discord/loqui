defmodule Loqui.Protocol.Parser do
  use Loqui.{Opcodes, Types}

  def parse(<<>>, extra_data),
    do: parse(extra_data)

  def parse(buffer, extra_data) when is_binary(buffer) and is_binary(extra_data),
    do: parse(buffer <> extra_data)

  def parse(response_data),
    do: do_parse_packet(response_data, [])

  defp do_parse_packet(<<@opcode_hello::uint8, flags::uint8, version::uint8, payload_size::uint32, payload::binary-size(payload_size), rest::binary>>, accum) do

    case parse_settings(payload) do
      [encodings, compressions] ->
        parsed_packet = {:hello, flags, version, encodings, compressions}
        do_parse_packet(rest, [parsed_packet | accum])

      {:error, _}=error  ->
        error
    end
  end

  defp do_parse_packet(<<@opcode_hello_ack::uint8, flags::uint8, ping_interval::uint32, payload_size::uint32, payload_data::binary-size(payload_size), rest::binary>>, accum) do
    do_parse_packet(rest, [{:hello_ack, flags, ping_interval, payload_data} | accum])
  end

  defp do_parse_packet(<<@opcode_ping::uint8, flags::uint8, sequence::uint32, rest::binary>>, accum) do
    do_parse_packet(rest, [{:ping, flags, sequence} | accum])
  end

  defp do_parse_packet(<<@opcode_pong::uint8, flags::uint8, sequence::uint32, rest::binary>>, accum) do
    do_parse_packet(rest, [{:pong, flags, sequence} | accum])
  end

  defp do_parse_packet(<<@opcode_request::uint8, flags::uint8, seq::uint32, psize::uint32, payload::binary-size(psize), rest::binary>>, accum) do
    do_parse_packet(rest, [{:request, flags, seq, payload} | accum])
  end

  defp do_parse_packet(<<@opcode_response::uint8, flags::uint8, sequence::uint32, payload_size::uint32, payload_data::binary-size(payload_size), rest::binary>>, accum) do
    do_parse_packet(rest, [{:response, flags, sequence, payload_data} | accum])
  end

  defp do_parse_packet(<<@opcode_push::uint8, flags::uint8, payload_size::uint32, payload_data::binary-size(payload_size), rest::binary>>, accum) do
    do_parse_packet(rest, [{:push, flags, payload_data} | accum])
  end

  defp do_parse_packet(<<@opcode_goaway, flags::uint8, close_code::uint16, payload_size::uint32, payload_data::binary-size(payload_size), rest::binary>>, accum) do
    do_parse_packet(rest, [{:go_away, flags, close_code, payload_data} | accum])
  end

  defp do_parse_packet(<<@opcode_error, flags::uint8, sequence::uint32, error_code::uint16, payload_size::uint32, payload_data::binary-size(payload_size), rest::binary>>, accum) do
    do_parse_packet(rest, [{:error, flags, sequence, error_code, payload_data} | accum])
  end

  defp do_parse_packet(leftover_data, []) do
    {:error, {:need_more_data, leftover_data}}
  end

  defp do_parse_packet(leftover_data, accum) do
    {:ok, Enum.reverse(accum), leftover_data}
  end

  defp parse_settings(settings) do

    case String.split(settings, "|") do
      [_encoders, _compressors] = settings ->
        settings
          |> Enum.map(&String.split(&1, ","))

      _ ->
        {:error, :not_enough_options}
    end
  end

end
