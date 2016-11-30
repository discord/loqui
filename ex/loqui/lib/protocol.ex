defmodule Loqui.Protocol do
  use Loqui.{Opcodes, Types}

  def handle_data(<<@opcode_hello :: uint8, flags :: uint8, version :: uint8, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    settings = parse_settings(payload)
    [encodings, compressions] = settings
    {:ok, {:hello, flags, version, encodings, compressions}, rest}
  end
  def handle_data(<<@opcode_ping :: uint8, flags :: uint8, seq :: uint32, rest :: binary>>) do
    {:ok, {:ping, flags, seq}, rest}
  end
  def handle_data(<<@opcode_pong :: uint8, flags :: uint8, seq :: uint32, rest :: binary>>) do
    {:ok, {:pong, flags, seq}, rest}
  end
  def handle_data(<<@opcode_request :: uint8, flags :: uint8, seq :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:request, flags, seq, payload}, rest}
  end
  def handle_data(<<@opcode_response :: uint8, flags :: uint8, seq :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:response, flags, seq, payload}, rest}
  end
  def handle_data(<<@opcode_push :: uint8, flags :: uint8, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:push, flags, payload}, rest}
  end
  def handle_data(<<@opcode_goaway :: uint8, flags :: uint8, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:goaway, flags, payload}, rest}
  end
  def handle_data(<<@opcode_error :: uint8, flags :: uint8, seq :: uint32, code :: uint16, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:error, flags, seq, code, payload}, rest}
  end
  def handle_data(data) do
    {:continue, data}
  end

  defp parse_settings(settings), do: String.split(settings, "|") |> Enum.map(&parse_setting/1)

  defp parse_setting(""), do: []
  defp parse_setting(setting), do: String.split(setting, ",")
end
