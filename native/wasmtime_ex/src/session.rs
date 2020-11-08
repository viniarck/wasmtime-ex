use rustler::Term;
use wasmtime::{Module, Val, ValType};

pub struct Session {
    pub module: Module,
    pub fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    pub tch: (
        crossbeam::Sender<(String, Vec<SVal>)>,
        crossbeam::Receiver<(String, Vec<SVal>)>,
    ),
    pub fch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>),
}

impl Session {
    pub fn new(
        module: Module,
        tch: (
            crossbeam::Sender<(String, Vec<SVal>)>,
            crossbeam::Receiver<(String, Vec<SVal>)>,
        ),
        fch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>),
    ) -> Self {
        Self {
            module,
            tch,
            fch,
            fn_imports: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct SVal {
    pub v: Val,
}

unsafe impl Send for SVal {}
