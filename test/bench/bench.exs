mod1 = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
{:ok, pid1} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod1})
{:ok, [10]} = Wasmtime.call_func(pid1, "add", [3, 7])

mod2 = ~S/
    (module
      (func (export "plus_10") (param i32) (result i32)
        local.get 0
        i32.const 10
        i32.add)
    )
    /

{:ok, pid2} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod2})
{:ok, [20]} = Wasmtime.call_func(pid2, "plus_10", [10])

mod3 = ~S/
(module
  (import "" "" (func $compute (param i32) (result i32)))
  (func (export "run") (param i32) (result i32) (call $compute (local.get 0)))
)
/

{:ok, pid3} =
  Wasmtime.load(%Wasmtime.FromBytes{
    bytes: mod3,
    func_imports: [
      {fn x ->
         20 + x
       end, [:i32], [:i32]}
    ]
  })

{:ok, [200]} = Wasmtime.call_func(pid3, "run", [180])

Benchee.run(
  %{
    "add" => fn ->
      Wasmtime.call_func(pid1, "add", [20, 40])
    end,
    "add_xt" => fn ->
      Wasmtime.call_func_xt(pid1, "add", [20, 40])
    end,
    "plus_10" => fn ->
      Wasmtime.call_func(pid2, "plus_10", [20])
    end,
    "plus_10_xt" => fn ->
      Wasmtime.call_func_xt(pid2, "plus_10", [20])
    end,
    "imports" => fn ->
      Wasmtime.call_func(pid3, "run", [10])
    end
  },
  warmup: 20,
  time: 60
)
