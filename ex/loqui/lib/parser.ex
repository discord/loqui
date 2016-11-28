defmodule Loqui.Parser do
  @opcode_hello 1
  @opcode_select_encoding 2
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
  defmacro uint32 do
    quote do: unsigned-integer-size(32)
  end

  def handle_data(<<@opcode_hello :: uint8, version :: uint8, ping_interval :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:hello, version, ping_interval, payload}, rest}
  end
  def handle_data(<<@opcode_select_encoding :: uint8, psize :: uint32, encoding :: binary-size(psize), rest :: binary>>) do
    {:ok, {:select_encoding, encoding}, rest}
  end
  def handle_data(<<@opcode_ping :: uint8, seq :: uint32, rest :: binary>>) do
    {:ok, {:ping, seq}, rest}
  end
  def handle_data(<<@opcode_pong :: uint8, seq :: uint32, rest :: binary>>) do
    {:ok, {:pong, seq}, rest}
  end
  def handle_data(<<@opcode_request :: uint8, seq :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:request, seq, payload}, rest}
  end
  def handle_data(<<@opcode_response :: uint8, seq :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:response, seq, payload}, rest}
  end
  def handle_data(<<@opcode_push :: uint8, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:push, payload}, rest}
  end
  def handle_data(<<@opcode_goaway :: uint8, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:goaway, payload}, rest}
  end
  def handle_data(<<@opcode_error :: uint8, seq :: uint32, psize :: uint32, payload :: binary-size(psize), rest :: binary>>) do
    {:ok, {:error, seq, payload}, rest}
  end
  def handle_data(data) do
    {:continue, data}
  end
end
