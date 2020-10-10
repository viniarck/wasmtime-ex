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
    {:ok, Map.put(payload, :id, System.unique_integer())}
  end

  @impl true
  def init(payload = %FromFile{}) do
    {:ok, Map.put(payload, :id, System.unique_integer())}
  end

  @impl true
  def handle_call({:func_call, fn_name, params}, _from, payload) do
    {:reply, Native.func_call(payload.id |> Integer.to_string(), fn_name, params), payload}
  end

  @impl true
  def handle_call({:load}, _from, payload) do
    case payload do
      payload = %FromBytes{} ->
        {:reply,
         Native.load_from_bytes(
           payload.id |> Integer.to_string(),
           payload.bytes |> :binary.bin_to_list()
         ), payload}

      payload = %FromFile{} ->
        {:reply,
         Native.load_from_file(
           payload.id |> Integer.to_string(),
           payload.file_path
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

  @impl true
  def handle_info({:call_back, msg}, payload) do
    IO.inspect("call_back #{msg}")

    # TODO parametrize accordingly
    # if length(payload.func_imports) == 1 do
    #   IO.inspect(Enum.at(payload.func_imports, 0).())
    # end

    {:noreply, payload}
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
end
