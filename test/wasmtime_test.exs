defmodule WasmtimeTest do
  use ExUnit.Case
  doctest Wasmtime

  test "load wat from bytes" do
    mod = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, [{"add", :func}]} = Wasmtime.exports(pid)
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
    a = 6
    b = 4
    expected = a + b
    {:ok, [^expected]} = Wasmtime.call_func(pid, "add", [a, b])
  end

  test "load wat from bytes bad module arity" do
    mod = ~S/
    (module
      (func (export "add") (param i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:error, "WebAssembly failed to compile"} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
  end

  test "load wat from bytes bad formatted" do
    mod = ~S/
    (module
      (func (export "add") param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:error, _} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
  end

  test "load from wat file" do
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/adder.wat"})
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
    {:ok, [20]} = Wasmtime.call_func(pid, "add", [11, 9])
  end

  test "add [:i64, :i64], [:i64]" do
    mod = ~S/
    (module
      (func (export "add") (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, [{"add", :func}]} = Wasmtime.exports(pid)
    {:ok, {[:i64, :i64], [:i64]}} = Wasmtime.get_func(pid, "add")
    {:ok, [8_589_934_593]} = Wasmtime.call_func(pid, "add", [8_589_934_592, 1])
  end

  test "add [:i64, :i64], [:i64] in parallel" do
    mod = ~S/
    (module
      (func (export "add") (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})

    stream =
      Task.async_stream(1..50, fn _ ->
        Wasmtime.call_func(pid, "add", [10, 5])
      end)

    expected_res = 50 * (10 + 5)
    ^expected_res = Enum.reduce(stream, 0, fn {:ok, {:ok, [num]}}, acc -> num + acc end)
  end

  test "load from wasm file" do
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/wasmapp/wasmapp_bg.wasm"})
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
    {:ok, {[:i32], [:i32]}} = Wasmtime.get_func(pid, "plus_10")
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "sum")
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "min")
    {:ok, [20]} = Wasmtime.call_func(pid, "add", [11, 9])
    {:ok, [30]} = Wasmtime.call_func(pid, "plus_10", [20])
    {:ok, [6]} = Wasmtime.call_func(pid, "sum", [0, 3])
    {:ok, [-10]} = Wasmtime.call_func(pid, "min", [-10, 3])
  end

  test "call_func_xt add [:i64, :i64], [:i64]" do
    mod = ~S/
    (module
      (func (export "add") (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, [8_589_934_593]} = Wasmtime.call_func_xt(pid, "add", [8_589_934_592, 1])
  end

  test "add [:f32, :f32], [:f32]" do
    mod = ~S/
    (module
      (func (export "add") (param f32 f32) (result f32)
        local.get 0
        local.get 1
        f32.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, [{"add", :func}]} = Wasmtime.exports(pid)
    {:ok, {[:f32, :f32], [:f32]}} = Wasmtime.get_func(pid, "add")
    a = 2.1
    b = 1.3
    expected = Float.round(a + b, 5)
    {:ok, [result]} = Wasmtime.call_func(pid, "add", [a, b])
    ^expected = Float.round(result, 5)
  end

  test "add [:f64, :f64], [:f64]" do
    mod = ~S/
    (module
      (func (export "add") (param f64 f64) (result f64)
        local.get 0
        local.get 1
        f64.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, [{"add", :func}]} = Wasmtime.exports(pid)
    {:ok, {[:f64, :f64], [:f64]}} = Wasmtime.get_func(pid, "add")
    a = 2.1
    b = 1.3
    expected = Float.round(a + b, 5)
    {:ok, [result]} = Wasmtime.call_func(pid, "add", [a, b])
    ^expected = Float.round(result, 5)
  end

  test "import func [:i32], [:i32]" do
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
             20 + x
           end, [:i32], [:i32]}
        ]
      })

    {:ok, [200]} = Wasmtime.call_func(pid, "run", [180])
  end

  test "multiple import funcs" do
    mod = ~S/
    (module
      (import "" "" (func $fcompute (param f32) (result f32)))
      (import "" "" (func $icompute (param i32) (result i32)))
      (func (export "runfc") (param f32) (result f32) (call $fcompute (local.get 0)))
      (func (export "runic") (param i32) (result i32) (call $icompute (local.get 0)))
    )
    /

    {:ok, pid} =
      Wasmtime.load(%Wasmtime.FromBytes{
        bytes: mod,
        func_imports: [
          {fn x ->
             20.0 + x
           end, [:f32], [:f32]},
          {fn x ->
             10 + x
           end, [:i32], [:i32]}
        ]
      })

    {:ok, [100.0]} = Wasmtime.call_func(pid, "runfc", [80.0])
    {:ok, [32]} = Wasmtime.call_func(pid, "runic", [22])
  end

  test "call_func non existing function" do
    mod = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:error, "function \"non_existing\" not found"} = Wasmtime.call_func(pid, "non_existing", [1])
  end

  test "get_func" do
    mod = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:ok, {[:i32, :i32], [:i32]}} = Wasmtime.get_func(pid, "add")
  end

  test "get_func not found" do
    mod = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})
    {:error, "function \"sub\" not found"} = Wasmtime.get_func(pid, "sub")
  end

  test "Wasmtime.load(payload) must be called first" do
    mod = ~S/
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
    )
    /

    {:ok, pid} = GenServer.start_link(Wasmtime, %Wasmtime.FromBytes{bytes: mod})

    {:error, "Wasmtime.load(payload) hasn't been called yet"} =
      Wasmtime.call_func(pid, "non_existing", [1])
  end

  test "load wat memory type" do
    mod = ~S/
    (module
      (memory (export "memory") 2 3)

      (func (export "size") (result i32) (memory.size))
      (func (export "load") (param i32) (result i32)
        (i32.load8_s (local.get 0))
      )
      (func (export "store") (param i32 i32)
        (i32.store8 (local.get 0) (local.get 1))
      )

      (data (i32.const 0x1000) "\01\02\03\04")
    )
    /
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromBytes{bytes: mod})

    {:ok, [{"memory", :memory}, {"size", :func}, {"load", :func}, {"store", :func}]} =
      Wasmtime.exports(pid)
  end
end
