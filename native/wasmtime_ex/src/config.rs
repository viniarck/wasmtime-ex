use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub debug_info: bool,
    pub interruptable: bool,
    pub max_wasm_stack: usize,
    pub strategy: String,
    pub cranelift_opt_level: String,
}
