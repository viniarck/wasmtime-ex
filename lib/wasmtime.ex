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
    payload = Map.put(payload, :id, System.unique_integer([:positive]))

    Map.put(
      payload,
      :imports,
      Enum.reduce(payload.func_imports, %{}, fn x, acc ->
        Map.put(acc, System.unique_integer([:positive]), {elem(x, 0), elem(x, 1), elem(x, 2)})
      end)
    )
  end

  @impl true
  def handle_call({:func_call, fn_name, params_ty}, from, payload) do
    # TODO switch to no reply... i'll have to pass the gen_pid and from serd...
    {:reply,
     Native.func_call(
       payload.id,
       self(),
       from |> pidref_encode,
       fn_name,
       params_ty,
       payload |> func_imports_to_term
     ), payload}
  end

  defp func_imports_to_term(payload) do
    imps = Map.get(payload, :imports)

    Enum.reduce(Map.keys(imps), [], fn x, acc ->
      [{x, Map.get(imps, x) |> elem(1), Map.get(imps, x) |> elem(2)} | acc]
    end)
    |> Enum.reverse()
  end

  defp pidref_encode(pid_ref) do
    pid_ref |> :erlang.term_to_binary() |> Base.encode64()
  end

  defp pidref_decode(hex) do
    hex |> Base.decode64!() |> :erlang.binary_to_term()
  end

  @impl true
  def handle_call({:load_from_t}, from, payload) do
    # TODO continue here..
    # payload = Map.put(payload, :from, from)
    payload = Map.put(payload, from |> pidref_encode, from)

    case payload do
      payload = %FromBytes{} ->
        Native.load_from_t(
          payload.id,
          self(),
          from |> pidref_encode(),
          "",
          payload.bytes |> :binary.bin_to_list(),
          payload |> func_imports_to_term
        )

      payload = %FromFile{} ->
        Native.load_from_t(
          payload.id,
          self(),
          from |> pidref_encode(),
          payload.file_path,
          [],
          payload |> func_imports_to_term
        )
    end

    {:noreply, payload}
  end

  @impl true
  def handle_call({:load}, _from, payload) do
    case payload do
      payload = %FromBytes{} ->
        {:reply,
         Native.load_from(
           payload.id,
           "",
           payload.bytes |> :binary.bin_to_list(),
           payload |> func_imports_to_term
         ), payload}

      payload = %FromFile{} ->
        {:reply,
         Native.load_from(
           payload.id,
           payload.file_path,
           [],
           payload |> func_imports_to_term
         ), payload}
    end
  end

  @impl true
  def handle_call({:exports}, _from, payload) do
    {:reply, Native.exports(payload.id, payload |> func_imports_to_term), payload}
  end

  @impl true
  def handle_call({:func_exports}, _from, payload) do
    {:reply, Native.func_exports(payload.id, payload |> func_imports_to_term), payload}
  end

  defp invoke_import_res_ty(payload, id, params) do
    func_t =
      Map.get(payload, :imports)
      |> Map.get(id)

    Enum.zip([func_t |> elem(0) |> apply(params)], func_t |> elem(2))
  end

  defp invoke_import(payload, id, params) do
    Map.get(payload, :imports)
    |> Map.get(id)
    |> elem(0)
    |> apply(params)
  end

  defp _load(payload) do
    {:ok, pid} = GenServer.start_link(__MODULE__, payload)

    case GenServer.call(pid, {:load_from_t}) do
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

  def func_call(pid, fn_name, params_ty)
      when is_pid(pid) and is_bitstring(fn_name) and is_list(params_ty) do
    # TODO enhance params_ty with optional, and try to derive first.
    GenServer.call(pid, {:func_call, fn_name, params_ty})
  end

  def exports(pid) when is_pid(pid) do
    GenServer.call(pid, {:exports})
  end

  def func_exports(pid) when is_pid(pid) do
    GenServer.call(pid, {:func_exports})
  end

  @impl true
  def handle_info({:call_back_res, from, results}, payload) do
    IO.inspect("call_back_res")
    IO.inspect(from)
    IO.inspect(results)
    {:noreply, payload}
  end

  @impl true
  def handle_info({:call_back, id, params}, payload) do
    IO.inspect("call_back")
    IO.inspect(id)
    IO.inspect(params)

    Native.call_back_reply(payload.id, id, invoke_import_res_ty(payload, id, params))
    {:noreply, payload}
  end

  @impl true
  def handle_info({:t_ctl, from, msg}, payload) do
    IO.inspect("handling t_ctl #{msg}")
    GenServer.reply(Map.get(payload, from), msg)
    # case msg do
    #   :ok -> IO.inspect("ok baby")
    #   {:error, msg} -> IO.inspect("error #{msg}")
    # end
    {:noreply, payload}
  end
end
