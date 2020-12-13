defmodule Wasmtime do
  @moduledoc """
  Documentation for `Wasmtime`.
  """

  use GenServer
  alias Wasmtime.Native
  alias Wasmtime.FromBytes
  alias Wasmtime.FromFile

  @impl true
  def init(payload = %FromBytes{}) do
    {:ok, payload |> init_payload}
  end

  @impl true
  def init(payload = %FromFile{}) do
    {:ok, payload |> init_payload}
  end

  defp init_payload(payload) do
    payload = Map.put(payload, :id, System.unique_integer([:monotonic]))

    Map.put(
      payload,
      :imports,
      Enum.reduce(payload.func_imports, %{}, fn x, acc ->
        Map.put(acc, System.unique_integer([:monotonic]), {elem(x, 0), elem(x, 1), elem(x, 2)})
      end)
    )
  end

  defp pidref_encode(pid_ref) do
    pid_ref |> :erlang.term_to_binary() |> Base.encode64()
  end

  defp func_imports_to_term(payload) do
    imps = Map.get(payload, :imports)

    Enum.reduce(Map.keys(imps) |> Enum.sort(), [], fn x, acc ->
      [{x, Map.get(imps, x) |> elem(1), Map.get(imps, x) |> elem(2)} | acc]
    end)
    |> Enum.reverse()
  end

  @impl true
  def handle_call({:func_call, fn_name, params}, from, payload) do
    payload = Map.put(payload, from |> pidref_encode, from)

    Native.func_call(
      Map.get(payload, :id),
      self(),
      from |> pidref_encode(),
      fn_name,
      params,
      payload |> func_imports_to_term
    )

    {:noreply, payload}
  end

  @impl true
  def handle_call({:func_call_xt, fn_name, params}, _from, payload) do
    {:reply, Native.func_call_xt(Map.get(payload, :id), fn_name, params), payload}
  end

  @impl true
  def handle_call({:load_from}, from, payload) do
    payload = Map.put(payload, from |> pidref_encode, from)

    {:ok, config_encoded} = payload.config |> Jason.encode()

    case payload do
      payload = %FromBytes{} ->
        Native.load_from(
          Map.get(payload, :id),
          self(),
          from |> pidref_encode(),
          "",
          payload.bytes |> :binary.bin_to_list(),
          payload |> func_imports_to_term,
          config_encoded
        )

      payload = %FromFile{} ->
        Native.load_from(
          Map.get(payload, :id),
          self(),
          from |> pidref_encode(),
          payload.file_path,
          [],
          payload |> func_imports_to_term,
          config_encoded
        )
    end

    {:noreply, payload}
  end

  @impl true
  def handle_call({:exports}, _from, payload) do
    {:reply, Native.exports(payload.id, payload |> func_imports_to_term), payload}
  end

  @impl true
  def handle_call({:get_func, fn_name}, _from, payload) do
    {:reply, Native.get_func(payload.id, fn_name, payload |> func_imports_to_term), payload}
  end

  defp invoke_import_res_ty(payload, id, params) do
    func_t =
      Map.get(payload, :imports)
      |> Map.get(id)

    Enum.zip([func_t |> elem(0) |> apply(params)], func_t |> elem(2))
  end

  defp _load(payload) do
    {:ok, pid} = GenServer.start_link(__MODULE__, payload)

    case GenServer.call(pid, {:load_from}) do
      :ok -> {:ok, pid}
      {:error, msg} -> {:error, msg}
    end
  end

  @doc """
  Load a Wasm module given bytes in memory or from a Wasm file. Both `.wasm` and `.wat` files are supported.

  iex> {:ok, _pid} = Wasmtime.load(%Wasmtime.FromFile{file_path: "test/data/adder.wat"})
  """
  @spec load(%FromBytes{} | %FromFile{}) :: {atom(), pid()}
  def load(payload = %FromBytes{}) do
    _load(payload)
  end

  def load(payload = %FromFile{}) do
    _load(payload)
  end

  def func_call(pid, fn_name, params)
      when is_pid(pid) and is_bitstring(fn_name) and is_list(params) do
    GenServer.call(pid, {:func_call, fn_name, params})
  end

  def func_call_xt(pid, fn_name, params)
      when is_pid(pid) and is_bitstring(fn_name) and is_list(params) do
    GenServer.call(pid, {:func_call_xt, fn_name, params})
  end

  def exports(pid) when is_pid(pid) do
    GenServer.call(pid, {:exports})
  end

  def get_func(pid, fn_name) when is_pid(pid) and is_bitstring(fn_name) do
    GenServer.call(pid, {:get_func, fn_name})
  end

  @impl true
  def handle_info({:gen_reply, from, results}, payload) do
    GenServer.reply(Map.get(payload, from), results)
    {:noreply, Map.delete(payload, from)}
  end

  @impl true
  def handle_info({:call_exfn, id, params}, payload) do
    Native.exfn_reply(payload.id, id, invoke_import_res_ty(payload, id, params))
    {:noreply, payload}
  end
end
