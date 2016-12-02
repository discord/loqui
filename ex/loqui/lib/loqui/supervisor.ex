defmodule Loqui.Supervisor do
  use Supervisor
  require Logger

  def start_link(opts \\ []) do
    Supervisor.start_link(__MODULE__, :ok, opts)
  end

  def init(:ok) do
    children = [
      :poolboy.child_spec(:loqui, [
        name: {:local, Loqui.pool_name},
        size: Application.get_env(:loqui, :pool_size, 50),
        max_overflow: 10,
        worker_module: Loqui.Worker,
        strategy: :fifo,
      ]),
    ]
    supervise(children, strategy: :one_for_one)
  end
end