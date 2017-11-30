defmodule EchoServer do
  use Application

  def start(_type, _args) do
    import Supervisor.Spec, warn: false
    children = [
      worker(Loqui.Server, [8080, "/_rpc", EchoServer.Handler]),
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: EchoServer.Supervisor)
  end
end
