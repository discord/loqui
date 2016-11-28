defmodule Loqui.Mixfile do
  use Mix.Project

  def project do
    [
      app: :loqui,
      version: "0.0.1",
      elixir: "~> 1.3",
      deps: deps
    ]
  end

  def application do
    [
      applications: [:logger, :cowboy]
    ]
  end

  defp deps do
    [
      {:cowboy, "~> 1.0.0"},
    ]
  end
end
