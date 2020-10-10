// TODO
// use rustler::schedule::SchedulerFlags;
use rustler::{Encoder, Env, Error, Term};
use std::collections::HashMap;
use std::sync::Mutex;
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
        atom func_type;
        atom global_type;
        atom table_type;
        atom memory_type;
        atom call_back;
    }
}

// Instance couldn't be bootstraped with lazy_static! for not implementing Send
static mut INSTANCES: Option<Mutex<HashMap<&'static str, Instance>>> = None;

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [
        ("load_from_file", 2, load_from_file),
        ("load_from_bytes", 2, load_from_bytes),
        ("exports", 1, exports),
        ("func_call", 3, func_call),
        ("func_exports", 1, func_exports),
    ],
    Some(on_load)
}

fn on_load(_env: Env, _term: Term) -> bool {
    unsafe {
        INSTANCES = Some(Mutex::new(HashMap::new()));
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

fn load_from_file<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;
    let file_name: &str = args[1].decode()?;
    let store = Store::default();

    let module = match Module::from_file(store.engine(), file_name) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };
    let instance = match Instance::new(&store, &module, &[]) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };
    unsafe {
        match INSTANCES {
            Some(ref mut v) => v
                .lock()
                .unwrap()
                .insert(Box::leak(id.clone().to_owned().into_boxed_str()), instance),
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

fn load_from_bytes<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;
    let bin: Vec<u8> = args[1].decode()?;
    let array: &[u8] = &bin;
    let store = Store::default();
    let module = match Module::from_binary(store.engine(), array) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };
    let instance = match Instance::new(&store, &module, &[]) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };
    unsafe {
        match INSTANCES {
            Some(ref mut v) => v
                .lock()
                .unwrap()
                .insert(Box::leak(id.clone().to_owned().into_boxed_str()), instance),
            None => {
                return Ok((
                    atoms::error(),
                    "INSTANCES didn't initialized properly on_load. Please, file an issue.",
                )
                    .encode(env))
            }
        }
    };
    Ok((atoms::ok()).encode(env))
}

fn func_call<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;
    let func_name: &str = args[1].decode()?;
    let params: Vec<Term> = args[2].decode()?;

    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(id) {
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
    let id: &str = args[0].decode()?;
    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(id) {
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
    let id: &str = args[0].decode()?;

    unsafe {
        match INSTANCES {
            Some(ref mut v) => match v.lock().unwrap().get(id) {
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
