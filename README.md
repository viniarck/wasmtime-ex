<div align="center">
  <h1><code>wasmtime-ex</code></h1>

  <strong>💧Elixir WebAssembly runtime powered by <a href="https://github.com/bytecodealliance/wasmtime">Wasmtime 🦀</a></strong>

  <p></p>
  <p>
    <a href="https://github.com/viniarck/wasmtime-ex/workflows/.github/workflows/tests.yml/badge.svg"><img src="https://github.com/viniarck/wasmtime-ex/workflows/.github/workflows/tests.yml/badge.svg" alt="tests" /></a>
    <a href="https://img.shields.io/hexpm/v/wasmtime.svg"><img src="https://img.shields.io/hexpm/v/wasmtime.svg" alt="hex.pm version" /></a>
    <a href="https://img.shields.io/hexpm/v/wasmtime.svg"><img src="https://img.shields.io/hexpm/dt/wasmtime.svg" alt="hex.pm downloads" /></a>
  </p>


  <h3>
    <a href="https://hexdocs.pm/wasmtime">Docs</a>
  </h3>

  <!-- this html was based on https://github.com/bytecodealliance/wasmtime -->
</div>

## Installation

You can add `wasmtime` to your list of dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:wasmtime, "~> 0.2.0"}
  ]
end
```

## Usage

In this example, the Wasm module is compiled, instantiated and a host function is called and imported from Elixir:

```elixir
mod = ~S/
    (module
      (import "" "" (func $compute (param i32) (result i32)))
      (func (export "run") (param i32) (result i32) (call $compute (local.get 0)))
    )
    /

{:ok, pid} =
  Wasmtime.load(%Wasmtime.FromBytes{
    bytes: mod,
    func_imports: [
      {fn x ->
         "Hello from Elixir! Got #{x}. Returning an i32 value" |> IO.inspect()
         20 + x
       end, [:i32], [:i32]}
    ]
  })

{:ok, [200]} = Wasmtime.call_func(pid, "run", [180])
```

This next example loads a Wasm module from this [rust lib.rs file](./test/data/wasmapp/src/lib.rs) that's been built with [wasm-pack](https://github.com/rustwasm/wasm-pack):

```
{:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/wasmapp/wasmapp_bg.wasm"})
{:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
{:ok, {[:i32], [:i32]}} = Wasmtime.get_func(pid, "plus_10")
{:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "min")

{:ok, [20]} = Wasmtime.call_func(pid, "add", [11, 9])
{:ok, [30]} = Wasmtime.call_func(pid, "plus_10", [20])
{:ok, [-10]} = Wasmtime.call_func(pid, "min", [-10, 3])
```

If you want to see more usage examples, check [this test file](./test/wasmtime_test.exs) out.

## Benchmark

Wasmtime is really fast. Here's a benchmark with [these simple functions](./test/bench/bench.exs), running on my computer, for a reference:

```
Compiling NIF crate :wasmtime_ex (native/wasmtime_ex)...
    Finished release [optimized] target(s) in 0.07s
Operating System: Linux
CPU Information: Intel(R) Core(TM) i7-4720HQ CPU @ 2.60GHz
Number of Available Cores: 8
Available memory: 31.25 GB
Elixir 1.11.0
Erlang 23.1.1

Benchmark suite executing with the following configuration:
warmup: 20 s
time: 1 min
memory time: 0 ns
parallel: 1
inputs: none specified
Estimated total run time: 6.67 min

Name                 ips        average  deviation         median         99th %
add_xt          140.22 K        7.13 μs   ±461.08%        6.94 μs       11.20 μs
plus_10_xt      134.72 K        7.42 μs   ±443.67%        7.21 μs       11.29 μs
add              21.75 K       45.98 μs    ±52.83%       42.90 μs       83.64 μs
plus_10          19.78 K       50.55 μs    ±47.20%       46.26 μs       91.51 μs
imports           4.28 K      233.76 μs    ±20.80%      230.13 μs      332.01 μs
```

## Supported Wasm types

- Functions are supported with the four value types `i32`, `i64`, `f32` and `f64`
- Memory will be supported soon
- Globals and Tables are will be considered in the future
