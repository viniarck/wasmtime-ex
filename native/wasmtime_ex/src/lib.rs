pub mod atoms;
pub mod session;
pub mod config;

use rustler::schedule::SchedulerFlags;
use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, OwnedEnv, Pid, Term};

use crate::session::{SVal, SValType, Session};
use crossbeam::channel::unbounded;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::sync::RwLock;
use std::thread;
use wasmtime::Val;
use wasmtime::*;

lazy_static! {
    static ref SESSIONS: RwLock<HashMap<i64, Box<Session>>> = RwLock::new(HashMap::new());
}

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

fn imports_term_to_valtype(
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
) -> Result<Vec<(i64, Vec<ValType>, Vec<ValType>)>, Box<dyn Error>> {
    let mut fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)> = Vec::new();
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

fn imports_valtype_to_extern_recv(
    fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
    fchs: &HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
    gen_pid: &Pid,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::new();
    for (func_id, func_params, func_results) in fn_imports {
        match fchs.get(&func_id) {
            Some(fch) => {
                let pid = gen_pid.clone();
                let recv = fch.1.clone();
                let fun: Extern = Func::new(
                    &store,
                    FuncType::new(func_params.into_iter(), func_results.into_iter()),
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
                        let mut msg_env = OwnedEnv::new();
                        msg_env.send_and_clear(&pid, |env| {
                            (atoms::call_exfn(), func_id, sval_vec_to_term(env, values)).encode(env)
                        });
                        for (i, result) in recv.recv().unwrap().iter().enumerate() {
                            _results[i] = result.v.clone();
                        }
                        Ok(())
                    },
                )
                .into();
                _func_imports.push(fun);
                ()
            }
            None => (),
        };
    }
    _func_imports
}

fn imports_valtype_to_extern(
    fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::new();
    for (_, func_params, func_results) in fn_imports {
        let fun: Extern = Func::new(
            &store,
            FuncType::new(func_params.into_iter(), func_results.into_iter()),
            move |_, _, _| Ok(()),
        )
        .into();
        _func_imports.push(fun);
    }
    _func_imports
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
    let results = values_to_sval(results)?;

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

fn gen_config(config: &config::Config) -> Result<Config, Box<dyn Error>>{
    let mut cfg = Config::new();
    cfg.interruptable(config.interruptable);
    cfg.debug_info(config.debug_info);
    cfg.max_wasm_stack(config.max_wasm_stack);
    let strategy = match &config.strategy {
        x if x == "cranelift" => Strategy::Cranelift,
        x if x == "lightbeam" => Strategy::Lightbeam,
        _ => Strategy::Auto,
    };
    let cranelift_opt_level = match &config.cranelift_opt_level {
        x if x == "speed" => OptLevel::Speed,
        x if x == "speed_and_size" => OptLevel::SpeedAndSize,
        _ => OptLevel::None,
    };
    cfg.cranelift_opt_level(cranelift_opt_level);
    match cfg.strategy(strategy) {
        Ok(_) => (),
        Err(e) => return Err(e.into()),
    };
    Ok(cfg)
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

    let fn_imports = match imports_term_to_valtype(func_imports) {
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
            fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
            config: &config::Config,
        ) -> Result<(), Box<dyn Error>> {
            let config = match gen_config(config) {
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
            let func_ids: Vec<i64> = fn_imports.iter().map(|x| x.0).collect();
            let func_imports = imports_valtype_to_extern(fn_imports, &store);

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
            > = HashMap::new();
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

        match run(tid, &gen_pid, &from_encoded, &bin, file_name, fn_imports, &config) {
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

fn func_param_tys(tid: i64, func_name: String) -> Result<Vec<ValType>, Box<dyn Error>> {
    let mut tys: Vec<ValType> = Vec::new();
    if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
        match session.exports.get(&func_name) {
            Some(v) => {
                for val in v.iter() {
                    tys.push(val.ty.clone());
                }
            }
            None => {
                return Err(std::format!("function {:?} not found", func_name).into());
            }
        };
        Ok(tys)
    } else {
        Err("Wasmtime.load(payload) hasn't been called yet".into())
    }
}

fn fn_imports_and_exports_tys(
    tid: i64,
    func_name: String,
    func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)>,
) -> Result<(Vec<(i64, Vec<ValType>, Vec<ValType>)>, Vec<ValType>), Box<dyn Error>> {
    let fn_imports = match imports_term_to_valtype(func_imports) {
        Ok(v) => v,
        Err(e) => {
            return Err(e.to_string().into());
        }
    };
    let tys = match func_param_tys(tid, func_name.clone()) {
        Ok(v) => v,
        Err(e) => {
            return Err(e.to_string().into());
        }
    };
    Ok((fn_imports, tys))
}

fn call_func<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: i64 = args[0].decode()?;
    let gen_pid: Pid = args[1].decode()?;
    let from_encoded: String = args[2].decode()?;
    let func_name: String = args[3].decode()?;
    let params: Vec<Term> = args[4].decode()?;
    let func_imports: Vec<(i64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;

    let (fn_imports, tys) = match fn_imports_and_exports_tys(tid, func_name.clone(), func_imports) {
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
    let svals = params_ty_sval_vec(&params, &tys)?;

    thread::spawn(move || {
        fn run(
            tid: i64,
            gen_pid: &Pid,
            from_encoded: &String,
            func_name: String,
            fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
            svals: Vec<SVal>,
        ) -> Result<(), Box<dyn Error>> {
            if let Some(session) = SESSIONS.read().unwrap().get(&tid) {
                let store = Store::new(session.module.engine());
                let func_imports = imports_valtype_to_extern_recv(
                    fn_imports,
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
            fn_imports,
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
        let fn_imports = match imports_term_to_valtype(func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };
        let fn_imports = imports_valtype_to_extern(fn_imports, &store);
        let instance = match Instance::new(&store, &session.module, &*fn_imports.into_boxed_slice())
        {
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
                return Ok((atoms::ok(), (func_name, params, results)).encode(env));
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
        let fn_imports = match imports_term_to_valtype(func_imports) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };
        let fn_imports = imports_valtype_to_extern(fn_imports, &store);
        let instance = match Instance::new(&store, &session.module, &*fn_imports.into_boxed_slice())
        {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        };

        let mut _exports: Vec<(&str, Term)> = Vec::new();
        for v in instance.exports() {
            match v.ty() {
                ExternType::Func(_) => {
                    _exports.push((v.name(), atoms::func_type().encode(env)));
                }
                ExternType::Global(_) => {
                    _exports.push((v.name(), atoms::global_type().encode(env)));
                }
                ExternType::Table(_) => {
                    _exports.push((v.name(), atoms::table_type().encode(env)));
                }
                ExternType::Memory(_) => {
                    _exports.push((v.name(), atoms::memory_type().encode(env)));
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

fn values_to_sval(params: Vec<(Term, Atom)>) -> Result<Vec<SVal>, RustlerError> {
    let mut values: Vec<SVal> = Vec::new();
    for (param, ty) in params.iter() {
        match ty {
            x if *x == atoms::i32() => values.push(SVal {
                v: Val::I32(param.decode()?),
            }),
            x if *x == atoms::i64() => values.push(SVal {
                v: Val::I64(param.decode()?),
            }),
            x if *x == atoms::f32() => {
                let v: f32 = param.decode()?;
                values.push(SVal {
                    v: Val::F32(v.to_bits()),
                })
            }
            x if *x == atoms::f64() => {
                let v: f64 = param.decode()?;
                values.push(SVal {
                    v: Val::F64(v.to_bits()),
                })
            }
            _ => (),
        };
    }
    Ok(values)
}

fn params_ty_sval_vec(params: &Vec<Term>, tys: &Vec<ValType>) -> Result<Vec<SVal>, RustlerError> {
    let mut values: Vec<SVal> = Vec::new();
    for (param, ty) in params.iter().zip(tys) {
        match ty {
            ValType::I32 => values.push(SVal {
                v: Val::I32(param.decode()?),
            }),
            ValType::I64 => values.push(SVal {
                v: Val::I64(param.decode()?),
            }),
            ValType::F32 => {
                let arg: f32 = param.decode()?;
                values.push(SVal {
                    v: Val::F32(arg.to_bits()),
                })
            }
            ValType::F64 => {
                let arg: f64 = param.decode()?;
                values.push(SVal {
                    v: Val::F64(arg.to_bits()),
                })
            }
            _ => (),
        };
    }
    Ok(values)
}

fn sval_vec_to_term<'a>(env: Env<'a>, params: Vec<SVal>) -> Term<'a> {
    let mut res: Vec<Term> = Vec::new();
    for param in params {
        match param.v.ty() {
            ValType::I32 => res.push(param.v.unwrap_i32().encode(env)),
            ValType::I64 => res.push(param.v.unwrap_i64().encode(env)),
            ValType::F32 => res.push(param.v.unwrap_f32().encode(env)),
            ValType::F64 => res.push(param.v.unwrap_f64().encode(env)),
            _ => (),
        };
    }
    res.encode(env)
}
