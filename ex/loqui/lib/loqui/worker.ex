defmodule Loqui.Worker do
  require Logger

  def request({m, f, a}, seq, from) do
    response = apply(m, f, a)
    send(from, {:response, seq, response})
  end

  def push({m, f, a}) do
    apply(m, f, a)
  end
end