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
        crossbeam::Sender<(TCmd, String, Vec<SVal>)>,
        crossbeam::Receiver<(TCmd, String, Vec<SVal>)>,
    ),
    pub fch: (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>),
}

impl Session {
    pub fn new(
        module: Module,
        tch: (
            crossbeam::Sender<(TCmd, String, Vec<SVal>)>,
            crossbeam::Receiver<(TCmd, String, Vec<SVal>)>,
        ),
        fch: (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>),
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
