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

    /// Size in bytes under the native C ABI. `Void` has no size and is rejected
    /// by struct-layout code before this is called.
    fn size(self) -> usize {
        match self {
            CType::Void => 0,
            CType::Int | CType::Float => 4,
            CType::Long | CType::Double => 8,
            CType::CharPtr | CType::VoidPtr => std::mem::size_of::<usize>(),
        }
    }

    /// Natural alignment in bytes (equal to the size for these scalar/pointer
    /// types on the platforms we support).
    fn align(self) -> usize {
        self.size().max(1)
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
                // A string is passed as a raw pointer to its bytes — no copy,
                // binary-safe, valid for the duration of the call (ADR-0021).
                // `args` outlives the call, so the buffer stays alive. Any other
                // value is treated as an integer address.
                CType::VoidPtr => match v {
                    Value::Str(b) => Scalar::Ptr(b.as_ptr() as *const c_void),
                    _ => Scalar::Ptr(to_i64(v)? as usize as *const c_void),
                },
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

// ---- callback (C -> Lisp, ADR-0020) --------------------------------------

/// Userdata for a callback closure: how to re-enter and what to run.
#[cfg(all(feature = "ffi", unix))]
struct CallbackData {
    interp: *const Interp,
    func: Value,
    arg_types: Vec<CType>,
    ret_type: CType,
}

/// The C-callable trampoline: decode C args, evaluate the niiLISP function with
/// an implicit catch, encode the result. Exceptions never cross the C frames.
#[cfg(all(feature = "ffi", unix))]
unsafe extern "C" fn trampoline(
    _cif: &libffi::low::ffi_cif,
    result: &mut u64,
    args: *const *const std::ffi::c_void,
    data: &CallbackData,
) {
    use std::ffi::{c_void, CStr};
    use std::os::raw::c_char;

    let interp = &*data.interp;
    let mut values = Vec::with_capacity(data.arg_types.len());
    for (i, ct) in data.arg_types.iter().enumerate() {
        let p = *args.add(i);
        let v = match ct {
            CType::Int => Value::Int(*(p as *const i32) as i64),
            CType::Long => Value::Int(*(p as *const i64)),
            CType::Float => Value::Float(*(p as *const f32) as f64),
            CType::Double => Value::Float(*(p as *const f64)),
            CType::CharPtr => {
                let sp = *(p as *const *const c_char);
                if sp.is_null() {
                    Value::Nil
                } else {
                    Value::Str(CStr::from_ptr(sp).to_bytes().to_vec())
                }
            }
            CType::VoidPtr => Value::Int(*(p as *const *const c_void) as usize as i64),
            CType::Void => Value::Nil,
        };
        values.push(v);
    }

    let out = match interp.call(&data.func, values) {
        Ok(v) => v,
        Err(Signal::Error(m)) => {
            eprintln!("callback error: {}", m);
            Value::Nil
        }
        Err(Signal::Throw(_)) => {
            eprintln!("callback: uncaught throw");
            Value::Nil
        }
    };

    let as_i64 = |v: &Value| match v {
        Value::Int(n) => *n,
        Value::Float(f) => *f as i64,
        _ => 0,
    };
    let as_f64 = |v: &Value| match v {
        Value::Int(n) => *n as f64,
        Value::Float(f) => *f,
        _ => 0.0,
    };
    match data.ret_type {
        CType::Void => {}
        CType::Int | CType::Long | CType::VoidPtr | CType::CharPtr => *result = as_i64(&out) as u64,
        CType::Float => *(result as *mut u64 as *mut f32) = as_f64(&out) as f32,
        CType::Double => *(result as *mut u64 as *mut f64) = as_f64(&out),
    }
}

/// `(callback 'func "ret-type" "arg-type"...)` — a C function pointer that calls
/// back into `func`. Returns the code-pointer address as an integer.
#[cfg(all(feature = "ffi", unix))]
fn b_callback(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let target = args
        .first()
        .ok_or_else(|| Signal::error("callback: missing function"))?;
    // A symbol resolves to its function value; a function value is used directly.
    let func = match target {
        Value::Symbol(_) => interp.eval(target)?,
        other => other.clone(),
    };
    let ret_type = match args.get(1) {
        Some(Value::Str(b)) => parse_type(b)?,
        None => CType::Void,
        _ => return Err(Signal::error("callback: return type must be a string")),
    };
    let mut arg_types = Vec::new();
    for a in &args[2..] {
        match a {
            Value::Str(b) => arg_types.push(parse_type(b)?),
            _ => return Err(Signal::error("callback: argument types must be strings")),
        }
    }

    let cif = libffi::middle::Cif::new(
        arg_types.iter().copied().map(ctype_to_ffi),
        ctype_to_ffi(ret_type),
    );
    // Leak the userdata to 'static; the closure is kept for the process lifetime
    // anyway (ADR-0020), so this is not an additional leak.
    let data: &'static CallbackData = Box::leak(Box::new(CallbackData {
        interp: interp as *const Interp,
        func,
        arg_types,
        ret_type,
    }));
    let closure = libffi::middle::Closure::new(cif, trampoline, data);
    let code = *closure.code_ptr() as usize as i64;
    interp.keep_callback(closure);
    Ok(Value::Int(code))
}

// ---- memory API: struct / pack / unpack / get-* / address (ADR-0021) ------

/// Round `off` up to a multiple of `align` (a power of two).
#[cfg(all(feature = "ffi", unix))]
fn align_up(off: usize, align: usize) -> usize {
    (off + align - 1) & !(align - 1)
}

/// Parse a struct layout — a niiLISP list of C type-name strings — into `CType`s,
/// rejecting `void` (a struct field must have a size).
#[cfg(all(feature = "ffi", unix))]
fn layout_types(v: &Value) -> Result<Vec<CType>, Signal> {
    let items = match v {
        Value::List(items) => items,
        _ => return Err(Signal::error("expected a struct (a list of C type names)")),
    };
    let mut types = Vec::with_capacity(items.len());
    for it in items {
        match it {
            Value::Str(b) => {
                let ct = parse_type(b)?;
                if ct == CType::Void {
                    return Err(Signal::error("struct field type cannot be void"));
                }
                types.push(ct);
            }
            _ => {
                return Err(Signal::error(
                    "struct layout must be a list of type strings",
                ))
            }
        }
    }
    Ok(types)
}

/// Native-ABI field offsets and the padded total size for a struct layout.
#[cfg(all(feature = "ffi", unix))]
fn layout_offsets(types: &[CType]) -> (Vec<usize>, usize) {
    let mut off = 0usize;
    let mut max_align = 1usize;
    let mut offsets = Vec::with_capacity(types.len());
    for &ct in types {
        let (size, align) = (ct.size(), ct.align());
        max_align = max_align.max(align);
        off = align_up(off, align);
        offsets.push(off);
        off += size;
    }
    (offsets, align_up(off, max_align))
}

/// `(struct 'name t…)` — bind `name` to the list of C type names `t…`. A struct
/// is just that list (no new value type); `pack`/`unpack` consume it as a layout.
#[cfg(all(feature = "ffi", unix))]
fn b_struct(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let sym = match args.first() {
        Some(Value::Symbol(id)) => *id,
        _ => return Err(Signal::error("struct: first argument must be a symbol")),
    };
    let mut fields = Vec::with_capacity(args.len().saturating_sub(1));
    for a in &args[1..] {
        match a {
            // Validate each name now so a bad type is caught at definition time.
            Value::Str(b) => {
                if parse_type(b)? == CType::Void {
                    return Err(Signal::error("struct field type cannot be void"));
                }
                fields.push(Value::Str(b.clone()));
            }
            _ => return Err(Signal::error("struct: field types must be strings")),
        }
    }
    let layout = Value::List(fields);
    interp.set_global(sym, layout.clone());
    Ok(layout)
}

/// `(pack layout val…)` — serialise `val…` to a binary string laid out as the C
/// struct `layout` (native alignment, padding, byte order). Pointer fields
/// (`char*`/`void*`) take an integer address.
#[cfg(all(feature = "ffi", unix))]
fn b_pack(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let layout = args
        .first()
        .ok_or_else(|| Signal::error("pack: missing layout"))?;
    let types = layout_types(layout)?;
    let vals = &args[1..];
    if vals.len() < types.len() {
        return Err(Signal::error("pack: too few values for the struct layout"));
    }
    let (offsets, total) = layout_offsets(&types);
    let mut buf = vec![0u8; total];
    for (i, &ct) in types.iter().enumerate() {
        let v = &vals[i];
        let off = offsets[i];
        let write =
            |buf: &mut [u8], bytes: &[u8]| buf[off..off + bytes.len()].copy_from_slice(bytes);
        match ct {
            CType::Int => write(&mut buf, &(to_i64(v)? as i32).to_ne_bytes()),
            CType::Long => write(&mut buf, &to_i64(v)?.to_ne_bytes()),
            CType::Float => write(&mut buf, &(to_f64(v)? as f32).to_ne_bytes()),
            CType::Double => write(&mut buf, &to_f64(v)?.to_ne_bytes()),
            CType::CharPtr | CType::VoidPtr => {
                write(&mut buf, &(to_i64(v)? as usize).to_ne_bytes())
            }
            CType::Void => unreachable!("layout_types rejects void"),
        }
    }
    Ok(Value::Str(buf))
}

/// `(unpack layout str)` — the inverse of `pack`: read each field of `layout`
/// from `str` and return the values as a list. A pointer field is dereferenced
/// (`char*` -> the pointed-to C string); a NULL pointer raises an error.
#[cfg(all(feature = "ffi", unix))]
fn b_unpack(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    let layout = args
        .first()
        .ok_or_else(|| Signal::error("unpack: missing layout"))?;
    let types = layout_types(layout)?;
    let bytes = match args.get(1) {
        Some(Value::Str(b)) => b,
        _ => return Err(Signal::error("unpack: second argument must be a string")),
    };
    let (offsets, total) = layout_offsets(&types);
    if bytes.len() < total {
        return Err(Signal::error(
            "unpack: string is shorter than the struct layout",
        ));
    }
    let read = |off: usize, n: usize| -> [u8; 8] {
        let mut b = [0u8; 8];
        b[..n].copy_from_slice(&bytes[off..off + n]);
        b
    };
    let mut out = Vec::with_capacity(types.len());
    for (i, &ct) in types.iter().enumerate() {
        let off = offsets[i];
        let v = match ct {
            CType::Int => {
                Value::Int(i32::from_ne_bytes(read(off, 4)[..4].try_into().unwrap()) as i64)
            }
            CType::Long => Value::Int(i64::from_ne_bytes(read(off, 8))),
            CType::Float => {
                Value::Float(f32::from_ne_bytes(read(off, 4)[..4].try_into().unwrap()) as f64)
            }
            CType::Double => Value::Float(f64::from_ne_bytes(read(off, 8))),
            CType::VoidPtr => Value::Int(usize::from_ne_bytes(read(off, 8)) as i64),
            CType::CharPtr => {
                let addr = usize::from_ne_bytes(read(off, 8));
                if addr == 0 {
                    return Err(Signal::error("cannot convert NULL to string"));
                }
                // SAFETY: caller-supplied address; a non-NULL invalid pointer is
                // UB, the caller's risk (ADR-0015). NULL is handled above.
                let s = unsafe { CStr::from_ptr(addr as *const c_char).to_bytes().to_vec() };
                Value::Str(s)
            }
            CType::Void => unreachable!("layout_types rejects void"),
        };
        out.push(v);
    }
    Ok(Value::List(out))
}

/// The integer address argument to a `get-*` builtin, rejecting NULL (0).
#[cfg(all(feature = "ffi", unix))]
fn get_addr(args: &[Value], what: &str) -> Result<usize, Signal> {
    let addr = to_i64(args.first().unwrap_or(&Value::Nil))? as usize;
    if addr == 0 {
        return Err(Signal::error(if what == "string" {
            "cannot convert NULL to string".to_string()
        } else {
            format!("get-{}: cannot read from NULL address", what)
        }));
    }
    Ok(addr)
}

/// `(get-string addr [len [limit]])` — read a C string at `addr`. Without `len`,
/// reads a NUL-terminated string; with `len`, reads up to `len` bytes, truncated
/// at the first occurrence of `limit` if given. NULL (0) raises an error.
#[cfg(all(feature = "ffi", unix))]
fn b_get_string(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    let addr = get_addr(args, "string")?;
    let bytes = match args.get(1) {
        // SAFETY: caller-supplied address; NULL handled above, other invalid
        // addresses are UB (ADR-0015).
        None => unsafe { CStr::from_ptr(addr as *const c_char).to_bytes().to_vec() },
        Some(len_v) => {
            let len = to_i64(len_v)?.max(0) as usize;
            let slice = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
            match args.get(2) {
                Some(Value::Str(limit)) if !limit.is_empty() => {
                    let end = slice
                        .windows(limit.len())
                        .position(|w| w == limit.as_slice())
                        .unwrap_or(slice.len());
                    slice[..end].to_vec()
                }
                _ => slice.to_vec(),
            }
        }
    };
    Ok(Value::Str(bytes))
}

/// Read a fixed-width C scalar at an integer address (NULL rejected).
#[cfg(all(feature = "ffi", unix))]
fn b_get_int(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let addr = get_addr(args, "int")?;
    // SAFETY: caller-supplied address; NULL handled, other invalid is UB.
    Ok(Value::Int(
        unsafe { (addr as *const i32).read_unaligned() } as i64
    ))
}

#[cfg(all(feature = "ffi", unix))]
fn b_get_long(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let addr = get_addr(args, "long")?;
    Ok(Value::Int(unsafe { (addr as *const i64).read_unaligned() }))
}

#[cfg(all(feature = "ffi", unix))]
fn b_get_float(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let addr = get_addr(args, "float")?;
    // newLISP `get-float` reads a 64-bit C double.
    Ok(Value::Float(unsafe {
        (addr as *const f64).read_unaligned()
    }))
}

#[cfg(all(feature = "ffi", unix))]
fn b_get_char(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let addr = get_addr(args, "char")?;
    Ok(Value::Int(
        unsafe { (addr as *const i8).read_unaligned() } as i64
    ))
}

/// `(address 'sym)` — the stable buffer address of the string held by `sym`.
/// Only a symbol-held string qualifies: a temporary's copy is dropped at once
/// under ORO, so its address would dangle (ADR-0021). Caller must not resize or
/// reassign `sym` while C holds the address.
#[cfg(all(feature = "ffi", unix))]
fn b_address(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let sym = match args.first() {
        Some(Value::Symbol(id)) => *id,
        _ => {
            return Err(Signal::error(
                "address: argument must be a symbol (a temporary's address would dangle)",
            ))
        }
    };
    interp.with_global(sym, |v| match v {
        Some(Value::Str(b)) => Ok(Value::Int(b.as_ptr() as usize as i64)),
        _ => Err(Signal::error(
            "address: symbol does not hold a string buffer",
        )),
    })
}

/// Register FFI builtins. A no-op in a pure build.
#[cfg(all(feature = "ffi", unix))]
pub fn install(interp: &Interp) {
    interp.register_builtin("import", b_import);
    interp.register_builtin("callback", b_callback);
    interp.register_builtin("struct", b_struct);
    interp.register_builtin("pack", b_pack);
    interp.register_builtin("unpack", b_unpack);
    interp.register_builtin("get-string", b_get_string);
    interp.register_builtin("get-int", b_get_int);
    interp.register_builtin("get-long", b_get_long);
    interp.register_builtin("get-float", b_get_float);
    interp.register_builtin("get-char", b_get_char);
    interp.register_builtin("address", b_address);
}

#[cfg(not(all(feature = "ffi", unix)))]
pub fn install(_interp: &Interp) {}
