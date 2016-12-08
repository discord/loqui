defmodule EchoServer do
  use Application

  def start(_type, _args) do
    import Supervisor.Spec, warn: false

    cowboy_dispatch = :cowboy_router.compile([
      {:_, [
        {"/_rpc", EchoServer.Handler, []},
      ]}
    ])

    children = [
      :ranch.child_spec(:http, 100,
        :ranch_tcp, [
          port: 8080,
          max_connections: :infinity
        ],
        :cowboy_protocol, [env: [dispatch: cowboy_dispatch]]),
    ]

    Supervisor.start_link(children, strategy: :one_for_one, name: EchoServer.Supervisor)
  end
end
