use wasmtime::{Module, ValType};

pub struct Session {
    pub module: Module,
    pub fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    pub tch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>),
    pub fch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>),
}

impl Session {
    pub fn new(
        module: Module,
        tch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>),
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
