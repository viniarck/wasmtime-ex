defmodule Wasmtime.MixProject do
  use Mix.Project

  def project do
    [
      app: :wasmtime,
      version: "0.1.0",
      elixir: "~> 1.10",
      start_permanent: Mix.env() == :prod,
      compilers: [:rustler] ++ Mix.compilers(),
      rustler_crates: [
        wasmtime_ex: [
          mode: if(Mix.env() == :prod, do: :release, else: :debug)
        ]
      ],
      name: "Wasmtime",
      description: "Elixir WebAssembly runtime powered by Wasmtime",
      deps: deps(),
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
      {:rustler, "~> 0.21.1"},
      {:excoveralls, "~> 0.13.2", only: :test},
      {:benchee, "~> 1.0", only: :dev}
    ]
  end
end
