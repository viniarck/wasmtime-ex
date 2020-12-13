defmodule Wasmtime.FromFile do
  @moduledoc """
  A struct representing a Wasm instance from a file. Both `.wasm` and `.wat`
  files can be interpreted by wasmtime.
  """

  alias Wasmtime.Config, as: Config

  @enforce_keys [:file_path]
  defstruct file_path: nil, func_imports: [], config: %Config{}

  @typedoc """
  Wasmtime.FromFile
  """
  @type t() :: %__MODULE__{
          file_path: String.t(),
          func_imports: list(),
          config: %Config{}
        }
end
