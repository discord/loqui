defmodule Loqui.Worker do
  use GenServer
  require Logger

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, :ok, opts)
  end

  def init(:ok) do
    {:ok, :ok}
  end

  def request(pid, {m, f, a}, seq, from) do
    GenServer.cast(pid, {:request, {m, f, a}, seq, from})
  end

  def push(pid, {m, f, a}) do
    GenServer.cast(pid, {:push, {m, f, a}})
  end

  def handle_cast({:request, {m, f, a}, seq, from}, state) do
    response = apply(m, f, a)
    send(from, {:response, seq, response})
    {:noreply, state}
  end

  def handle_cast({:push, {m, f, a}}, state) do
    apply(m, f, a)
    {:noreply, state}
  end
end