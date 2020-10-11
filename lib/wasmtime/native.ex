defmodule Wasmtime.Native do
  @moduledoc """
  Documentation for `Wasmtime.Native.
  """
  use Rustler, otp_app: :wasmtime, crate: "wasmtime_ex"

  def exports(_id), do: :erlang.nif_error(:nif_not_loaded)
  def func_call(_id, _func, _params), do: :erlang.nif_error(:nif_not_loaded)
  def func_exports(_id), do: :erlang.nif_error(:nif_not_loaded)

  def load_from(_id, _file_name, _bin, _func_imports),
    do: :erlang.nif_error(:nif_not_loaded)
end
