<div align="center">
  <h1><code>wasmtime-ex</code></h1>
  <strong>💧Elixir WebAssembly runtime powered by <a href="https://github.com/bytecodealliance/wasmtime">Wasmtime 🦀</a></strong>
  <p></p>
  <a href="https://github.com/viniarck/wasmtime-ex/workflows/.github/workflows/tests.yml/badge.svg"><img src="https://github.com/viniarck/wasmtime-ex/workflows/.github/workflows/tests.yml/badge.svg" alt="tests" /></a>
  <a href="https://img.shields.io/hexpm/v/wasmtime.svg"><img src="https://img.shields.io/hexpm/v/wasmtime.svg" alt="hex.pm version" /></a>
  <a href="https://img.shields.io/hexpm/v/wasmtime.svg"><img src="https://img.shields.io/hexpm/dt/wasmtime.svg" alt="hex.pm downloads" /></a>
</div>

## Installation

You can add `wasmtime` to your list of dependencies in `mix.exs`:

```elixir
def deps do
  [
    {:wasmtime, "~> 0.1.0"}
  ]
end
```

This package is still under heavy development, I'd recommend you to wait for the `0.2.0` release since the core API might still change, feel free to explore it in the meantime. Enjoy!

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

This next example loads a Wasm module from this [wasmapp_bg.wasm file](./test/data/wasmapp) that's been built with [wasm-pack](https://github.com/rustwasm/wasm-pack):

```
{:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/wasmapp/wasmapp_bg.wasm"})
{:ok, {"add", [:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
{:ok, {"plus_10", [:i32], [:i32]}} = Wasmtime.get_func(pid, "plus_10")
{:ok, {"min", [:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "min")

{:ok, [20]} = Wasmtime.call_func(pid, "add", [11, 9])
{:ok, [30]} = Wasmtime.call_func(pid, "plus_10", [20])
{:ok, [-10]} = Wasmtime.call_func(pid, "min", [-10, 3])
```

## Docs

- [https://hexdocs.pm/wasmtime](https://hexdocs.pm/wasmtime)
- If you're looking for more usage snippets, check out the [tests](./test/wasmtime_test.exs)

## Supported Wasm types

- Functions are supported with the four value types `i32`, `i64`, `f32` and `f64`
- Memory will be supported soon
- Globals and Tables are will be considered in the future
