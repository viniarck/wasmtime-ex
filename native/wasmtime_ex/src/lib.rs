// TODO
// use rustler::schedule::SchedulerFlags;
use lazy_static::lazy_static;
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

lazy_static! {
    static ref MODULE_ENV: Mutex<HashMap<&'static str, (Module, Engine)>> =
        Mutex::new(HashMap::new());
}

rustler::rustler_export_nifs! {
    "Elixir.Wasmtime.Native",
    [
        ("load_from_file", 2, load_from_file),
        ("load_from_bytes", 2, load_from_bytes),
        ("exports", 1, exports),
        ("func_call", 3, func_call),
        ("func_exports", 1, func_exports),
    ],
    None
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
    MODULE_ENV.lock().unwrap().insert(
        Box::leak(id.clone().to_owned().into_boxed_str()),
        (module.clone(), store.engine().clone()),
    );
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
    MODULE_ENV.lock().unwrap().insert(
        Box::leak(id.clone().to_owned().into_boxed_str()),
        (module.clone(), store.engine().clone()),
    );
    Ok((atoms::ok()).encode(env))
}

fn func_call<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;
    let func_name: &str = args[1].decode()?;
    let params: Vec<Term> = args[2].decode()?;

    match MODULE_ENV.lock().unwrap().get(id) {
        Some((module, engine)) => {
            return {
                let store = &Store::new(engine);
                call(
                    env,
                    func_name,
                    store,
                    module,
                    params,
                    &[],
                )
            };
        }
        None => {
            return Ok((
                atoms::error(),
                "Please, load the module first by calling Wasmtime.load(payload)",
            )
                .encode(env))
        }
    }
}

fn func_exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;

    match MODULE_ENV.lock().unwrap().get(id) {
        Some((module, _)) => {
            return {
                let mut _exports: Vec<(&str, Vec<Term>, Vec<Term>)> = Vec::new();
                for v in module.exports() {
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
                                            std::format!("ValType not supported yet: {:?}", t),
                                        )
                                            .encode(env))
                                    }
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
                    };
                }
                Ok((atoms::ok(), _exports).encode(env))
            };
        }
        None => {
            return Ok((
                atoms::error(),
                "Please, load the module first by calling Wasmtime.load(payload)",
            )
                .encode(env))
        }
    }
}

fn exports<'a>(env: Env<'a>, args: &[Term<'a>]) -> Result<Term<'a>, Error> {
    let id: &str = args[0].decode()?;
    match MODULE_ENV.lock().unwrap().get(id) {
        Some((module, _)) => {
            return {
                let mut _exports: Vec<(&str, Term)> = Vec::new();
                for v in module.exports() {
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
            }
        }
        None => {
            return Ok((
                atoms::error(),
                "Please, load the module first by calling Wasmtime.load(payload)",
            )
                .encode(env))
        }
    }
}

fn call<'a>(
    env: Env<'a>,
    func_name: &str,
    store: &Store,
    module: &Module,
    params: Vec<Term>,
    imports: &[Extern],
) -> Result<Term<'a>, Error> {
    let instance = match Instance::new(store, module, imports) {
        Ok(v) => v,
        Err(e) => return Ok((atoms::error(), e.to_string()).encode(env)),
    };

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
