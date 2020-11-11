use std::collections::HashMap;
use wasmtime::{Module, Val, ValType};

#[derive(Debug)]
pub enum TCmd {
    Call,
    Stop,
}

pub struct Session {
    pub module: Module,
    pub fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    pub tch: (
        crossbeam::Sender<(TCmd, String, String, Vec<SVal>)>,
        crossbeam::Receiver<(TCmd, String, String, Vec<SVal>)>,
    ),
    pub fchs: HashMap<u64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
}

impl Session {
    pub fn new(
        module: Module,
        tch: (
            crossbeam::Sender<(TCmd, String, String, Vec<SVal>)>,
            crossbeam::Receiver<(TCmd, String, String, Vec<SVal>)>,
        ),
        fchs: HashMap<u64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
    ) -> Self {
        Self {
            module,
            tch,
            fchs,
            fn_imports: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct SVal {
    pub v: Val,
}

unsafe impl Send for SVal {}
