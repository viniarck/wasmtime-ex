pub mod atom;
pub mod aux;
pub mod config;
pub mod session;

use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, LocalPid, OwnedEnv, Term};

use crate::session::{SVal, SValType, Session, SESSIONS};
use crossbeam::channel::unbounded;
use std::collections::HashMap;
use std::error::Error;
use std::thread;
use wasmtime::Val;
use wasmtime::*;

rustler::init!(
    "Elixir.Wasmtime.Native",
    [
        load_from,
        call_func,
        call_func_xt,
        get_func,
        exfn_reply,
        exports
    ]
);

#[rustler::nif(schedule = "DirtyCpu")]
fn call_func_xt<'a>(
    env: Env<'a>,
    tid: i64,
    func_name: String,
    params: Vec<Term<'a>>,
) -> Result<Term<'a>, RustlerError> {
    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let mut store = Store::new(session.module.engine(), ());
        let instance = match Instance::new(&mut store, &session.module, &[]) {
            Ok(v) => v,
            Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
        };

        match instance.get_func(&mut store, &func_name) {
            Some(f) => {
                let mut args: Vec<Val> = Vec::new();
                for (i, v) in f.ty(&store).params().enumerate() {
                    match v {
                        ValType::I32 => args.push(Val::I32(params.get(i).unwrap().decode()?)),
                        ValType::I64 => args.push(Val::I64(params.get(i).unwrap().decode()?)),
                        _ => (),
                    }
                }
                let mut res: Vec<Val> = Vec::new();
                let func_ty = f.ty(&mut store);
                for result in func_ty.results() {
                  match result {
                      ValType::I32 => {res.push(Val::I32(0));},
                      ValType::I64 => {res.push(Val::I64(0));},
                      ValType::F32 => {res.push(Val::F32(0));},
                      ValType::F64 => {res.push(Val::F64(0));},
                      _ => ()
                  }
                }
                match f.call(&mut store, &args, &mut res) {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok((atom::error(), e.to_string()).encode(env));
                    }
                };

                let mut results: Vec<Term> = Vec::new();
                for (i, v) in f.ty(store).results().enumerate() {
                    match v {
                        ValType::I32 => {
                            results.push((res.get(i).unwrap().unwrap_i32()).encode(env))
                        }
                        ValType::I64 => {
                            results.push((res.get(i).unwrap().unwrap_i64()).encode(env))
                        }
                        ValType::F32 => {
                            results.push((res.get(i).unwrap().unwrap_f32()).encode(env));
                        }
                        ValType::F64 => {
                            results.push((res.get(i).unwrap().unwrap_f64()).encode(env))
                        }
                        _ => (),
                    };
                }

                return Ok((atom::ok(), results).encode(env));
            }
            None => {
                return Ok((
                    atom::error(),
                    std::format!("function {:?} not found", func_name),
                )
                    .encode(env))
            }
        };
    } else {
        Ok((
            atom::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

#[rustler::nif(schedule = "DirtyCpu")]
fn exfn_reply<'a>(
    env: Env<'a>,
    tid: i64,
    func_id: i64,
    results: Vec<(Term, Atom)>,
) -> Result<Term<'a>, RustlerError> {
    let results = aux::args_to_svals(results)?;

    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        if let Some(fch) = session.fchs.get(&func_id) {
            match fch.0.send(results) {
                Ok(_) => Ok((atom::ok()).encode(env)),
                Err(_) => Ok((atom::error(), "exfn_reply failed to send").encode(env)),
            }
        } else {
            Ok((atom::error(), "exfn_reply failed to get func_id").encode(env))
        }
    } else {
        Ok((
            atom::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

#[rustler::nif]
fn load_from<'a>(
    env: Env<'a>,
    tid: i64,
    gen_pid: LocalPid,
    from_encoded: String,
    file_name: String,
    bin: Vec<u8>,
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
    config_val: String,
) -> Result<Term<'a>, RustlerError> {
    let config: config::Config = match serde_json::from_str(&config_val) {
        Ok(v) => v,
        Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
    };

    let func_imports = match aux::imports_term_to_valtype(&func_imports) {
        Ok(v) => v,
        Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
    };

    thread::spawn(move || {
        fn run(
            tid: i64,
            gen_pid: &LocalPid,
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
            let engine = match Engine::new(&config) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };
            let store = Store::new(&engine, ());
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

            let mut store = Store::new(module.engine(), ());
            let func_ids: Vec<i64> = func_imports.iter().map(|x| x.0).collect();
            let func_imports = aux::imports_valtype_to_extern(func_imports, &mut store);

            let instance =
                match Instance::new(&mut store, &module, &*func_imports.into_boxed_slice()) {
                    Ok(v) => v,
                    Err(e) => return Err(e.into()),
                };

            let mut _exports: HashMap<String, Vec<SValType>> = HashMap::new();
            let exported_functions = instance
                .exports(&mut store)
                .map(|e| (e.name().to_owned(), e.into_func()))
                .filter_map(|(n, f)| f.map(|f| (n, f)))
                .collect::<Vec<_>>();
            let exp_funcs = exported_functions
                .into_iter()
                .map(|(n, f)| (n, f.ty(&mut store)))
                .collect::<Vec<(String, FuncType)>>();
            for (name, v) in exp_funcs {
                let mut params: Vec<SValType> = Vec::new();
                for param in v.params() {
                    params.push(SValType { ty: param.clone() });
                };
                _exports.insert(name, params);
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

            let session = Box::new(Session::new(module, fchs, _exports));
            SESSIONS.write().unwrap().insert(tid, session);

            msg_env.send_and_clear(gen_pid, |env| {
                (atom::gen_reply(), from_encoded, atom::ok()).encode(env)
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
                        atom::gen_reply(),
                        from_encoded,
                        (atom::error(), e.to_string()),
                    )
                        .encode(env)
                });
            }
        };
    });
    Ok((atom::ok()).encode(env))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn call_func<'a>(
    env: Env<'a>,
    tid: i64,
    gen_pid: LocalPid,
    from_encoded: String,
    func_name: String,
    params: Vec<Term>,
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
) -> Result<Term<'a>, RustlerError> {
    let (func_imports, tys) =
        match aux::imports_with_exports_tys(tid, func_name.clone(), &func_imports) {
            Ok(v) => v,
            Err(e) => {
                env.send(
                    &gen_pid,
                    (
                        atom::gen_reply(),
                        from_encoded,
                        (atom::error(), e.to_string()),
                    )
                        .encode(env),
                );
                return Ok((atom::ok()).encode(env));
            }
        };
    let svals = aux::args_ty_to_svals(&params, &tys)?;

    thread::spawn(move || {
        fn run(
            tid: i64,
            gen_pid: &LocalPid,
            from_encoded: &String,
            func_name: String,
            func_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
            svals: Vec<SVal>,
        ) -> Result<(), Box<dyn Error>> {
            if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
                let mut store = Store::new(session.module.engine(), ());
                let func_imports = aux::imports_valtype_to_extern_recv(
                    func_imports,
                    &mut store,
                    &session.fchs,
                    &gen_pid.clone(),
                );

                let instance = match Instance::new(
                    &mut store,
                    &session.module,
                    &*func_imports.into_boxed_slice(),
                ) {
                    Ok(v) => v,
                    Err(e) => return Err(e.into()),
                };

                let func = instance.get_func(&mut store, &func_name).unwrap();
                OwnedEnv::new().send_and_clear(&gen_pid, |env| {
                    let mut params: Vec<Val> = Vec::new();
                    for val in svals {
                        params.push(val.v);
                    }
                    let mut res: Vec<Val> = Vec::new();
                    let func_ty = func.ty(&mut store);
                    for result in func_ty.results() {
                      match result {
                          ValType::I32 => {res.push(Val::I32(0));},
                          ValType::I64 => {res.push(Val::I64(0));},
                          ValType::F32 => {res.push(Val::F32(0));},
                          ValType::F64 => {res.push(Val::F64(0));},
                          _ => ()
                      }
                    }
                    match func.call(&mut store, &params, &mut res) {
                        Ok(v) => v,
                        Err(e) => {
                            return (atom::gen_reply(), from_encoded, e.to_string()).encode(env)
                        }
                    };
                    let mut results: Vec<Term> = Vec::new();
                    for (i, v) in func.ty(store).results().enumerate() {
                        match v {
                            ValType::I32 => {
                                results.push((res.get(i).unwrap().unwrap_i32()).encode(env));
                            }
                            ValType::I64 => {
                                results.push((res.get(i).unwrap().unwrap_i64()).encode(env));
                            }
                            ValType::F32 => {
                                results.push((res.get(i).unwrap().unwrap_f32()).encode(env));
                            }
                            ValType::F64 => {
                                results.push((res.get(i).unwrap().unwrap_f64()).encode(env));
                            }
                            _ => (),
                        };
                    }

                    (atom::gen_reply(), from_encoded, (atom::ok(), results)).encode(env)
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
                        atom::gen_reply(),
                        from_encoded,
                        (atom::error(), e.to_string()),
                    )
                        .encode(env)
                });
            }
        };
    });
    Ok((atom::ok()).encode(env))
}

#[rustler::nif]
fn get_func<'a>(
    env: Env<'a>,
    tid: i64,
    func_name: String,
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
) -> Result<Term<'a>, RustlerError> {
    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let mut store = Store::new(session.module.engine(), ());
        let func_imports = match aux::imports_term_to_valtype(&func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
        };
        let func_imports = aux::imports_valtype_to_extern(func_imports, &mut store);
        let instance = match Instance::new(
            &mut store,
            &session.module,
            &*func_imports.into_boxed_slice(),
        ) {
            Ok(v) => v,
            Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
        };
        match instance.get_func(&mut store, &func_name) {
            Some(f) => {
                let mut params: Vec<Term> = Vec::new();
                let mut results: Vec<Term> = Vec::new();
                for v in f.ty(&store).clone().params() {
                    match v {
                        ValType::I32 => params.push((atom::i32()).encode(env)),
                        ValType::I64 => params.push((atom::i64()).encode(env)),
                        ValType::F32 => params.push((atom::f32()).encode(env)),
                        ValType::F64 => params.push((atom::f64()).encode(env)),
                        ValType::V128 => params.push((atom::v128()).encode(env)),
                        ValType::ExternRef => params.push((atom::extern_ref()).encode(env)),
                        ValType::FuncRef => params.push((atom::func_ref()).encode(env)),
                    };
                }
                for v in f.ty(&store).clone().results() {
                    match v {
                        ValType::I32 => results.push((atom::i32()).encode(env)),
                        ValType::I64 => results.push((atom::i64()).encode(env)),
                        ValType::F32 => results.push((atom::f32()).encode(env)),
                        ValType::F64 => results.push((atom::f64()).encode(env)),
                        t => {
                            return Ok((
                                atom::error(),
                                std::format!("ValType not supported yet: {:?}", t),
                            )
                                .encode(env))
                        }
                    };
                }
                return Ok((atom::ok(), (params, results)).encode(env));
            }
            None => {
                return Ok((
                    atom::error(),
                    std::format!("function {:?} not found", func_name),
                )
                    .encode(env))
            }
        };
    } else {
        Ok((
            atom::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}

#[rustler::nif]
fn exports<'a>(
    env: Env<'a>,
    tid: i64,
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
) -> Result<Term<'a>, RustlerError> {
    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        let mut store = Store::new(session.module.engine(), ());
        let func_imports = match aux::imports_term_to_valtype(&func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
        };
        let func_imports = aux::imports_valtype_to_extern(func_imports, &mut store);
        let instance = match Instance::new(
            &mut store,
            &session.module,
            &*func_imports.into_boxed_slice(),
        ) {
            Ok(v) => v,
            Err(e) => return Ok((atom::error(), e.to_string()).encode(env)),
        };

        let mut _exports: Vec<(&str, Term)> = Vec::new();
        for v in instance.exports(&mut store) {
            if let Some(_) = v.clone().into_func() {
                _exports.push((v.name(), atom::func().encode(env)));
                continue;
            }
            if let Some(_) = v.clone().into_global() {
                _exports.push((v.name(), atom::global().encode(env)));
                continue;
            }
            if let Some(_) = v.clone().into_table() {
                _exports.push((v.name(), atom::table().encode(env)));
                continue;
            }
            if let Some(_) = v.clone().into_memory() {
                _exports.push((v.name(), atom::memory().encode(env)));
            }
        }
        Ok((atom::ok(), _exports).encode(env))
    } else {
        Ok((
            atom::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}
