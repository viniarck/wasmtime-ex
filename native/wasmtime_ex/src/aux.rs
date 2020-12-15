use crate::atoms;
use crate::config;
use crate::session::SESSIONS;

use crate::session::SVal;
use rustler::Error as RustlerError;
use rustler::{Atom, Encoder, Env, OwnedEnv, Pid, Term};
use std::collections::HashMap;
use std::error::Error;
use wasmtime::*;

pub fn imports_valtype_to_extern(
    fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::with_capacity(fn_imports.len());
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

pub fn imports_term_to_valtype(
    func_imports: &Vec<(i64, Vec<Atom>, Vec<Atom>)>
) -> Result<Vec<(i64, Vec<ValType>, Vec<ValType>)>, Box<dyn Error>> {
    let mut fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)> =
        Vec::with_capacity(func_imports.len());
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

pub fn imports_valtype_to_extern_recv(
    fn_imports: Vec<(i64, Vec<ValType>, Vec<ValType>)>,
    store: &Store,
    fchs: &HashMap<i64, (crossbeam::Sender<Vec<SVal>>, crossbeam::Receiver<Vec<SVal>>)>,
    gen_pid: &Pid,
) -> Vec<Extern> {
    let mut _func_imports: Vec<Extern> = Vec::with_capacity(fn_imports.len());
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

pub fn imports_with_exports_tys(
    tid: i64,
    func_name: String,
    func_imports: &Vec<(i64, Vec<Atom>, Vec<Atom>)>,
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

pub fn args_ty_to_svals(
    args: &Vec<Term>,
    tys: &Vec<ValType>,
) -> Result<Vec<SVal>, RustlerError> {
    let mut values: Vec<SVal> = Vec::new();
    for (param, ty) in args.iter().zip(tys) {
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

pub fn args_to_svals(args: Vec<(Term, Atom)>) -> Result<Vec<SVal>, RustlerError> {
    let mut values: Vec<SVal> = Vec::new();
    for (arg, ty) in args.iter() {
        match ty {
            x if *x == atoms::i32() => values.push(SVal {
                v: Val::I32(arg.decode()?),
            }),
            x if *x == atoms::i64() => values.push(SVal {
                v: Val::I64(arg.decode()?),
            }),
            x if *x == atoms::f32() => {
                let v: f32 = arg.decode()?;
                values.push(SVal {
                    v: Val::F32(v.to_bits()),
                })
            }
            x if *x == atoms::f64() => {
                let v: f64 = arg.decode()?;
                values.push(SVal {
                    v: Val::F64(v.to_bits()),
                })
            }
            _ => (),
        };
    }
    Ok(values)
}

pub fn gen_config(config: &config::Config) -> Result<Config, Box<dyn Error>> {
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
