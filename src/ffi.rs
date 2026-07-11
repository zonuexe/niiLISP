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
                        Value::Str(b) => b.to_vec(),
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
                        Value::str(CStr::from_ptr(p).to_bytes().to_vec())
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
                    Value::str(CStr::from_ptr(sp).to_bytes().to_vec())
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
        // A callback body cannot unwind past the C caller, so `(exit)` / the
        // step limit collapse to nil here (ADR-0040).
        Err(Signal::Exit(_)) => {
            eprintln!("callback: exit");
            Value::Nil
        }
        Err(Signal::Limit) => {
            eprintln!("callback: eval-step limit exceeded");
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
    for it in items.iter() {
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
    let layout = Value::list(fields);
    interp.set_global(sym, layout.clone());
    Ok(layout)
}

// ---- the terse `pack` format-char mini-language --------------------------
//
// A format string is a sequence of specifiers, whitespace-separated for
// readability, tightly packed with **no alignment or padding** (unlike a
// struct). Specifiers (following newLISP):
//   c  signed 8-bit   b  unsigned 8-bit
//   d  signed 16-bit  u  unsigned 16-bit
//   ld signed 32-bit  lu unsigned 32-bit
//   Ld signed 64-bit  Lu unsigned 64-bit
//   f  32-bit float   lf 64-bit double
//   sN a string of N bytes (null-padded)   nN  N null bytes (no value)
//   >  big endian for following fields      <   little endian for following

/// Byte order for a numeric field.
#[cfg(all(feature = "ffi", unix))]
#[derive(Clone, Copy)]
enum Endian {
    Native,
    Little,
    Big,
}

/// One parsed format specifier.
#[cfg(all(feature = "ffi", unix))]
enum Field {
    Int {
        bytes: usize,
        signed: bool,
        endian: Endian,
    },
    F32(Endian),
    F64(Endian),
    /// A fixed-width string of `n` bytes (null-padded on pack).
    Str(usize),
    /// `n` null bytes that consume no value.
    Pad(usize),
}

/// Read an optional decimal count (defaulting to 1) for `sN` / `nN`.
#[cfg(all(feature = "ffi", unix))]
fn read_count(fmt: &[u8], i: &mut usize) -> usize {
    let mut n = 0usize;
    let mut any = false;
    while *i < fmt.len() && fmt[*i].is_ascii_digit() {
        n = n * 10 + usize::from(fmt[*i] - b'0');
        *i += 1;
        any = true;
    }
    if any {
        n
    } else {
        1
    }
}

/// Parse a format string into a sequence of fields.
#[cfg(all(feature = "ffi", unix))]
fn parse_format(fmt: &[u8]) -> Result<Vec<Field>, Signal> {
    let mut fields = Vec::new();
    let mut endian = Endian::Native;
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => {}
            b'>' => endian = Endian::Big,
            b'<' => endian = Endian::Little,
            b'c' => fields.push(Field::Int {
                bytes: 1,
                signed: true,
                endian,
            }),
            b'b' => fields.push(Field::Int {
                bytes: 1,
                signed: false,
                endian,
            }),
            b'd' => fields.push(Field::Int {
                bytes: 2,
                signed: true,
                endian,
            }),
            b'u' => fields.push(Field::Int {
                bytes: 2,
                signed: false,
                endian,
            }),
            b'f' => fields.push(Field::F32(endian)),
            b'l' => {
                let next = fmt.get(i).copied();
                i += 1;
                match next {
                    Some(b'd') => fields.push(Field::Int {
                        bytes: 4,
                        signed: true,
                        endian,
                    }),
                    Some(b'u') => fields.push(Field::Int {
                        bytes: 4,
                        signed: false,
                        endian,
                    }),
                    Some(b'f') => fields.push(Field::F64(endian)),
                    _ => return Err(Signal::error("pack: `l` must be followed by d, u, or f")),
                }
            }
            b'L' => {
                let next = fmt.get(i).copied();
                i += 1;
                match next {
                    Some(b'd') => fields.push(Field::Int {
                        bytes: 8,
                        signed: true,
                        endian,
                    }),
                    Some(b'u') => fields.push(Field::Int {
                        bytes: 8,
                        signed: false,
                        endian,
                    }),
                    _ => return Err(Signal::error("pack: `L` must be followed by d or u")),
                }
            }
            b's' => fields.push(Field::Str(read_count(fmt, &mut i))),
            b'n' => fields.push(Field::Pad(read_count(fmt, &mut i))),
            other => {
                return Err(Signal::Error(format!(
                    "pack: unknown format char `{}`",
                    other as char
                )))
            }
        }
    }
    Ok(fields)
}

/// Reorder little-endian `le` bytes to (or from — the op is its own inverse) the
/// wire order for `endian`. `Native` follows the host's byte order.
#[cfg(all(feature = "ffi", unix))]
fn order(le: &[u8], endian: Endian) -> Vec<u8> {
    let reverse = match endian {
        Endian::Little => false,
        Endian::Big => true,
        Endian::Native => cfg!(target_endian = "big"),
    };
    let mut v = le.to_vec();
    if reverse {
        v.reverse();
    }
    v
}

/// `pack` for the format-string path: tightly packed, no alignment.
#[cfg(all(feature = "ffi", unix))]
fn pack_format(fmt: &[u8], vals: &[Value]) -> Result<Value, Signal> {
    let fields = parse_format(fmt)?;
    let mut out = Vec::new();
    let mut vi = 0;
    let too_few = || Signal::error("pack: too few values for the format");
    for f in &fields {
        match f {
            Field::Int { bytes, endian, .. } => {
                let v = vals.get(vi).ok_or_else(too_few)?;
                vi += 1;
                let le = (to_i64(v)? as u64).to_le_bytes();
                out.extend_from_slice(&order(&le[..*bytes], *endian));
            }
            Field::F32(endian) => {
                let v = vals.get(vi).ok_or_else(too_few)?;
                vi += 1;
                let le = (to_f64(v)? as f32).to_bits().to_le_bytes();
                out.extend_from_slice(&order(&le, *endian));
            }
            Field::F64(endian) => {
                let v = vals.get(vi).ok_or_else(too_few)?;
                vi += 1;
                let le = to_f64(v)?.to_bits().to_le_bytes();
                out.extend_from_slice(&order(&le, *endian));
            }
            Field::Str(n) => {
                let v = vals.get(vi).ok_or_else(too_few)?;
                vi += 1;
                let bytes = match v {
                    Value::Str(b) => b,
                    _ => return Err(Signal::error("pack: `s` field expects a string")),
                };
                let start = out.len();
                out.resize(start + n, 0);
                let m = bytes.len().min(*n);
                out[start..start + m].copy_from_slice(&bytes[..m]);
            }
            Field::Pad(n) => out.resize(out.len() + n, 0),
        }
    }
    Ok(Value::str(out))
}

/// `unpack` for the format-string path.
#[cfg(all(feature = "ffi", unix))]
fn unpack_format(fmt: &[u8], data: &[u8]) -> Result<Value, Signal> {
    let fields = parse_format(fmt)?;
    let mut out = Vec::new();
    let mut off = 0usize;
    let take = |off: &mut usize, n: usize| -> Result<Vec<u8>, Signal> {
        if *off + n > data.len() {
            return Err(Signal::error("unpack: string is shorter than the format"));
        }
        let s = data[*off..*off + n].to_vec();
        *off += n;
        Ok(s)
    };
    for f in &fields {
        match f {
            Field::Int {
                bytes,
                signed,
                endian,
            } => {
                let le = order(&take(&mut off, *bytes)?, *endian);
                let mut u = 0u64;
                for (k, b) in le.iter().enumerate() {
                    u |= u64::from(*b) << (8 * k);
                }
                let val = if *signed && *bytes < 8 {
                    let shift = 64 - 8 * bytes;
                    ((u << shift) as i64) >> shift
                } else {
                    u as i64
                };
                out.push(Value::Int(val));
            }
            Field::F32(endian) => {
                let le = order(&take(&mut off, 4)?, *endian);
                let bits = u32::from_le_bytes(le.try_into().unwrap());
                out.push(Value::Float(f64::from(f32::from_bits(bits))));
            }
            Field::F64(endian) => {
                let le = order(&take(&mut off, 8)?, *endian);
                let bits = u64::from_le_bytes(le.try_into().unwrap());
                out.push(Value::Float(f64::from_bits(bits)));
            }
            Field::Str(n) => out.push(Value::str(take(&mut off, *n)?)),
            Field::Pad(n) => {
                take(&mut off, *n)?;
            }
        }
    }
    Ok(Value::list(out))
}

/// `(pack layout val…)` — serialise `val…` to a binary string. `layout` is
/// either a **struct** (a list of C type names — native alignment, padding, and
/// byte order, so the bytes match a real C struct) or a **format string** (the
/// terse `c b d u ld lu Ld Lu f lf sN nN` mini-language with `>`/`<` endian
/// toggles — tightly packed, no alignment).
#[cfg(all(feature = "ffi", unix))]
fn b_pack(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let layout = args
        .first()
        .ok_or_else(|| Signal::error("pack: missing layout"))?;
    // A single list argument is spread into its elements, as in newLISP:
    // `(pack fmt (sequence 1 10))` packs the ten numbers.
    let spread;
    let vals: &[Value] = match &args[1..] {
        [Value::List(l)] => {
            spread = l.to_vec();
            &spread
        }
        rest => rest,
    };
    match layout {
        Value::Str(fmt) => pack_format(fmt, vals),
        Value::List(_) => pack_struct(layout, vals),
        _ => Err(Signal::error(
            "pack: layout must be a struct or a format string",
        )),
    }
}

/// `pack` for the struct path: native C ABI layout (alignment + padding).
/// Pointer fields (`char*`/`void*`) take an integer address.
#[cfg(all(feature = "ffi", unix))]
fn pack_struct(layout: &Value, vals: &[Value]) -> Result<Value, Signal> {
    let types = layout_types(layout)?;
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
    Ok(Value::str(buf))
}

/// `(unpack layout str)` — the inverse of `pack`. `layout` is a struct or a
/// format string (see `b_pack`); returns a list of values. In the struct path a
/// pointer field is dereferenced (`char*` -> the pointed-to C string) and a NULL
/// pointer raises an error.
#[cfg(all(feature = "ffi", unix))]
fn b_unpack(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let layout = args
        .first()
        .ok_or_else(|| Signal::error("unpack: missing layout"))?;
    let data = match args.get(1) {
        Some(Value::Str(b)) => b,
        _ => return Err(Signal::error("unpack: second argument must be a string")),
    };
    match layout {
        Value::Str(fmt) => unpack_format(fmt, data),
        Value::List(_) => unpack_struct(layout, data),
        _ => Err(Signal::error(
            "unpack: layout must be a struct or a format string",
        )),
    }
}

/// `unpack` for the struct path (native C ABI layout).
#[cfg(all(feature = "ffi", unix))]
fn unpack_struct(layout: &Value, bytes: &[u8]) -> Result<Value, Signal> {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    let types = layout_types(layout)?;
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
                Value::str(s)
            }
            CType::Void => unreachable!("layout_types rejects void"),
        };
        out.push(v);
    }
    Ok(Value::list(out))
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
    Ok(Value::str(bytes))
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

#[cfg(all(feature = "ffi", unix))]
#[cfg(test)]
mod tests {
    use super::*;

    // `Signal` isn't `Debug`, so `Result::unwrap` is unavailable here.
    fn ok(r: Result<Value, Signal>) -> Value {
        match r {
            Ok(v) => v,
            Err(_) => panic!("expected Ok"),
        }
    }

    fn bytes(v: Value) -> Vec<u8> {
        match v {
            Value::Str(b) => b.to_vec(),
            _ => panic!("expected a string"),
        }
    }

    fn ints(v: Value) -> Vec<i64> {
        match v {
            Value::List(items) => items
                .iter()
                .map(|x| match x {
                    Value::Int(n) => *n,
                    _ => panic!("expected an int field"),
                })
                .collect(),
            _ => panic!("expected a list"),
        }
    }

    /// Pack `vals` with `fmt`, then unpack with `fmt` again.
    fn round(fmt: &[u8], vals: &[Value]) -> Value {
        ok(unpack_format(fmt, &bytes(ok(pack_format(fmt, vals)))))
    }

    #[test]
    fn format_widths_and_signedness() {
        assert_eq!(bytes(ok(pack_format(b"c", &[Value::Int(65)]))), vec![65]);
        // A signed byte sees 0xFF as -1; an unsigned byte sees 255.
        let packed = bytes(ok(pack_format(b"b", &[Value::Int(255)])));
        assert_eq!(ints(ok(unpack_format(b"c", &packed))), vec![-1]);
        assert_eq!(ints(ok(unpack_format(b"b", &packed))), vec![255]);
        // 32-/64-bit signed round-trips.
        assert_eq!(
            ints(round(b"ld", &[Value::Int(-1_000_000)])),
            vec![-1_000_000]
        );
        assert_eq!(
            ints(round(b"Ld", &[Value::Int(123_456_789_012)])),
            vec![123_456_789_012]
        );
    }

    #[test]
    fn format_endianness() {
        // 0x1234 packs big-endian as [0x12, 0x34], little-endian reversed.
        assert_eq!(
            bytes(ok(pack_format(b">d", &[Value::Int(0x1234)]))),
            vec![0x12, 0x34]
        );
        assert_eq!(
            bytes(ok(pack_format(b"<d", &[Value::Int(0x1234)]))),
            vec![0x34, 0x12]
        );
        // A toggle applies to following fields only.
        assert_eq!(
            bytes(ok(pack_format(
                b">d <d",
                &[Value::Int(0x1234), Value::Int(0x1234)]
            ))),
            vec![0x12, 0x34, 0x34, 0x12]
        );
        assert_eq!(ints(round(b">Ld", &[Value::Int(-5)])), vec![-5]);
    }

    #[test]
    fn format_string_and_pad() {
        // `s5` null-pads; `n2` writes two nulls and consumes no value.
        let packed = bytes(ok(pack_format(
            b"s5 n2 c",
            &[Value::str(b"hi".to_vec()), Value::Int(66)],
        )));
        assert_eq!(packed, vec![b'h', b'i', 0, 0, 0, 0, 0, 66]);
        match ok(unpack_format(b"s5 n2 c", &packed)) {
            Value::List(items) => {
                assert_eq!(items.len(), 2, "pad consumes no value");
                assert!(matches!(&items[0], Value::Str(s) if s.as_slice() == b"hi\0\0\0"));
                assert!(matches!(items[1], Value::Int(66)));
            }
            _ => panic!("expected a list"),
        }
    }

    #[test]
    fn format_float_roundtrip() {
        match round(b"f lf", &[Value::Float(3.5), Value::Float(2.5)]) {
            Value::List(items) => {
                assert!(matches!(items[0], Value::Float(f) if f == 3.5));
                assert!(matches!(items[1], Value::Float(f) if f == 2.5));
            }
            _ => panic!("expected a list"),
        }
    }

    #[test]
    fn format_errors() {
        assert!(pack_format(b"z", &[]).is_err(), "unknown char");
        assert!(pack_format(b"l", &[]).is_err(), "dangling l");
        assert!(pack_format(b"c", &[]).is_err(), "too few values");
        assert!(unpack_format(b"Ld", b"\x01\x02").is_err(), "short input");
    }
}
