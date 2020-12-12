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

If you were to execute this snippet, you'd see:

```
"Hello from Elixir! Got 180. Returning an i32 value"
```

## Docs

[https://hexdocs.pm/wasmtime](https://hexdocs.pm/wasmtime).
