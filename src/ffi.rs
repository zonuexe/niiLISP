//! `import` / FFI (ADR-0015/0018/0019).
//!
//! First slice: typed `import` of scalar / string / pointer C functions. The
//! `ForeignFn` type and the `CType` tags are always compiled; the libffi call
//! path, the `import` builtin, and the library cache are gated on the `ffi`
//! feature, so a `--no-default-features` build stays pure, safe Rust.

#[cfg(all(feature = "ffi", unix))]
use std::rc::Rc;

use crate::eval::{Interp, Signal};
use crate::value::Value;

/// A C scalar / pointer type usable in an `import` signature.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(all(feature = "ffi", unix)), allow(dead_code))]
pub enum CType {
    Void,
    Int,
    Long,
    Float,
    Double,
    CharPtr,
    VoidPtr,
}

#[cfg(all(feature = "ffi", unix))]
impl CType {
    fn parse(s: &str) -> Option<CType> {
        Some(match s {
            "void" => CType::Void,
            "int" => CType::Int,
            "long" => CType::Long,
            "float" => CType::Float,
            "double" => CType::Double,
            "char*" => CType::CharPtr,
            "void*" => CType::VoidPtr,
            _ => return None,
        })
    }
}

/// A C function resolved through `import`: its declared signature plus (under the
/// `ffi` feature) the libffi CIF and resolved code pointer.
#[cfg_attr(not(all(feature = "ffi", unix)), allow(dead_code))]
pub struct ForeignFn {
    pub name: String,
    pub arg_types: Vec<CType>,
    pub ret_type: CType,
    #[cfg(all(feature = "ffi", unix))]
    cif: libffi::middle::Cif,
    #[cfg(all(feature = "ffi", unix))]
    code: libffi::middle::CodePtr,
}

impl ForeignFn {
    /// Call the foreign function with already-evaluated arguments.
    #[cfg(all(feature = "ffi", unix))]
    pub fn call(&self, args: &[Value]) -> Result<Value, Signal> {
        use libffi::middle::arg as ffi_arg;
        use std::ffi::{c_void, CStr, CString};
        use std::os::raw::c_char;

        // Backing storage kept alive for the duration of the call.
        enum Scalar {
            I32(i32),
            I64(i64),
            F32(f32),
            F64(f64),
            Ptr(*const c_void),
        }
        let mut cstrings: Vec<CString> = Vec::new();
        let mut scalars: Vec<Scalar> = Vec::with_capacity(self.arg_types.len());

        for (i, ct) in self.arg_types.iter().enumerate() {
            let v = args.get(i).unwrap_or(&Value::Nil);
            let s = match ct {
                CType::Int => Scalar::I32(to_i64(v)? as i32),
                CType::Long => Scalar::I64(to_i64(v)?),
                CType::Float => Scalar::F32(to_f64(v)? as f32),
                CType::Double => Scalar::F64(to_f64(v)?),
                CType::CharPtr => {
                    let bytes = match v {
                        Value::Str(b) => b.clone(),
                        Value::Nil => Vec::new(),
                        _ => return Err(Signal::error("char* argument expects a string")),
                    };
                    let cs = CString::new(bytes)
                        .map_err(|_| Signal::error("char* argument has an interior NUL"))?;
                    // CString owns a heap buffer; its pointer stays valid even if
                    // `cstrings` reallocates (only the CString handle moves).
                    let p = cs.as_ptr() as *const c_void;
                    cstrings.push(cs);
                    Scalar::Ptr(p)
                }
                CType::VoidPtr => Scalar::Ptr(to_i64(v)? as usize as *const c_void),
                CType::Void => return Err(Signal::error("void is not a valid argument type")),
            };
            scalars.push(s);
        }

        // `scalars` is now stable; build Args referencing its entries.
        let ffi_args: Vec<libffi::middle::Arg> = scalars
            .iter()
            .map(|s| match s {
                Scalar::I32(x) => ffi_arg(x),
                Scalar::I64(x) => ffi_arg(x),
                Scalar::F32(x) => ffi_arg(x),
                Scalar::F64(x) => ffi_arg(x),
                Scalar::Ptr(p) => ffi_arg(p),
            })
            .collect();

        // SAFETY: the CIF was built from `arg_types`/`ret_type` and `code` was
        // resolved from a still-loaded library. Calling arbitrary C is inherently
        // unsafe and may crash the interpreter (ADR-0015, accepted).
        let result = unsafe {
            match self.ret_type {
                CType::Void => {
                    self.cif.call::<()>(self.code, &ffi_args);
                    Value::Nil
                }
                CType::Int => Value::Int(self.cif.call::<i32>(self.code, &ffi_args) as i64),
                CType::Long => Value::Int(self.cif.call::<i64>(self.code, &ffi_args)),
                CType::Float => Value::Float(self.cif.call::<f32>(self.code, &ffi_args) as f64),
                CType::Double => Value::Float(self.cif.call::<f64>(self.code, &ffi_args)),
                CType::CharPtr => {
                    let p: *mut c_char = self.cif.call(self.code, &ffi_args);
                    if p.is_null() {
                        Value::Nil
                    } else {
                        Value::Str(CStr::from_ptr(p).to_bytes().to_vec())
                    }
                }
                CType::VoidPtr => {
                    let p: *mut c_void = self.cif.call(self.code, &ffi_args);
                    Value::Int(p as usize as i64)
                }
            }
        };
        Ok(result)
    }

    #[cfg(not(all(feature = "ffi", unix)))]
    pub fn call(&self, _args: &[Value]) -> Result<Value, Signal> {
        Err(Signal::error("FFI is not available in this build"))
    }
}

#[cfg(all(feature = "ffi", unix))]
fn to_i64(v: &Value) -> Result<i64, Signal> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Float(f) => Ok(*f as i64),
        _ => Err(Signal::error("FFI: expected a number")),
    }
}

#[cfg(all(feature = "ffi", unix))]
fn to_f64(v: &Value) -> Result<f64, Signal> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(Signal::error("FFI: expected a number")),
    }
}

#[cfg(all(feature = "ffi", unix))]
fn ctype_to_ffi(ct: CType) -> libffi::middle::Type {
    use libffi::middle::Type;
    match ct {
        CType::Void => Type::void(),
        CType::Int => Type::i32(),
        CType::Long => Type::i64(),
        CType::Float => Type::f32(),
        CType::Double => Type::f64(),
        CType::CharPtr | CType::VoidPtr => Type::pointer(),
    }
}

/// `(import "lib" "fn" "ret-type" "arg-type"...)` — bind a foreign function.
/// Returns the foreign function on success, `nil` if the library or symbol
/// cannot be resolved (the `(if (import …) …)` idiom).
#[cfg(all(feature = "ffi", unix))]
fn b_import(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = match args.first() {
        Some(Value::Str(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => {
            return Err(Signal::error(
                "import: first argument must be a library path",
            ))
        }
    };
    let fname = match args.get(1) {
        Some(Value::Str(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => {
            return Err(Signal::error(
                "import: second argument must be a function name",
            ))
        }
    };
    // A return type string is required (extended/typed import only, ADR-0019).
    let ret_type = match args.get(2) {
        Some(Value::Str(b)) => parse_type(b)?,
        _ => return Ok(Value::Nil),
    };
    let mut arg_types = Vec::new();
    for a in &args[3..] {
        match a {
            Value::Str(b) => arg_types.push(parse_type(b)?),
            _ => return Err(Signal::error("import: argument types must be strings")),
        }
    }

    let code = match interp.ffi_resolve(&path, &fname) {
        Some(c) => c,
        None => return Ok(Value::Nil), // library or symbol not found
    };
    let cif = libffi::middle::Cif::new(
        arg_types.iter().copied().map(ctype_to_ffi),
        ctype_to_ffi(ret_type),
    );
    let ff = Rc::new(ForeignFn {
        name: fname.clone(),
        arg_types,
        ret_type,
        cif,
        code,
    });
    let sym = interp.intern(&fname);
    interp.set_global(sym, Value::Foreign(ff.clone()));
    Ok(Value::Foreign(ff))
}

#[cfg(all(feature = "ffi", unix))]
fn parse_type(b: &[u8]) -> Result<CType, Signal> {
    let s = String::from_utf8_lossy(b);
    CType::parse(&s).ok_or_else(|| Signal::Error(format!("import: unknown FFI type `{}`", s)))
}

/// Register FFI builtins. A no-op in a pure build.
#[cfg(all(feature = "ffi", unix))]
pub fn install(interp: &Interp) {
    interp.register_builtin("import", b_import);
}

#[cfg(not(all(feature = "ffi", unix)))]
pub fn install(_interp: &Interp) {}
