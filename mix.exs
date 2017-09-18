defmodule Loqui.Mixfile do
  use Mix.Project

  @project_url "https://github.com/hammerandchisel/loqui/"
  @version "0.3.1"

  def project do
    [
      app: :loqui,
      version: @version,
      elixir: "~> 1.3",
      build_embedded: Mix.env == :prod,
      start_permanent: Mix.env == :prod,
      deps: deps(),
      elixirc_paths: ["ex/loqui/lib/"],
      test_paths: ["ex/loqui/test"],
      description: "An RPC Transport Layer - with minimal bullshit.",
      package: package(),
      source_url: @project_url,
      homepage_url: @project_url
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
      {:ex_doc, ">= 0.0.0", only: :dev},
      {:connection, "~> 1.0"},
      {:jiffy, "~> 0.14.11", optional: true},
      {:msgpax, "~> 2.0", optional: true},
    ]
  end

  defp package do
    [
      name: :loqui,
      files: ~w(README.md mix.exs ex/loqui/lib/*),
      maintainers: ["Jesse Howarth", "Stanislav Vishnevskiy", "Steve Cohen"],
      licenses: [],
      links: %{"GitHub" => @project_url},
    ]
  end
end
