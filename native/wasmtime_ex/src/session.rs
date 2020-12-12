use std::collections::HashMap;
use wasmtime::{Module, Val, ValType};

pub struct Session {
    pub module: Module,
    pub fchs: HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
    pub exports: HashMap<String, Vec<SValType>>,
}

impl Session {
    pub fn new(
        module: Module,
        fchs: HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
        exports: HashMap<String, Vec<SValType>>,
    ) -> Self {
        Self {
            module,
            fchs,
            exports,
        }
    }
}

#[derive(Debug)]
pub struct SVal {
    pub v: Val,
}

unsafe impl Send for SVal {}

#[derive(Debug)]
pub struct SValType {
    pub ty: ValType,
}

unsafe impl Send for SValType {}
