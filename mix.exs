defmodule Loqui.Mixfile do
  use Mix.Project

  def project do
    [
      app: :loqui,
      version: "0.2.2",
      elixir: "~> 1.3",
      build_embedded: Mix.env == :prod,
      start_permanent: Mix.env == :prod,
      deps: deps,
      elixirc_paths: ["ex/loqui/lib/"],
      description: "An RPC Transport Layer - with minimal bullshit.",
      package: package
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
      {:ex_doc, ">= 0.0.0", only: :dev},
    ]
  end

  defp package do
    [
      name: :loqui,
      files: ["mix.exs", "ex/loqui/lib/*"],
      maintainers: [],
      licenses: [],
      links: [],
    ]
  end
end
