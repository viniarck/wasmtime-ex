![.github/workflows/tests.yml](https://github.com/viniarck/wasmtime-ex/workflows/.github/workflows/tests.yml/badge.svg)[![Coverage Status](https://coveralls.io/repos/github/viniarck/wasmtime-ex/badge.svg?branch=develop)](https://coveralls.io/github/viniarck/wasmtime-ex?branch=develop)

# wasmtime-ex

![logo](https://lh3.googleusercontent.com/pw/ACtC-3cxbQiBP8tra3bdyBk0A1gqo8Ui5rVS-4sVjMdHRRaQxSphTH9FxIuP-O29EV4Vb0aAUvdsXv1gEX6PF5xGOBmCy4YWtt9WBVTS6YOsbeKCOJyU5HZh9kXC7thVEJDZYKN2j_ncTFcp-WvYtuLJQK87=w500-h300-no?authuser=0)

Elixir WebAssembly runtime powered by Wasmtime

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

{:ok, [200]} = Wasmtime.func_call(pid, "run", [180])
```

If you were to execute this code snippet, you'd see this message in the stdout:

```
"Hello from Elixir! Got 180. Returning an i32 value"
```

The following example loads a Wasm module from the [adder.wat file](./test/data/adder.wat) (Wasmtime supports both .wasm and .wat file types). The exported function `add` is called with `[11, 9]`:

```
{:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/adder.wat"})
{:ok, {"add", [:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
{:ok, [20]} = Wasmtime.func_call(pid, "add", [11, 9])
```

## Docs

- [https://hexdocs.pm/wasmtime](https://hexdocs.pm/wasmtime)
- If you're looking for more usage snippets, check out the [tests](./test/test_helper.exs) folder

## Supported Wasm entities

Functions are supported with the four value types `i32`, `i64`, `f32` and `f64`. Memory will be supported soon. Globals and Tables are will be considered in the future.
