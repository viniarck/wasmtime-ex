pub mod atoms;
pub mod aux;
pub mod config;
pub mod session;

use rustler::schedule::SchedulerFlags;
use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, OwnedEnv, Pid, Term};

use crate::session::{SVal, SValType, Session, SESSIONS};
use crossbeam::channel::unbounded;
use std::collections::HashMap;
use std::error::Error;
use std::thread;
use wasmtime::Val;
use wasmtime::*;

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [

        ("load_from", 7, load_from),
        ("call_func", 6, call_func, SchedulerFlags::DirtyCpu),
        ("call_func_xt", 3, call_func_xt, SchedulerFlags::DirtyCpu),
        ("get_func", 3, get_func),
        ("exfn_reply", 3, exfn_reply, SchedulerFlags::DirtyCpu),
        ("exports", 2, exports),
    ],
    None
}

fn call_func_xt<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let func_name: String = args[1].decode()?;
    let params: Vec<Term> = args[2].decode()?;

    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let store = Store::new(session.module.engine());
        let instance = match Instance::new(&store, &session.module, &[]) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };

        match instance.get_func(&func_name) {
            Some(f) => {
                let mut args: Vec<Val> = Vec::new();
                for (i, v) in f.ty().params().enumerate() {
                    match v {
                        ValType::I32 => args.push(Val::I32(params.get(i).unwrap().decode()?)),
                        ValType::I64 => args.push(Val::I64(params.get(i).unwrap().decode()?)),
                        _ => (),
                    }
                }
                let call_res = match f.call(&args) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok((atoms::error(), e.to_string()).encode(env));
                    }
                };

                let mut results: Vec<Term> = Vec::new();
                for (i, v) in f.ty().results().enumerate() {
                    match v {
                        ValType::I32 => {
                            results.push((call_res.get(i).unwrap().unwrap_i32()).encode(env))
                        }
                        ValType::I64 => {
                            results.push((call_res.get(i).unwrap().unwrap_i64()).encode(env))
                        }
                        ValType::F32 => {
                            results.push((call_res.get(i).unwrap().unwrap_f32()).encode(env))
                        }
                        ValType::F64 => {
                            results.push((call_res.get(i).unwrap().unwrap_f64()).encode(env))
                        }
                        _ => (),
                    };
                }

                return Ok((atoms::ok(), results).encode(env));
            }
            None => {
                return Ok((
                    atoms::error(),
                    std::format!("function {:?} not found", func_name),
                )
                    .encode(env))
            }
        };
    } else {
        Ok((
            atoms::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

fn exfn_reply<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let func_id: i64 = args[1].decode()?;
    let results: Vec<(Term, Atom)> = args[2].decode()?;
    let results = aux::args_to_svals(results)?;

    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        if let Some(fch) = session.fchs.get(&func_id) {
            match fch.0.send(results) {
                Ok(_) => Ok((atoms::ok()).encode(env)),
                Err(_) => Ok((atoms::error(), "exfn_reply failed to send").encode(env)),
            }
        } else {
            Ok((atoms::error(), "exfn_reply failed to get func_id").encode(env))
        }
    } else {
        Ok((
            atoms::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

fn load_from<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let gen_pid: Pid = args[1].decode()?;
    let from_encoded: String = args[2].decode()?;
    let file_name: String = args[3].decode()?;
    let bin: Vec<u8> = args[4].decode()?;
    let func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;
    let config_val: String = args[6].decode()?;

    let config: config::Config = match serde_json::from_str(&config_val) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    let func_imports = match aux::imports_term_to_valtype(&func_imports) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    thread::spawn(move || {
        fn run(
            tid: i64,
            gen_pid: &Pid,
            from_encoded: &String,
            array: &[u8],
            file_name: String,
            func_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
            config: &config::Config,
        ) -> Result<(), Box<dyn Error>> {
            let config = match aux::gen_config(config) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };
            let engine = Engine::new(&config);
            let store = Store::new(&engine);
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

            let store = Store::new(module.engine());
            let func_ids: Vec<i64> = func_imports.iter().map(|x| x.0).collect();
            let func_imports = aux::imports_valtype_to_extern(func_imports, &store);

            let instance = match Instance::new(&store, &module, &*func_imports.into_boxed_slice()) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };

            let mut exports: HashMap<String, Vec<SValType>> = HashMap::new();
            for v in instance.exports() {
                match v.ty() {
                    ExternType::Func(t) => {
                        let mut params: Vec<SValType> = Vec::new();
                        for param in t.params() {
                            params.push(SValType { ty: param.clone() });
                        }
                        exports.insert(v.name().to_string(), params);
                    }
                    _ => (),
                }
            }

            let mut fchs: HashMap<
                i64,
                (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>),
            > = HashMap::with_capacity(func_ids.len());
            for func_id in func_ids {
                let fch: (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>) =
                    unbounded();
                fchs.insert(func_id, fch);
            }

            let session = Box::new(Session::new(module, fchs, exports));
            SESSIONS.write().unwrap().insert(tid, session);

            msg_env.send_and_clear(gen_pid, |env| {
                (atoms::gen_reply(), from_encoded, atoms::ok()).encode(env)
            });
            Ok(())
        }

        match run(
            tid,
            &gen_pid,
            &from_encoded,
            &bin,
            file_name,
            func_imports,
            &config,
        ) {
            Ok(_) => (),
            Err(e) => {
                let mut msg_env = OwnedEnv::new();
                msg_env.send_and_clear(&gen_pid, |env| {
                    (
                        atoms::gen_reply(),
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

fn call_func<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let gen_pid: Pid = args[1].decode()?;
    let from_encoded: String = args[2].decode()?;
    let func_name: String = args[3].decode()?;
    let params: Vec<Term> = args[4].decode()?;
    let func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;

    let (func_imports, tys) =
        match aux::imports_with_exports_tys(tid, func_name.clone(), &func_imports) {
            Ok(v) => v,
            Err(e) => {
                env.send(
                    &gen_pid,
                    (
                        atoms::gen_reply(),
                        from_encoded,
                        (atoms::error(), e.to_string()),
                    )
                        .encode(env),
                );
                return Ok((atoms::ok()).encode(env));
            }
        };
    let svals = aux::args_ty_to_svals(&params, &tys)?;

    thread::spawn(move || {
        fn run(
            tid: i64,
            gen_pid: &Pid,
            from_encoded: &String,
            func_name: String,
            func_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
            svals: Vec<SVal>,
        ) -> Result<(), Box<dyn Error>> {
            if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
                let store = Store::new(session.module.engine());
                let func_imports = aux::imports_valtype_to_extern_recv(
                    func_imports,
                    &store,
                    &session.fchs,
                    &gen_pid.clone(),
                );

                let instance =
                    match Instance::new(&store, &session.module, &*func_imports.into_boxed_slice())
                    {
                        Ok(v) => v,
                        Err(e) => return Err(e.into()),
                    };

                let func = instance.get_func(&func_name).unwrap();
                OwnedEnv::new().send_and_clear(&gen_pid, |env| {
                    let mut params: Vec<Val> = Vec::new();
                    for val in svals {
                        params.push(val.v);
                    }

                    let call_res = match func.call(&params) {
                        Ok(v) => v,
                        Err(e) => {
                            return (atoms::gen_reply(), from_encoded, e.to_string()).encode(env)
                        }
                    };

                    let mut results: Vec<Term> = Vec::new();
                    for (i, v) in func.ty().results().enumerate() {
                        match v {
                            ValType::I32 => {
                                results.push((call_res.get(i).unwrap().unwrap_i32()).encode(env))
                            }
                            ValType::I64 => {
                                results.push((call_res.get(i).unwrap().unwrap_i64()).encode(env))
                            }
                            ValType::F32 => {
                                results.push((call_res.get(i).unwrap().unwrap_f32()).encode(env))
                            }
                            ValType::F64 => {
                                results.push((call_res.get(i).unwrap().unwrap_f64()).encode(env))
                            }
                            _ => (),
                        };
                    }

                    (atoms::gen_reply(), from_encoded, (atoms::ok(), results)).encode(env)
                });
                Ok(())
            } else {
                Ok(())
            }
        }

        match run(
            tid,
            &gen_pid,
            &from_encoded,
            func_name.to_string(),
            func_imports,
            svals,
        ) {
            Ok(_) => (),
            Err(e) => {
                let mut msg_env = OwnedEnv::new();
                msg_env.send_and_clear(&gen_pid, |env| {
                    (
                        atoms::gen_reply(),
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

fn get_func<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let func_name: String = args[1].decode()?;
    let func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)> = args[2].decode()?;

    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let store = Store::new(session.module.engine());
        let func_imports = match aux::imports_term_to_valtype(&func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };
        let func_imports = aux::imports_valtype_to_extern(func_imports, &store);
        let instance =
            match Instance::new(&store, &session.module, &*func_imports.into_boxed_slice()) {
                Ok(v) => v,
                Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
            };
        match instance.get_func(&func_name) {
            Some(f) => {
                let mut params: Vec<Term> = Vec::new();
                let mut results: Vec<Term> = Vec::new();
                for v in f.ty().params() {
                    match v {
                        ValType::I32 => params.push((atoms::i32()).encode(env)),
                        ValType::I64 => params.push((atoms::i64()).encode(env)),
                        ValType::F32 => params.push((atoms::f32()).encode(env)),
                        ValType::F64 => params.push((atoms::f64()).encode(env)),
                        ValType::V128 => params.push((atoms::v128()).encode(env)),
                        ValType::ExternRef => params.push((atoms::extern_ref()).encode(env)),
                        ValType::FuncRef => params.push((atoms::func_ref()).encode(env)),
                    };
                }
                for v in f.ty().results() {
                    match v {
                        ValType::I32 => results.push((atoms::i32()).encode(env)),
                        ValType::I64 => results.push((atoms::i64()).encode(env)),
                        ValType::F32 => results.push((atoms::f32()).encode(env)),
                        ValType::F64 => results.push((atoms::f64()).encode(env)),
                        t => {
                            return Ok((
                                atoms::error(),
                                std::format!("ValType not supported yet: {:?}", t),
                            )
                                .encode(env))
                        }
                    };
                }
                return Ok((atoms::ok(), (params, results)).encode(env));
            }
            None => {
                return Ok((
                    atoms::error(),
                    std::format!("function {:?} not found", func_name),
                )
                    .encode(env))
            }
        };
    } else {
        Ok((
            atoms::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

fn exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)> = args[1].decode()?;

    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let store = Store::new(session.module.engine());
        let func_imports = match aux::imports_term_to_valtype(&func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };
        let func_imports = aux::imports_valtype_to_extern(func_imports, &store);
        let instance =
            match Instance::new(&store, &session.module, &*func_imports.into_boxed_slice()) {
                Ok(v) => v,
                Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
            };

        let mut _exports: Vec<(&str, Term)> = Vec::new();
        for v in instance.exports() {
            match v.ty() {
                ExternType::Func(_) => {
                    _exports.push((v.name(), atoms::func().encode(env)));
                }
                ExternType::Global(_) => {
                    _exports.push((v.name(), atoms::global().encode(env)));
                }
                ExternType::Table(_) => {
                    _exports.push((v.name(), atoms::table().encode(env)));
                }
                ExternType::Memory(_) => {
                    _exports.push((v.name(), atoms::memory().encode(env)));
                }
            };
        }
        Ok((atoms::ok(), _exports).encode(env))
    } else {
        Ok((
            atoms::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}
