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
    {:ok, [10]} = Wasmtime.func_call(pid, "add", [6, 4])
  end

  test "load wat from file" do

    {:ok, pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/adder.wat"})
    {:ok, [{"add", [:i32, :i32], [:i32]}]} = Wasmtime.func_exports(pid)
  end
end
