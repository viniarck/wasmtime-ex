defmodule Wasmtime.Native do
  @moduledoc """
  Wasmtime NIFs. These functions aren't meant to be accessed directly by application
  code. The Wasmtime module expose them through a GenServer.
  """
  use Rustler, otp_app: :wasmtime, crate: "wasmtime_ex"

  def load_from(_id, _gen_pid, _from_pid, _file_name, _bin, _func_ids, _config),
    do: :erlang.nif_error(:nif_not_loaded)

  def func_call(_id, _gen_pid, _from_pid, _func_name, _params, _func_imports),
    do: :erlang.nif_error(:nif_not_loaded)

  def func_call_xt(_id, _func_name, _params), do: :erlang.nif_error(:nif_not_loaded)

  def exfn_reply(_id, _func_id, _results), do: :erlang.nif_error(:nif_not_loaded)

  def get_func(_id, _func_name, _func_imports), do: :erlang.nif_error(:nif_not_loaded)

  def exports(_id, _func_imports), do: :erlang.nif_error(:nif_not_loaded)
end
