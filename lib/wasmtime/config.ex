defmodule Wasmtime.Config do
  @moduledoc """
  Struct for configuring Wasmtime options.
  """
  @derive Jason.Encoder
  defstruct debug_info: false,
            interruptable: false,
            max_wasm_stack: Bitwise.<<<(1, 20),
            strategy: :auto,
            cranelift_opt_level: :none

  @typedoc """
  Wasmtime.Config
  """
  @type t() :: %__MODULE__{
          debug_info: boolean(),
          interruptable: boolean(),
          max_wasm_stack: pos_integer(),
          strategy: :auto | :cranelift | :lightbeam,
          cranelift_opt_level: :none | :speed | :speed_and_size
        }
end
