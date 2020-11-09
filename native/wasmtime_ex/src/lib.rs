// TODO
// use rustler::schedule::SchedulerFlags;

pub mod atoms;
pub mod session;

use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, OwnedEnv, Pid, Term};

use crate::session::{SVal, Session, TCmd};
use crossbeam::channel::unbounded;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;
use std::thread;
use wasmtime::Val;
use wasmtime::*;

lazy_static! {
    static ref SESSIONS: Mutex<HashMap<u64, Box<Session>>> = Mutex::new(HashMap::new());
}

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [
        ("load_from_t", 6, load_from_t),
        ("func_call", 6, func_call),
        ("call_back_reply", 3, call_back_reply),
        ("exports", 2, exports),
        ("func_exports", 2, func_exports),
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

fn imports_valtype_to_extern_recv(
    fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
    fchs: &mut HashMap<u64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
    gen_pid: &Pid,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::new();
    for (func_id, func_params, func_results) in fn_imports {
        let fch: (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>) = unbounded();
        fchs.insert(func_id, fch.clone());
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

                let mut msg_env = OwnedEnv::new();
                msg_env.send_and_clear(&pid, |env| {
                    (atoms::call_back(), func_id, sval_vec_to_term(env, values)).encode(env)
                });
                for (i, result) in fch.1.recv().unwrap().iter().enumerate() {
                    _results[i] = result.v.clone();
                }
                Ok(())
            },
        )
        .into();
        _func_imports.push(fun);
    }
    _func_imports
}

fn imports_valtype_to_extern(
    fn_imports: Vec<(u64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::new();
    for (_, func_params, func_results) in fn_imports {
        let fun: Extern = Func::new(
            &store,
            FuncType::new(
                func_params.into_boxed_slice(),
                func_results.into_boxed_slice(),
            ),
            move |_, _, _| Ok(()),
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
    let tid: u64 = args[0].decode()?;
    let func_id: u64 = args[1].decode()?;
    let results: Vec<(Term, Atom)> = args[2].decode()?;
    let results = vec_term_to_sval(results)?;

    if let Some(session) = SESSIONS.lock().unwrap().get(&tid) {
        if let Some(fch) = session.fchs.get(&func_id) {
            fch.0.send(results);
            Ok((atoms::ok()).encode(env))
        } else {
            Ok((atoms::error(), "call_back_reply failed to send").encode(env))
        }
    } else {
        Ok((
            atoms::error(),
            "Wasmtime.load(payload) hasn't been called yet",
        )
            .encode(env))
    }
}
fn load_from_t<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
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
            let mut not_stopped = true;
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

            let tch: (
                crossbeam::Sender<(TCmd, String, Vec<SVal>)>,
                crossbeam::Receiver<(TCmd, String, Vec<SVal>)>,
            ) = unbounded();

            let mut fchs: HashMap<
                u64,
                (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>),
            > = HashMap::new();
            let func_imports =
                imports_valtype_to_extern_recv(fn_imports, &store, &mut fchs, &gen_pid.clone());
            let instance = match Instance::new(&store, &module, &*func_imports.into_boxed_slice()) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };

            let session = Box::new(Session::new(module, tch, fchs));
            let tch_recv = session.tch.1.clone();
            SESSIONS.lock().unwrap().insert(tid, session);

            msg_env.send_and_clear(gen_pid, |env| {
                (atoms::t_ctl(), from_encoded, atoms::ok()).encode(env)
            });

            while not_stopped {
                let val = tch_recv.recv();
                msg_env.send_and_clear(gen_pid, |env| {
                    let val = val.unwrap();
                    match val.0 {
                        TCmd::Call => {
                            let mut params: Vec<Term> = Vec::new();
                            let f_name = val.1;
                            for sval in val.2 {
                                params.push(sval_to_term(env, &sval));
                            }
                            let call_res = match call(env, &instance, &f_name, params) {
                                Ok(v) => v,
                                Err(_) => (atoms::error(), "func_call failed to call").encode(env),
                            };
                            (atoms::call_back_res(), from_encoded, call_res).encode(env)
                        }
                        TCmd::Stop => {
                            not_stopped = true;
                            (atoms::t_ctl(), from_encoded, atoms::ok()).encode(env)
                        }
                    }
                });
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
    let params_ty: Vec<(Term, Atom)> = args[4].decode()?;
    let func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)> = args[5].decode()?;
    let fn_imports = match imports_term_to_valtype(func_imports) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

    let params: Vec<SVal> = vec_term_to_sval(params_ty)?;
    let mut call_args: Vec<SVal> = Vec::new();
    SESSIONS.lock().unwrap().get(&tid).unwrap().tch.0.send((
        TCmd::Call,
        func_name.to_string(),
        params,
    ));

    // TODO genserver :noreply..
    Ok(atoms::ok().encode(env))
}

fn func_exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: u64 = args[0].decode()?;
    let func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)> = args[1].decode()?;

    if let Some(session) = SESSIONS.lock().unwrap().get(&tid) {
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

        let mut _exports: Vec<(&str, Vec<Term>, Vec<Term>)> = Vec::new();
        for v in instance.exports() {
            match v.ty() {
                ExternType::Func(t) => {
                    let mut params: Vec<Term> = Vec::new();
                    let mut results: Vec<Term> = Vec::new();
                    for v in t.params().iter() {
                        match v {
                            ValType::I32 => params.push((atoms::i32()).encode(env)),
                            ValType::I64 => params.push((atoms::i64()).encode(env)),
                            ValType::F32 => params.push((atoms::f32()).encode(env)),
                            ValType::F64 => params.push((atoms::f32()).encode(env)),
                            ValType::V128 => params.push((atoms::v128()).encode(env)),
                            ValType::ExternRef => params.push((atoms::extern_ref()).encode(env)),
                            ValType::FuncRef => params.push((atoms::func_ref()).encode(env)),
                        };
                    }
                    for v in t.results().iter() {
                        match v {
                            ValType::I32 => results.push((atoms::i32()).encode(env)),
                            ValType::I64 => results.push((atoms::i64()).encode(env)),
                            ValType::F32 => results.push((atoms::f32()).encode(env)),
                            ValType::F64 => results.push((atoms::f32()).encode(env)),
                            t => {
                                return Ok((
                                    atoms::error(),
                                    std::format!("ValType not supported yet: {:?}", t),
                                )
                                    .encode(env))
                            }
                        };
                    }
                    _exports.push((v.name(), params, results));
                }
                _ => (),
            }
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

fn exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, RustlerError> {
    let tid: u64 = args[0].decode()?;
    let func_imports: Vec<(u64, Vec<Atom>, Vec<Atom>)> = args[1].decode()?;

    if let Some(session) = SESSIONS.lock().unwrap().get(&tid) {
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

fn vec_term_to_sval(params: Vec<(Term, Atom)>) -> Result<Vec<SVal>, RustlerError> {
    let mut values: Vec<SVal> = Vec::new();
    for (param, ty) in params.iter() {
        match ty {
            x if *x == atoms::i32() => values.push(SVal {
                v: Val::I32(param.decode()?),
            }),
            x if *x == atoms::i64() => values.push(SVal {
                v: Val::I64(param.decode()?),
            }),
            x if *x == atoms::f32() => values.push(SVal {
                v: Val::F32(param.decode()?),
            }),
            x if *x == atoms::f64() => values.push(SVal {
                v: Val::F64(param.decode()?),
            }),
            _ => (),
        };
    }
    Ok(values)
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

fn sval_vec_to_term<'a>(env: Env<'a>, params: Vec<SVal>) -> Term<'a> {
    let mut res: Vec<Term> = Vec::new();
    for param in params {
        match param.v.ty() {
            ValType::I32 => res.push(param.v.unwrap_i32().encode(env)),
            ValType::I64 => res.push(param.v.unwrap_i64().encode(env)),
            ValType::F32 => res.push(param.v.unwrap_f32().encode(env)),
            ValType::F64 => res.push(param.v.unwrap_f64().encode(env)),
            t => res.push(format!("Unsuported type {}", t).encode(env)),
        };
    }
    // TODO should be able to error out.
    res.encode(env)
}
