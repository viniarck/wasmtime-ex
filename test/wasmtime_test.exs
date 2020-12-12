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
    {:ok, [{"add", :func_type}]} = Wasmtime.exports(pid)
    {:ok, [{"add", [:i32, :i32], [:i32]}]} = Wasmtime.func_exports(pid)
    a = 6
    b = 4
    expected = a + b
    {:ok, [^expected]} = Wasmtime.func_call(pid, "add", [a, b])
  end

  test "load wat from file" do
    {:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/adder.wat"})
    {:ok, [{"add", [:i32, :i32], [:i32]}]} = Wasmtime.func_exports(pid)
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
    {:ok, [{"add", :func_type}]} = Wasmtime.exports(pid)
    {:ok, [{"add", [:i64, :i64], [:i64]}]} = Wasmtime.func_exports(pid)
    # TODO // remove inference cache func tys
    # {:ok, [8589934593]} = Wasmtime.func_call(pid, "add", [8589934592, 1])
    {:ok, [17_179_869_184]} = Wasmtime.func_call(pid, "add", [8_589_934_592, 8_589_934_592])
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
    {:ok, [{"add", :func_type}]} = Wasmtime.exports(pid)
    {:ok, [{"add", [:f32, :f32], [:f32]}]} = Wasmtime.func_exports(pid)
    a = 2.1
    b = 1.3
    expected = Float.round(a + b, 5)
    {:ok, [result]} = Wasmtime.func_call(pid, "add", [a, b])
    ^expected = Float.round(result, 5)
  end
end
