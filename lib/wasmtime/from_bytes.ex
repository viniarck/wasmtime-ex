defmodule Wasmtime.FromBytes do
  @moduledoc """
  A struct representing a Wasm instance from a bytes payload.
  """

  alias Wasmtime.Config, as: Config

  @enforce_keys [:bytes]
  defstruct bytes: nil, func_imports: [], config: %Config{}

  @typedoc """
  Wasmtime.FromBytes
  """
  @type t() :: %__MODULE__{
          bytes: nonempty_charlist(),
          func_imports: list(),
          config: %Config{}
        }
end
