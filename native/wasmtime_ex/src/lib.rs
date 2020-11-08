// TODO
// use rustler::schedule::SchedulerFlags;

pub mod atoms;
pub mod session;

use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, OwnedEnv, Pid, Term};

use crossbeam::channel::unbounded;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;
use std::thread;
use wasmtime::Val;
use wasmtime::*;
use crate::session::Session;
use crate::session::SVal;


lazy_static! {
    static ref SESS: Mutex<HashMap<u64, Box<Session>>> = Mutex::new(HashMap::new());
}

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [
        ("load_from_t", 6, load_from_t),
        ("exports", 1, exports),
        ("func_call", 6, func_call),
        ("call_back_reply", 2, call_back_reply),
        ("func_exports", 1, func_exports),
    ],
    None
}

fn imports_term_to_valtype(
    func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)>,
) -> Result<Vec<(u64, Vec<ValType>, Vec<ValType>)>, Box<dyn Error>> {
    let mut fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)> = Vec::new();
    for (f_id, params, results) in func_imports.iter() {
        let mut par: Vec<ValType> = Vec::new();
        let mut res: Vec<ValType> = Vec::new();
        for p in params {
            match p {
                x if *x == atoms::i32() => par.push(ValType::I32),
                x if *x == atoms::i64() => par.push(ValType::I64),
                x if *x == atoms::f32() => par.push(ValType::F32),
                x if *x == atoms::f64() => par.push(ValType::F64),
                x => return Err(std::format!("ValType not supported yet: {:?}", x).into()),
            }
        }
        for r in results {
            match r {
                x if *x == atoms::i32() => res.push(ValType::I32),
                x if *x == atoms::i64() => res.push(ValType::I64),
                x if *x == atoms::f32() => res.push(ValType::F32),
                x if *x == atoms::f64() => res.push(ValType::F64),
                x => return Err(std::format!("ValType not supported yet: {:?}", x).into()),
            }
        }
        fn_imports.push((*f_id, par, res));
    }
    Ok(fn_imports)
}

fn imports_valtype_to_extern(
    fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
    cbch_recv: crossbeam::Receiver<i64>,
    gen_pid: &Pid,
    from_encoded: &String,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::new();
    println!("fn_imports len {:?}", fn_imports.len());
    for (func_id, func_params, func_results) in fn_imports {
        let f = from_encoded.clone();
        // TODO refactor this, should belong to fn_imports
        let cbch_recv = cbch_recv.clone();
        let pid = gen_pid.clone();
        let fun: Extern = Func::new(
            &store,
            FuncType::new(
                func_params.into_boxed_slice(),
                func_results.into_boxed_slice(),
            ),
            move |_, params, _results| {
                let mut values: Vec<SVal> = Vec::new();
                for v in params.iter() {
                    match v {
                        Val::I32(k) => values.push(SVal { v: Val::I32(*k) }),
                        Val::I64(k) => values.push(SVal { v: Val::I64(*k) }),
                        Val::F32(k) => values.push(SVal { v: Val::F32(*k) }),
                        Val::F64(k) => values.push(SVal { v: Val::F64(*k) }),
                        _ => (),
                    }
                }

                // TODO most likely I want one ch per func

                println!("executing...");
                let mut msg_env = OwnedEnv::new();
                // TODO why serd is scrwed up why I vec_to_terms?
                let mut res: Vec<i32> = Vec::new();
                // res.push(3);
                // res.push(4);
                msg_env.send_and_clear(&pid, |env| (atoms::call_back(), func_id, res).encode(env));

                // TODO iterate on them...
                let v = cbch_recv.recv().unwrap();
                println!("cbch_recv {:?}", v);
                _results[0] = (v as i32).into();
                println!("fch done.");
                Ok(())
            },
        )
        .into();
        _func_imports.push(fun);
    }
    _func_imports
}

fn vec_to_terms<'a>(
    env: Env<'a>,
    values: Vec<Val>,
    func_ty: &[ValType],
) -> Result<Term<'a>, RustlerError> {
    let mut results: Vec<Term> = Vec::new();
    for (i, v) in func_ty.iter().enumerate() {
        match v {
            ValType::I32 => results.push((values.get(i).unwrap().unwrap_i32()).encode(env)),
            ValType::I64 => results.push((values.get(i).unwrap().unwrap_i64()).encode(env)),
            ValType::F32 => results.push((values.get(i).unwrap().unwrap_f32()).encode(env)),
            ValType::F64 => results.push((values.get(i).unwrap().unwrap_f64()).encode(env)),
            t => {
                return Ok((
                    atoms::error(),
                    std::format!("ValType not supported yet: {:?}", t),
                )
                    .encode(env))
            }
        };
    }
    Ok((atoms::ok(), results).encode(env))
}

fn call_back_reply<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    // println!("pid1 {:?}", env.pid().as_c_arg());
    let tid: u64 = args[0].decode()?;
    let value: i64 = args[1].decode()?;
    // TODO protect if invalid invoke order..
    // TODO iterate on vector params...

    SESS.lock().unwrap().get(&tid).unwrap().fch.0.send(value);
    Ok((atoms::ok()).encode(env))
}
fn load_from_t<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    // println!("pid1 {:?}", env.pid().as_c_arg());
    let tid: u64 = args[0].decode()?;
    let gen_pid: Pid = args[1].decode()?;
    let from_encoded: String = args[2].decode()?;
    let file_name: String = args[3].decode()?;
    let bin: Vec<u8> = args[4].decode()?;
    let func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;

    let fn_imports = match imports_term_to_valtype(func_imports) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    thread::spawn(move || {
        fn run(
            tid: u64,
            gen_pid: &Pid,
            from_encoded: &String,
            array: &[u8],
            file_name: String,
            fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
        ) -> Result<(), Box<dyn Error>> {
            let store = Store::default();

            let not_stopped = true;
            let mut msg_env = OwnedEnv::new();

            let module = if array.len() > 0 {
                match Module::new(store.engine(), array) {
                    Ok(v) => v,
                    Err(e) => return Err(e.into()),
                }
            } else {
                match Module::from_file(store.engine(), file_name) {
                    Ok(v) => v,
                    Err(e) => return Err(e.into()),
                }
            };
            // let fn_imports2 = fn_imports.clone();

            let tch: (crossbeam::Sender<(String, Vec<SVal>)>, crossbeam::Receiver<(String, Vec<SVal>)>) = unbounded();
            let fch: (crossbeam::Sender<i64>, crossbeam::Receiver<i64>) = unbounded();
            let func_id = fn_imports.get(0).unwrap().0;
            let func_imports = imports_valtype_to_extern(
                fn_imports,
                &store,
                fch.1.clone(),
                &gen_pid.clone(),
                from_encoded,
            );
            let instance = match Instance::new(&store, &module, &*func_imports.into_boxed_slice()) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };

            // let q: MsQueue<Vec<SVal>> = MsQueue::new();
            // q.clone();
            let session = Box::new(Session::new(module, tch, fch));
            let tch_recv = session.tch.1.clone();
            let cbch_send = session.fch.0.clone();
            SESS.lock().unwrap().insert(tid, session);

            msg_env.send_and_clear(gen_pid, |env| {
                (atoms::t_ctl(), from_encoded, atoms::ok()).encode(env)
            });

            // ev = env.clone();

            while not_stopped {
                // println!("tid {:?} waiting...", tid);
                // TODO add another ctl ch
                // std::thread::sleep_ms(1000);
                // let q_in = &SESS.lock().unwrap().get(&tid).unwrap().q_in;
                // let q_in = SESS.lock().unwrap().get(&tid);
                let val = tch_recv.recv();
                println!("got {:?}", val);

                msg_env.send_and_clear(gen_pid, |env| {
                    // TODO continue here hook it up with call..
                    // TODO get func_id from call resp
                    println!("t calling");
                    let mut params: Vec<Term> = Vec::new();
                    let res = val.unwrap();
                    let f_name = res.0;
                    for sval in res.1 {
                       params.push(sval_to_term(env, &sval));
                    }
                    let call_res = match call(env, &instance, &f_name, params) {
                        Ok(v) => v,
                        Err(e) => atoms::error().encode(env),
                    };
                    // TODO handle err
                    // let m = call_res.unwrap();
                    println!("t called");
                    let mut res: Vec<i32> = Vec::new();
                    // res.push(2);
                    // res.push(2);
                    (atoms::call_back_res(), func_id, call_res).encode(env)
                });

                // call(&msg_env.env, &instance, "run", Vec::new());
            }

            Ok(())
        }

        match run(tid, &gen_pid, &from_encoded, &bin, file_name, fn_imports) {
            Ok(_) => (),
            Err(e) => {
                let mut msg_env = OwnedEnv::new();
                msg_env.send_and_clear(&gen_pid, |env| {
                    (
                        atoms::t_ctl(),
                        from_encoded,
                        (atoms::error(), e.to_string()),
                    )
                        .encode(env)
                });
            }
        };
    });
    Ok((atoms::ok()).encode(env))
}

fn func_call<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: u64 = args[0].decode()?;
    let gen_pid: Pid = args[1].decode()?;
    let from_encoded: String = args[2].decode()?;
    let func_name: &str = args[3].decode()?;
    let params: Vec<Term> = args[4].decode()?;
    let func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;
    let fn_imports = match imports_term_to_valtype(func_imports) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    // TODO func call exec command...
    let mut call_args: Vec<SVal> = Vec::new();
    SESS.lock().unwrap().get(&tid).unwrap().tch.0.send(("run".to_string(), call_args));
    // let store = Store::new(SESS.lock().unwrap().get(&tid).unwrap().module.engine());
    // let fn_imports = imports_valtype_to_extern(fn_imports, &store, &gen_pid, &from_encoded);
    // let instance = match Instance::new(
    //     &store,
    //     &SESS.lock().unwrap().get(&tid).unwrap().module,
    //     &*fn_imports.into_boxed_slice(),
    // ) {
    //     Ok(v) => v,
    //     Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    // };

    // call(env, &instance, func_name, params)
    Ok(atoms::ok().encode(env))
}

fn func_exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let id: u64 = args[0].decode()?;
    Ok(atoms::ok().encode(env))
    // unsafe {
    //     match INSTANCES {
    //         Some(ref mut v) => match v.lock().unwrap().get(&id) {
    //             Some(inst) => {
    //                 return {
    //                     let mut _exports: Vec<(&str, Vec<Term>, Vec<Term>)> = Vec::new();
    //                     for v in inst.exports() {
    //                         match v.ty() {
    //                             ExternType::Func(t) => {
    //                                 let mut params: Vec<Term> = Vec::new();
    //                                 let mut results: Vec<Term> = Vec::new();
    //                                 for v in t.params().iter() {
    //                                     match v {
    //                                         ValType::I32 => params.push((atoms::i32()).encode(env)),
    //                                         ValType::I64 => params.push((atoms::i64()).encode(env)),
    //                                         ValType::F32 => params.push((atoms::f32()).encode(env)),
    //                                         ValType::F64 => params.push((atoms::f32()).encode(env)),
    //                                         ValType::V128 => {
    //                                             params.push((atoms::v128()).encode(env))
    //                                         }
    //                                         ValType::ExternRef => {
    //                                             params.push((atoms::extern_ref()).encode(env))
    //                                         }
    //                                         ValType::FuncRef => {
    //                                             params.push((atoms::func_ref()).encode(env))
    //                                         }
    //                                     };
    //                                 }
    //                                 for v in t.results().iter() {
    //                                     match v {
    //                                         ValType::I32 => {
    //                                             results.push((atoms::i32()).encode(env))
    //                                         }
    //                                         ValType::I64 => {
    //                                             results.push((atoms::i64()).encode(env))
    //                                         }
    //                                         ValType::F32 => {
    //                                             results.push((atoms::f32()).encode(env))
    //                                         }
    //                                         ValType::F64 => {
    //                                             results.push((atoms::f32()).encode(env))
    //                                         }
    //                                         t => {
    //                                             return Ok((
    //                                                 atoms::error(),
    //                                                 std::format!(
    //                                                     "ValType not supported yet: {:?}",
    //                                                     t
    //                                                 ),
    //                                             )
    //                                                 .encode(env))
    //                                         }
    //                                     };
    //                                 }
    //                                 _exports.push((v.name(), params, results));
    //                             }
    //                             _ => (),
    //                         };
    //                     }
    //                     Ok((atoms::ok(), _exports).encode(env))
    //                 }
    //             }
    //             None => {
    //                 return Ok((
    //                     atoms::error(),
    //                     "Please, load the module first by calling Wasmtime.load(payload)",
    //                 )
    //                     .encode(env))
    //             }
    //         },
    //         None => {
    //             return Ok((
    //                 atoms::error(),
    //                 "INSTANCES didn't initialize properly on_load. Please, file an issue.",
    //             )
    //                 .encode(env))
    //         }
    //     }
    // }
}

fn exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let id: u64 = args[0].decode()?;
    Ok((atoms::ok()).encode(env))
    // unsafe {
    //     match INSTANCES {
    //         Some(ref mut v) => match v.lock().unwrap().get(&id) {
    //             Some(inst) => {
    //                 let mut _exports: Vec<(&str, Term)> = Vec::new();
    //                 for v in inst.exports() {
    //                     match v.ty() {
    //                         ExternType::Func(_) => {
    //                             _exports.push((v.name(), atoms::func_type().encode(env)));
    //                         }
    //                         ExternType::Global(_) => {
    //                             _exports.push((v.name(), atoms::global_type().encode(env)));
    //                         }
    //                         ExternType::Table(_) => {
    //                             _exports.push((v.name(), atoms::table_type().encode(env)));
    //                         }
    //                         ExternType::Memory(_) => {
    //                             _exports.push((v.name(), atoms::memory_type().encode(env)));
    //                         }
    //                     };
    //                 }
    //                 return Ok((atoms::ok(), _exports).encode(env));
    //             }
    //             None => {
    //                 return Ok((
    //                     atoms::error(),
    //                     "Please, load the module first by calling Wasmtime.load(payload)",
    //                 )
    //                     .encode(env))
    //             }
    //         },
    //         None => {
    //             return Ok((
    //                 atoms::error(),
    //                 "INSTANCES didn't initialize properly on_load. Please, file an issue.",
    //             )
    //                 .encode(env))
    //         }
    //     }
    // }
}

fn call<'a>(
    env: Env<'a>,
    instance: &Instance,
    func_name: &str,
    params: Vec<Term>,
) -> Result<Term<'a>, RustlerError> {
    let func = match instance.get_func(func_name) {
        Some(v) => v,
        None => {
            return Ok((
                atoms::error(),
                format!("failed to find `{}` function export", func_name),
            )
                .encode(env))
        }
    };

    let mut call_args: Vec<Val> = Vec::new();
    for (i, v) in func.ty().params().iter().enumerate() {
        match v {
            ValType::I32 => call_args.push(Val::I32({
                let v: i32 = params[i].decode()?;
                v
            })),
            ValType::I64 => {
                let v: i64 = params[i].decode()?;
                call_args.push(Val::I64(v));
            }
            // # TODO add tests for floats
            ValType::F32 => {
                let v: u32 = params[i].decode()?;
                call_args.push(Val::F32(v));
            }
            ValType::F64 => {
                let v: u64 = params[i].decode()?;
                call_args.push(Val::F64(v));
            }
            t => {
                return Ok((
                    atoms::error(),
                    std::format!("ValType not supported yet: {:?}", t),
                )
                    .encode(env))
            }
        };
    }

    let res = match func.call(&call_args) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    vec_to_terms(env, res.into_vec(), func.ty().results())
}

fn sval_to_term<'a>(env: Env<'a>, send_val: &SVal) -> Term<'a> {
    match send_val.v.ty() {
        ValType::I32 => send_val.v.unwrap_i32().encode(env),
        ValType::I64 => send_val.v.unwrap_i64().encode(env),
        ValType::F32 => send_val.v.unwrap_f32().encode(env),
        ValType::F64 => send_val.v.unwrap_f64().encode(env),
        t => format!("Unsuported type {}", t).encode(env),
    }
}
