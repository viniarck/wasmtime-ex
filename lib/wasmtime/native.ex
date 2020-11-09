defmodule Wasmtime.Native do
  @moduledoc """
  Documentation for `Wasmtime.Native.
  """
  use Rustler, otp_app: :wasmtime, crate: "wasmtime_ex"

  def load_from_t(_id, _gen_pid, _from_pid, _file_name, _bin, _func_imports),
    do: :erlang.nif_error(:nif_not_loaded)

  def func_call(_id, _gen_pid, _from_pid, _func, _params, _func_imports),
    do: :erlang.nif_error(:nif_not_loaded)

  def call_back_reply(_id, _func_id, _results), do: :erlang.nif_error(:nif_not_loaded)

  def func_exports(_id, _func_imports), do: :erlang.nif_error(:nif_not_loaded)

  def exports(_id, _func_imports), do: :erlang.nif_error(:nif_not_loaded)
end
