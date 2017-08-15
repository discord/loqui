defmodule Loqui.Mixfile do
  use Mix.Project

  def project do
    [
      app: :loqui,
      version: "0.2.10",
      elixir: "~> 1.3",
      build_embedded: Mix.env == :prod,
      start_permanent: Mix.env == :prod,
      deps: deps(),
      elixirc_paths: ["ex/loqui/lib/"],
      test_paths: ["ex/loqui/test"],
      description: "An RPC Transport Layer - with minimal bullshit.",
      package: package()
    ]
  end

  def application do
    [
      applications: [:logger, :cowboy]
    ]
  end

  defp deps do
    [
      {:cowboy, github: "hammerandchisel/cowboy", ref: "ad4ec7cfa76abe054c24e64b61d8e9632a782810"},
      {:ex_doc, ">= 0.0.0", only: :dev},
      {:connection, "~> 1.0"},
      {:jiffy, "~> 0.14.11", optional: true},
      {:msgpax, "~> 2.0", optional: true},
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
