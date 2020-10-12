// TODO
// use rustler::schedule::SchedulerFlags;
use rustler::{Encoder, Env, Error, OwnedEnv, Pid, Term};

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use wasmtime::Val;
use wasmtime::*;

mod atoms {
    rustler::rustler_atoms! {
        atom ok;
        atom error;

        atom i32;
        atom i64;
        atom f32;
        atom f64;
        atom v128;
        atom extern_ref;
        atom func_ref;

        atom func_type;
        atom global_type;
        atom table_type;
        atom memory_type;
        atom call_back;
    }
}

// Instance couldn't be bootstraped with lazy_static! for not implementing Send
static mut INSTANCES: Option<Mutex<HashMap<u64, Instance>>> = None;
static mut IMPORTS: Option<Mutex<HashMap<u64, Pid>>> = None;

struct SVal {
    v: Val,
}

unsafe impl Send for SVal {}

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [
        ("load_from", 4, load_from),
        ("exports", 1, exports),
        ("func_call", 3, func_call),
        ("func_exports", 1, func_exports),
    ],
    Some(on_load)
}

fn on_load(_env: Env, _term: Term) -> bool {
    unsafe {
        INSTANCES = Some(Mutex::new(HashMap::new()));
        IMPORTS = Some(Mutex::new(HashMap::new()));
    }
    true
}

fn vec_to_terms<'a>(
    env: Env<'a>,
    values: Vec<Val>,
    func_ty: &[ValType],
) -> Result<Term<'a>, Error> {
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

fn load_from<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: u64 = args[0].decode()?;
    let file_name: &str = args[1].decode()?;
    let bin: Vec<u8> = args[2].decode()?;
    let array: &[u8] = &bin;
    let func_imports: Vec<(u64, Vec<Term>, Vec<Term>)> = args[3].decode()?;
    let mut _func_imports: Vec<Extern> = Vec::new();
    let store = Store::default();

    let module = if bin.len() > 0 {
        match Module::new(store.engine(), array) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        }
    } else {
        match Module::from_file(store.engine(), file_name) {
            Ok(v) => v,
            Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
        }
    };

    for (func_id, func_params, func_results) in func_imports {
        let mut _params: Vec<ValType> = Vec::new();
        let mut _results: Vec<ValType> = Vec::new();

        if func_results.len() > 0 {
            return Ok((
                atoms::error(),
                "func_imports: imported functions shouldn't return any result for now, please use []"
            )
                .encode(env));
        }

        for _param in func_params {
            let value: rustler::Atom = _param.decode()?;
            let atom_value = match value {
                x if x == atoms::i32() => ValType::I32,
                x if x == atoms::i64() => ValType::I64,
                x if x == atoms::f32() => ValType::F32,
                x if x == atoms::f64() => ValType::F32,
                t => {
                    return Ok((
                        atoms::error(),
                        std::format!("ValType not supported yet: {:?}", t),
                    )
                        .encode(env))
                }
            };
            _params.push(atom_value);
        }

        unsafe {
            match IMPORTS {
                Some(ref mut v) => v.lock().unwrap().insert(func_id, env.pid()),
                None => {
                    return Ok((
                        atoms::error(),
                        "IMPORTS didn't initialize properly on_load. Please, file an issue.",
                    )
                        .encode(env))
                }
            }
        };

        let fun: Extern = Func::new(
            &store,
            FuncType::new(_params.into_boxed_slice(), _results.into_boxed_slice()),
            move |_, params, _| {
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
                unsafe {
                    thread::spawn(move || {
                        match IMPORTS {
                            Some(ref mut v) => match v.lock().unwrap().get(&func_id) {
                                Some(pid) => send_and_clear(func_id, pid, &values),
                                None => (),
                            },
                            None => (),
                        };
                    });
                }
                Ok(())
            },
        )
        .into();
        _func_imports.push(fun);
    }
    let instance = match Instance::new(&store, &module, &*_func_imports.into_boxed_slice()) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };
    unsafe {
        match INSTANCES {
            Some(ref mut v) => v
                .lock()
                .unwrap()
                .insert(id, instance),
            None => {
                return Ok((
                    atoms::error(),
                    "INSTANCES didn't initialize properly on_load. Please, file an issue.",
                )
                    .encode(env))
            }
        }
    };
    Ok((atoms::ok()).encode(env))
}

fn func_call<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: u64 = args[0].decode()?;
    let func_name: &str = args[1].decode()?;
    let params: Vec<Term> = args[2].decode()?;

    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(&id) {
                Some(inst) => return call(env, inst, func_name, params),
                None => {
                    return Ok((
                        atoms::error(),
                        "Please, load the module first by calling Wasmtime.load(payload)",
                    )
                        .encode(env))
                }
            },
            None => {
                return Ok((
                    atoms::error(),
                    "INSTANCES didn't initialize properly on_load. Please, file an issue",
                )
                    .encode(env))
            }
        }
    }
}

fn func_exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: u64 = args[0].decode()?;
    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(&id) {
                Some(inst) => {
                    return {
                        let mut _exports: Vec<(&str, Vec<Term>, Vec<Term>)> = Vec::new();
                        for v in inst.exports() {
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
                                            ValType::V128 => {
                                                params.push((atoms::v128()).encode(env))
                                            }
                                            ValType::ExternRef => {
                                                params.push((atoms::extern_ref()).encode(env))
                                            }
                                            ValType::FuncRef => {
                                                params.push((atoms::func_ref()).encode(env))
                                            }
                                        };
                                    }
                                    for v in t.results().iter() {
                                        match v {
                                            ValType::I32 => {
                                                results.push((atoms::i32()).encode(env))
                                            }
                                            ValType::I64 => {
                                                results.push((atoms::i64()).encode(env))
                                            }
                                            ValType::F32 => {
                                                results.push((atoms::f32()).encode(env))
                                            }
                                            ValType::F64 => {
                                                results.push((atoms::f32()).encode(env))
                                            }
                                            t => {
                                                return Ok((
                                                    atoms::error(),
                                                    std::format!(
                                                        "ValType not supported yet: {:?}",
                                                        t
                                                    ),
                                                )
                                                    .encode(env))
                                            }
                                        };
                                    }
                                    _exports.push((v.name(), params, results));
                                }
                                _ => (),
                            };
                        }
                        Ok((atoms::ok(), _exports).encode(env))
                    }
                }
                None => {
                    return Ok((
                        atoms::error(),
                        "Please, load the module first by calling Wasmtime.load(payload)",
                    )
                        .encode(env))
                }
            },
            None => {
                return Ok((
                    atoms::error(),
                    "INSTANCES didn't initialize properly on_load. Please, file an issue.",
                )
                    .encode(env))
            }
        }
    }
}

fn exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: u64 = args[0].decode()?;

    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(&id) {
                Some(inst) => {
                    let mut _exports: Vec<(&str, Term)> = Vec::new();
                    for v in inst.exports() {
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
                    return Ok((atoms::ok(), _exports).encode(env));
                }
                None => {
                    return Ok((
                        atoms::error(),
                        "Please, load the module first by calling Wasmtime.load(payload)",
                    )
                        .encode(env))
                }
            },
            None => {
                return Ok((
                    atoms::error(),
                    "INSTANCES didn't initialize properly on_load. Please, file an issue.",
                )
                    .encode(env))
            }
        }
    }
}

fn call<'a>(
    env: Env<'a>,
    instance: &Instance,
    func_name: &str,
    params: Vec<Term>,
) -> Result<Term<'a>, Error> {
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

fn send_and_clear(func_id: u64, pid: &Pid, values: &Vec<SVal>) {
    let mut msg_env = OwnedEnv::new();
    msg_env.send_and_clear(pid, |env| match values.len() {
        x if x == 0 => (atoms::call_back(), func_id).encode(env),
        x if x == 1 => (
            atoms::call_back(),
            func_id,
            sval_to_term(env, values.get(0).unwrap()),
        )
            .encode(env),
        x if x == 2 => (
            atoms::call_back(),
            func_id,
            sval_to_term(env, values.get(0).unwrap()),
            sval_to_term(env, values.get(1).unwrap()),
        )
            .encode(env),
        x if x == 3 => (
            atoms::call_back(),
            func_id,
            sval_to_term(env, values.get(0).unwrap()),
            sval_to_term(env, values.get(1).unwrap()),
            sval_to_term(env, values.get(2).unwrap()),
        )
            .encode(env),
        x if x == 4 => (
            atoms::call_back(),
            func_id,
            sval_to_term(env, values.get(0).unwrap()),
            sval_to_term(env, values.get(1).unwrap()),
            sval_to_term(env, values.get(2).unwrap()),
            sval_to_term(env, values.get(3).unwrap()),
        )
            .encode(env),
        x if x == 5 => (
            atoms::call_back(),
            func_id,
            sval_to_term(env, values.get(0).unwrap()),
            sval_to_term(env, values.get(1).unwrap()),
            sval_to_term(env, values.get(2).unwrap()),
            sval_to_term(env, values.get(3).unwrap()),
            sval_to_term(env, values.get(4).unwrap()),
        )
            .encode(env),
        // TODO send err
        _ => (atoms::call_back(), func_id).encode(env),
    });
}
