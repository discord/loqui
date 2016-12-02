defmodule Loqui.Mixfile do
  use Mix.Project

  def project do
    [
      app: :loqui,
      version: "0.0.1",
      elixir: "~> 1.3",
      build_embedded: Mix.env == :prod,
      start_permanent: Mix.env == :prod,
      deps: deps
    ]
  end

  def application do
    [
      applications: [:logger, :cowboy, :poolboy],
      mod: {Loqui, []}
    ]
  end

  defp deps do
    [
      {:cowboy, "~> 1.0.0"},
      {:poolboy, "~> 1.5.1"},
    ]
  end
end
