defmodule Wasmtime.FromBytes do
  @moduledoc """
  A struct representing a Wasm instance from a bytes payload.
  """

  @enforce_keys [:bytes]
  defstruct bytes: nil, func_imports: []

  @typedoc "An Instance from bytes"
  @type t() :: %__MODULE__{
          bytes: nonempty_charlist()
        }
end
