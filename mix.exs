defmodule Wasmtime.MixProject do
  use Mix.Project

  def project do
    [
      app: :wasmtime,
      version: "0.3.0",
      elixir: "~> 1.10",
      start_permanent: Mix.env() == :prod,
      compilers: Mix.compilers(),
      name: "Wasmtime",
      description: "Elixir WebAssembly runtime powered by Wasmtime",
      package: package(),
      deps: deps(),
      source_url: "https://github.com/viniarck/wasmtime-ex",
      homepage_url: "https://github.com/viniarck/wasmtime-ex",
      docs: [
        main: "Wasmtime",
        extras: ["README.md"]
      ],
      test_coverage: [tool: ExCoveralls],
      preferred_cli_env: [
        coveralls: :test,
        "coveralls.detail": :test,
        "coveralls.post": :test,
        "coveralls.html": :test
      ]
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      extra_applications: [:logger]
    ]
  end

  # Run "mix help deps" to learn about dependencies.
  defp deps do
    [
      {:rustler, "~> 0.26.0"},
      {:jason, "~> 1.2.2"},
      {:excoveralls, "~> 0.13.2", only: :test},
      {:ex_doc, "~> 0.23", only: :dev, runtime: false},
      {:benchee, "~> 1.0", only: :dev}
    ]
  end

  defp package() do
    [
      files:
        ~w(lib native/wasmtime_ex/src native/wasmtime_ex/Cargo* native/wasmtime_ex/.cargo .formatter.exs mix.exs README* LICENSE*
                ),
      licenses: ["Apache-2.0"],
      links: %{
        "GitHub" => "https://github.com/viniarck/wasmtime-ex",
        "Docs" => "https://hexdocs.pm/wasmtime"
      }
    ]
  end
end
