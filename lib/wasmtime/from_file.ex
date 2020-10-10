defmodule Wasmtime.FromFile do
  @moduledoc """
  A struct representing a Wasm Instance from a file. Both .wasm and .wat
  files can be interpreted by wasmtime.
  """

  @enforce_keys [:file_path]
  defstruct file_path: nil, func_imports: []

  @typedoc "An Instance from file"
  @type t() :: %__MODULE__{
          file_path: String.t()
        }
end
