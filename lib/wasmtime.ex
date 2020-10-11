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
        Map.put(acc, System.unique_integer([:positive]), {elem(x, 0), elem(x, 1), elem(x, 2)})
      end)
    )
  end

  @impl true
  def handle_call({:func_call, fn_name, params}, _from, payload) do
    {:reply, Native.func_call(payload.id |> Integer.to_string(), fn_name, params), payload}
  end

  defp func_imports_to_term(payload) do
    imps = Map.get(payload, :imports)

    Enum.reduce(Map.keys(imps), [], fn x, acc ->
      [{x, Map.get(imps, x) |> elem(1), Map.get(imps, x) |> elem(2)} | acc]
    end)
    |> Enum.reverse()
  end

  @impl true
  def handle_call({:load}, _from, payload) do
    case payload do
      payload = %FromBytes{} ->
        {:reply,
         Native.load_from(
           payload.id |> Integer.to_string(),
           "",
           payload.bytes |> :binary.bin_to_list(),
           payload |> func_imports_to_term
         ), payload}

      payload = %FromFile{} ->
        {:reply,
         Native.load_from(
           payload.id |> Integer.to_string(),
           payload.file_path,
           [],
           payload |> func_imports_to_term
         ), payload}
    end
  end

  @impl true
  def handle_call({:exports}, _from, payload) do
    {:reply, Native.exports(payload.id |> Integer.to_string()), payload}
  end

  @impl true
  def handle_call({:func_exports}, _from, payload) do
    {:reply, Native.func_exports(payload.id |> Integer.to_string()), payload}
  end

  defp invoke_import(payload, id, params) do
    Map.get(payload, :imports)
    |> Map.get(id)
    |> elem(0)
    |> apply(params)
  end

  defp _load(payload) do
    {:ok, pid} = GenServer.start_link(__MODULE__, payload)

    case GenServer.call(pid, {:load}) do
      :ok -> {:ok, pid}
      {:error, msg} -> {:error, msg}
    end
  end

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

  def exports(pid) when is_pid(pid) do
    GenServer.call(pid, {:exports})
  end

  def func_exports(pid) when is_pid(pid) do
    GenServer.call(pid, {:func_exports})
  end

  @impl true
  def handle_info({:call_back, id, param0}, payload) do
    invoke_import(payload, id, [param0])
    {:noreply, payload}
  end

  @impl true
  def handle_info({:call_back, id, param0, param1}, payload) do
    invoke_import(payload, id, [param0, param1])
    {:noreply, payload}
  end

  @impl true
  def handle_info({:call_back, id, param0, param1, param2}, payload) do
    invoke_import(payload, id, [param0, param1, param2])
    {:noreply, payload}
  end

  @impl true
  def handle_info({:call_back, id, param0, param1, param2, param3}, payload) do
    invoke_import(payload, id, [param0, param1, param2, param3])
    {:noreply, payload}
  end

  @impl true
  def handle_info({:call_back, id, param0, param1, param2, param3, param4}, payload) do
    invoke_import(payload, id, [param0, param1, param2, param3, param4])
    {:noreply, payload}
  end

end
