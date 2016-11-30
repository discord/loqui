defmodule Loqui.Worker do
  use GenServer
  require Logger

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, :ok, opts)
  end

  def init(:ok) do
    {:ok, :ok}
  end

  def run(pid, {m, f, a}, on_complete) do
    GenServer.cast(pid, {:run, {m, f, a}, on_complete})
  end

  def handle_cast({:run, {m, f, a}, on_complete}, state) do
    {response, _} = apply(m, f, a)
    on_complete.(response)
    {:noreply, state}
  end
end