use std::collections::HashMap;
use wasmtime::{Module, Val};

pub struct Session {
    pub module: Module,
    pub fchs: HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>
}

impl Session {
    pub fn new(
        module: Module,
        fchs: HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>
    ) -> Self {
        Self {
            module,
            fchs,
        }
    }
}

#[derive(Debug)]
pub struct SVal {
    pub v: Val,
}

unsafe impl Send for SVal {}
