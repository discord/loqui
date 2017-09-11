defmodule EchoServer.Mixfile do
  use Mix.Project

  def project do
    [
      app: :echo_server,
      version: "0.2.0",
      elixir: "~> 1.3",
      build_embedded: Mix.env == :prod,
      start_permanent: Mix.env == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      applications: [:logger, :cowboy],
      mod: {EchoServer, []}
    ]
  end

  defp deps do
    [
      {:cowboy, "~> 1.0"},
      {:loqui, path: "../../.."},
      {:msgpax, "~>2.0"},
    ]
  end
end
