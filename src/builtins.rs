//! Primitive functions.
//!
//! Integer arithmetic (`+ - * / %`) wraps on overflow (ADR-0012); float
//! arithmetic uses `add sub mul div`, matching newLISP's split. Strings are
//! byte buffers, so `length` counts bytes (ADR-0013).

use std::cmp::Ordering;
use std::io::Write;

use crate::eval::{Interp, Signal};
use crate::printer::to_display;
use crate::value::{Builtin, BuiltinFn, SymId, Value};

#[cfg(feature = "bigint")]
use num_bigint::BigInt;
#[cfg(feature = "bigint")]
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};

pub fn install(interp: &Interp) {
    let reg = |name: &'static str, func: BuiltinFn| {
        let id = interp.intern(name);
        interp.set_global(id, Value::Builtin(Builtin { name, func }));
    };

    // Integer arithmetic (wrapping).
    reg("+", add_int);
    reg("-", sub_int);
    reg("*", mul_int);
    reg("/", div_int);
    reg("%", mod_int);
    // Float arithmetic.
    reg("add", add_flt);
    reg("sub", sub_flt);
    reg("mul", mul_flt);
    reg("div", div_flt);
    reg("sqrt", |_, a| flt1(a, f64::sqrt));
    reg("abs", b_abs);
    reg("atan", |_, a| flt1(a, f64::atan));
    reg("sin", |_, a| flt1(a, f64::sin));
    reg("cos", |_, a| flt1(a, f64::cos));
    reg("tan", |_, a| flt1(a, f64::tan));
    reg("asin", |_, a| flt1(a, f64::asin));
    reg("acos", |_, a| flt1(a, f64::acos));
    reg("exp", |_, a| flt1(a, f64::exp));
    reg("log", b_log);
    reg("pow", b_pow);
    reg("mod", b_mod);
    // Rounding, sign, extra trig/hyperbolic (ADR-0032 follow-on fill-ins).
    reg("ceil", |_, a| flt1(a, f64::ceil));
    reg("floor", |_, a| flt1(a, f64::floor));
    reg("round", b_round);
    reg("sgn", b_sgn);
    reg("atan2", b_atan2);
    reg("sinh", |_, a| flt1(a, f64::sinh));
    reg("cosh", |_, a| flt1(a, f64::cosh));
    reg("tanh", |_, a| flt1(a, f64::tanh));
    reg("asinh", |_, a| flt1(a, f64::asinh));
    reg("acosh", |_, a| flt1(a, f64::acosh));
    reg("atanh", |_, a| flt1(a, f64::atanh));
    reg("bits", b_bits);
    reg("base64-enc", b_base64_enc);
    reg("base64-dec", b_base64_dec);
    reg("parse", b_parse);
    reg("NaN?", is_nan_p);
    reg("inf?", is_inf_p);
    reg("int", b_int);
    reg("float", b_float);
    reg("char", b_char);
    // Arbitrary-precision integers (ADR-0022), only under the `bigint` feature.
    #[cfg(feature = "bigint")]
    {
        reg("bigint", b_bigint);
        reg("gcd", b_gcd);
    }
    // Comparison.
    reg("=", eq);
    reg("!=", ne);
    reg("<", lt);
    reg(">", gt);
    reg("<=", le);
    reg(">=", ge);
    // Lists.
    reg("list", b_list);
    reg("cons", b_cons);
    reg("first", b_first);
    reg("rest", b_rest);
    reg("last", b_last);
    reg("nth", b_nth);
    reg("length", b_length);
    reg("utf8len", b_utf8len);
    reg("term", b_term);
    reg("args", b_args);
    reg("expand", b_expand);
    reg("append", b_append);
    reg("sequence", b_sequence);
    reg("map", b_map);
    reg("apply", b_apply);
    reg("filter", b_filter);
    reg("dup", b_dup);
    // Predicates.
    reg("nil?", is_nil);
    reg("null?", is_nil);
    reg("integer?", is_integer);
    reg("float?", is_float);
    reg("number?", is_number);
    reg("string?", is_string);
    reg("symbol?", is_symbol);
    reg("list?", is_list);
    reg("array?", is_array);
    reg("true?", is_true_p);
    reg("atom?", is_atom);
    reg("array", b_array);
    reg("array-list", b_array_list);
    reg("zero?", is_zero);
    reg("empty?", is_empty);
    reg("not", b_not);
    reg("eval", b_eval);
    // Bitwise.
    reg("&", bit_and);
    reg("|", bit_or);
    reg("^", bit_xor);
    reg("<<", shl);
    reg(">>", shr);
    // I/O and misc.
    reg("time-of-day", time_of_day);
    reg("format", b_format);
    reg("lookup", b_lookup);
    reg("assoc", b_assoc);
    reg("new", b_new);
    reg("delete", b_delete);
    reg("sys-info", b_sys_info);
    reg("randomize", b_randomize);
    reg("starts-with", b_starts_with);
    reg("ends-with", b_ends_with);
    reg("upper-case", b_upper_case);
    reg("lower-case", b_lower_case);
    // Regular expressions (ADR-0028), only under the `regex` feature.
    #[cfg(feature = "regex")]
    {
        reg("regex", b_regex);
        reg("regex-comp", b_regex_comp);
    }
    reg("trim", b_trim);
    reg("slice", b_slice);
    reg("find", b_find);
    reg("chop", b_chop);
    reg("explode", b_explode);
    reg("flat", b_flat);
    reg("join", b_join);
    reg("member", b_member);
    reg("unique", b_unique);
    reg("min", b_min);
    reg("max", b_max);
    reg("even?", is_even);
    reg("odd?", is_odd);
    // RNG and process environment.
    reg("seed", b_seed);
    reg("rand", b_rand);
    reg("random", b_random);
    reg("main-args", b_main_args);
    reg("set-locale", |_, _| Ok(Value::str(b"C".to_vec())));
    reg("print", b_print);
    reg("println", b_println);
    reg("string", b_string);
    reg("exit", b_exit);

    // FOOP base context, predefined as in newLISP (ADR-0030). A non-nil default
    // functor marks a context as a FOOP class (object construction); a nil
    // functor leaves a context free to act as a Dictionary. `new` copies this
    // marker to a derived class.
    let class = interp.intern("Class");
    interp.set_global(class, Value::Context(class));
    let class_ctor = interp.intern("Class:Class");
    interp.set_global(
        class_ctor,
        Value::Builtin(Builtin {
            name: "Class:Class",
            func: class_marker,
        }),
    );
}

// ---- numeric coercion ----------------------------------------------------

fn to_i64(v: &Value) -> Result<i64, Signal> {
    match v {
        Value::Int(n) => Ok(*n),
        Value::Float(f) => Ok(*f as i64),
        #[cfg(feature = "bigint")]
        Value::Bigint(b) => Ok(bigint_to_i64(b)),
        _ => Err(Signal::error("expected a number")),
    }
}

// ---- bigint helpers (ADR-0022) -------------------------------------------

/// True if any argument is a bigint — selects the bigint arithmetic path.
#[cfg(feature = "bigint")]
fn any_bigint(args: &[Value]) -> bool {
    args.iter().any(|a| matches!(a, Value::Bigint(_)))
}

/// Coerce a numeric value to `BigInt`; a float is truncated toward zero, as the
/// integer operators `+ - * / %` truncate float arguments (ADR-0022).
#[cfg(feature = "bigint")]
fn to_bigint(v: &Value) -> Result<BigInt, Signal> {
    match v {
        Value::Int(n) => Ok(BigInt::from(*n)),
        Value::Bigint(b) => Ok(b.clone()),
        Value::Float(f) if f.is_finite() => Ok(BigInt::from_f64(f.trunc()).unwrap_or_default()),
        Value::Float(_) => Err(Signal::error(
            "cannot convert a non-finite float to a bigint",
        )),
        _ => Err(Signal::error("expected a number")),
    }
}

/// The low 64 bits of a bigint as `i64` (wrapping), for `int` and index uses;
/// the out-of-range case is unspecified beyond this (ADR-0022).
#[cfg(feature = "bigint")]
fn bigint_to_i64(b: &BigInt) -> i64 {
    b.to_i64().unwrap_or_else(|| {
        let (sign, digits) = b.to_u64_digits();
        let low = digits.first().copied().unwrap_or(0) as i64;
        if matches!(sign, num_bigint::Sign::Minus) {
            low.wrapping_neg()
        } else {
            low
        }
    })
}

fn to_f64(v: &Value) -> Result<f64, Signal> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        #[cfg(feature = "bigint")]
        Value::Bigint(b) => Ok(b.to_f64().unwrap_or(f64::INFINITY)),
        _ => Err(Signal::error("expected a number")),
    }
}

fn as_f64_opt(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        #[cfg(feature = "bigint")]
        Value::Bigint(b) => Some(b.to_f64().unwrap_or(f64::INFINITY)),
        _ => None,
    }
}

// ---- integer arithmetic (wrapping, ADR-0012) -----------------------------

fn add_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if any_bigint(args) {
        let mut acc = BigInt::from(0);
        for a in args {
            acc += to_bigint(a)?;
        }
        return Ok(Value::Bigint(acc));
    }
    let mut acc: i64 = 0;
    for a in args {
        acc = acc.wrapping_add(to_i64(a)?);
    }
    Ok(Value::Int(acc))
}

fn sub_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Ok(Value::Int(0));
    }
    #[cfg(feature = "bigint")]
    if any_bigint(args) {
        let mut acc = to_bigint(&args[0])?;
        if args.len() == 1 {
            return Ok(Value::Bigint(-acc));
        }
        for a in &args[1..] {
            acc -= to_bigint(a)?;
        }
        return Ok(Value::Bigint(acc));
    }
    let mut acc = to_i64(&args[0])?;
    if args.len() == 1 {
        return Ok(Value::Int(acc.wrapping_neg()));
    }
    for a in &args[1..] {
        acc = acc.wrapping_sub(to_i64(a)?);
    }
    Ok(Value::Int(acc))
}

fn mul_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if any_bigint(args) {
        let mut acc = BigInt::from(1);
        for a in args {
            acc *= to_bigint(a)?;
        }
        return Ok(Value::Bigint(acc));
    }
    let mut acc: i64 = 1;
    for a in args {
        acc = acc.wrapping_mul(to_i64(a)?);
    }
    Ok(Value::Int(acc))
}

fn div_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Ok(Value::Int(1));
    }
    #[cfg(feature = "bigint")]
    if any_bigint(args) {
        let mut acc = to_bigint(&args[0])?;
        for a in &args[1..] {
            let d = to_bigint(a)?;
            if d.is_zero() {
                return Err(Signal::error("division by zero"));
            }
            acc /= d; // truncates toward zero
        }
        return Ok(Value::Bigint(acc));
    }
    let mut acc = to_i64(&args[0])?;
    for a in &args[1..] {
        let d = to_i64(a)?;
        if d == 0 {
            return Err(Signal::error("division by zero"));
        }
        acc = acc.wrapping_div(d);
    }
    Ok(Value::Int(acc))
}

fn mod_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("%: expected 2 arguments"));
    }
    #[cfg(feature = "bigint")]
    if any_bigint(args) {
        let d = to_bigint(&args[1])?;
        if d.is_zero() {
            return Err(Signal::error("division by zero"));
        }
        // Remainder takes the dividend's sign (as the i64 path does).
        return Ok(Value::Bigint(to_bigint(&args[0])? % d));
    }
    let d = to_i64(&args[1])?;
    if d == 0 {
        return Err(Signal::error("division by zero"));
    }
    Ok(Value::Int(to_i64(&args[0])?.wrapping_rem(d)))
}

// ---- float arithmetic ----------------------------------------------------

fn add_flt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut acc = 0.0;
    for a in args {
        acc += to_f64(a)?;
    }
    Ok(Value::Float(acc))
}

fn sub_flt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Ok(Value::Float(0.0));
    }
    let mut acc = to_f64(&args[0])?;
    if args.len() == 1 {
        return Ok(Value::Float(-acc));
    }
    for a in &args[1..] {
        acc -= to_f64(a)?;
    }
    Ok(Value::Float(acc))
}

fn mul_flt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut acc = 1.0;
    for a in args {
        acc *= to_f64(a)?;
    }
    Ok(Value::Float(acc))
}

fn div_flt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Ok(Value::Float(1.0));
    }
    let mut acc = to_f64(&args[0])?;
    for a in &args[1..] {
        acc /= to_f64(a)?;
    }
    Ok(Value::Float(acc))
}

/// Apply a unary f64 function to the first argument.
fn flt1(args: &[Value], f: fn(f64) -> f64) -> Result<Value, Signal> {
    Ok(Value::Float(f(to_f64(
        args.first().unwrap_or(&Value::Nil),
    )?)))
}

fn b_log(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let x = to_f64(args.first().unwrap_or(&Value::Nil))?;
    match args.get(1) {
        Some(base) => Ok(Value::Float(x.log(to_f64(base)?))),
        None => Ok(Value::Float(x.ln())),
    }
}

fn b_pow(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("pow: expected 2 arguments"));
    }
    Ok(Value::Float(to_f64(&args[0])?.powf(to_f64(&args[1])?)))
}

/// Float modulo: `(mod x 0)` yields NaN (unlike integer `%`, which errors).
fn b_mod(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("mod: expected 2 arguments"));
    }
    Ok(Value::Float(to_f64(&args[0])? % to_f64(&args[1])?))
}

fn is_nan_p(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(
        matches!(args.first(), Some(Value::Float(f)) if f.is_nan()),
    ))
}

fn is_inf_p(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(
        matches!(args.first(), Some(Value::Float(f)) if f.is_infinite()),
    ))
}

fn b_int(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let default = || match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    };
    let n = match args.first() {
        // `as i64` saturates: inf -> i64::MAX/MIN, NaN -> 0, matching newLISP.
        Some(Value::Int(n)) => *n,
        Some(Value::Float(f)) => *f as i64,
        #[cfg(feature = "bigint")]
        Some(Value::Bigint(b)) => bigint_to_i64(b),
        Some(Value::Str(b)) => {
            let s = String::from_utf8_lossy(b);
            let t = s.trim();
            t.parse::<i64>()
                .ok()
                .or_else(|| t.parse::<f64>().ok().map(|f| f as i64))
                .unwrap_or_else(default)
        }
        _ => default(),
    };
    Ok(Value::Int(n))
}

fn b_float(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let default = || match args.get(1) {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(n)) => *n as f64,
        _ => 0.0,
    };
    let f = match args.first() {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(f)) => *f,
        #[cfg(feature = "bigint")]
        Some(Value::Bigint(b)) => b.to_f64().unwrap_or(f64::INFINITY),
        Some(Value::Str(b)) => String::from_utf8_lossy(b)
            .trim()
            .parse::<f64>()
            .unwrap_or_else(|_| default()),
        _ => default(),
    };
    Ok(Value::Float(f))
}

/// `(char n)` -> a one-character string; `(char "s")` -> the first code point.
fn b_char(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Int(n)) => {
            let s = char::from_u32(*n as u32)
                .map(|c| c.to_string().into_bytes())
                .unwrap_or_else(|| vec![(*n & 0xff) as u8]);
            Ok(Value::str(s))
        }
        Some(Value::Str(b)) => match String::from_utf8_lossy(b).chars().next() {
            Some(c) => Ok(Value::Int(c as i64)),
            None => Ok(Value::Nil),
        },
        _ => Err(Signal::error("char: expected an integer or string")),
    }
}

/// `(round num [digits])` — round to `digits` decimal places (default 0),
/// half-away-from-zero; a negative `digits` rounds to tens/hundreds/…
fn b_round(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let x = to_f64(args.first().unwrap_or(&Value::Nil))?;
    let digits = match args.get(1) {
        Some(v) => to_i64(v)?,
        None => 0,
    };
    let factor = 10f64.powi(digits as i32);
    Ok(Value::Float((x * factor).round() / factor))
}

/// `(sgn num [neg zero pos])` — the sign as `-1`/`0`/`1`, or the matching one of
/// the optional branch values.
fn b_sgn(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let x = to_f64(args.first().unwrap_or(&Value::Nil))?;
    let (branch, default) = if x < 0.0 {
        (1, -1)
    } else if x > 0.0 {
        (3, 1)
    } else {
        (2, 0)
    };
    match args.get(branch) {
        Some(v) if args.len() > branch => Ok(v.clone()),
        _ => Ok(Value::Int(default)),
    }
}

fn b_atan2(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(Value::Float(
        to_f64(args.first().unwrap_or(&Value::Nil))?
            .atan2(to_f64(args.get(1).unwrap_or(&Value::Nil))?),
    ))
}

/// `(bits int)` — the binary-digit string of `int` (two's-complement bit pattern
/// for a negative number, no leading zeros).
fn b_bits(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = to_i64(args.first().unwrap_or(&Value::Nil))?;
    Ok(Value::str(format!("{:b}", n as u64).into_bytes()))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b_base64_enc(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let data = match args.first() {
        Some(Value::Str(b)) => b.to_vec(),
        _ => return Err(Signal::error("base64-enc: expected a string")),
    };
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(B64[(b0 >> 2) as usize]);
        out.push(B64[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize]);
        out.push(if chunk.len() > 1 {
            B64[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            B64[(b2 & 0x3f) as usize]
        } else {
            b'='
        });
    }
    Ok(Value::str(out))
}

fn b_base64_dec(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let s = match args.first() {
        Some(Value::Str(b)) => b.to_vec(),
        _ => return Err(Signal::error("base64-dec: expected a string")),
    };
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for &c in &s {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => continue, // skip newlines and other separators
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(Value::str(out))
}

/// Split byte string `s` on each occurrence of the literal separator `sep`,
/// keeping empty pieces between adjacent separators.
fn split_literal(s: &[u8], sep: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + sep.len() <= s.len() {
        if &s[i..i + sep.len()] == sep {
            out.push(Value::str(s[start..i].to_vec()));
            i += sep.len();
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(Value::str(s[start..].to_vec()));
    out
}

/// `(parse str [sep [option]])` — split `str` into a list of strings: on runs of
/// whitespace by default; on the literal separator `sep`; or, with a third
/// `option` argument (a PCRE option integer), on the regex `sep` (regex feature).
#[cfg_attr(not(feature = "regex"), allow(unused_variables))]
fn b_parse(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let s = match args.first() {
        Some(Value::Str(b)) => b.clone(),
        _ => return Err(Signal::error("parse: expected a string")),
    };
    match args.get(1) {
        None => Ok(Value::list(
            s.split(u8::is_ascii_whitespace)
                .filter(|t| !t.is_empty())
                .map(|t| Value::str(t.to_vec()))
                .collect(),
        )),
        Some(Value::Str(sep)) => {
            #[cfg(feature = "regex")]
            if let Some(opt) = args.get(2) {
                let pattern = String::from_utf8_lossy(sep).into_owned();
                let re = interp.compiled_regex(&pattern, to_i64(opt)?)?;
                return Ok(Value::list(
                    re.split(&s).map(|p| Value::str(p.to_vec())).collect(),
                ));
            }
            if sep.is_empty() {
                // An empty separator splits into individual characters.
                return Ok(Value::list(
                    crate::utf8::char_ranges(&s)
                        .map(|(a, b)| Value::str(s[a..b].to_vec()))
                        .collect(),
                ));
            }
            Ok(Value::list(split_literal(&s, sep)))
        }
        _ => Err(Signal::error("parse: separator must be a string")),
    }
}

fn b_abs(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.wrapping_abs())),
        Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
        #[cfg(feature = "bigint")]
        Some(Value::Bigint(b)) => Ok(Value::Bigint(b.abs())),
        _ => Err(Signal::error("abs: expected a number")),
    }
}

/// `(bigint x)` — convert a number or numeric string to an arbitrary-precision
/// integer (ADR-0022). A float is truncated toward zero; a string may carry a
/// leading sign and a trailing `L`. Registering this builtin also makes the
/// bare symbol `bigint` truthy, which newLISP scripts probe via `(unless bigint …)`.
#[cfg(feature = "bigint")]
fn b_bigint(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let b = match args.first() {
        Some(Value::Int(n)) => BigInt::from(*n),
        Some(Value::Bigint(b)) => b.clone(),
        Some(Value::Float(f)) if f.is_finite() => BigInt::from_f64(f.trunc())
            .ok_or_else(|| Signal::error("bigint: float out of range"))?,
        Some(Value::Float(_)) => {
            return Err(Signal::error("bigint: cannot convert a non-finite float"))
        }
        Some(Value::Str(bytes)) => {
            let s = String::from_utf8_lossy(bytes);
            let t = s.trim();
            let t = t.strip_suffix('L').unwrap_or(t);
            let t = t.strip_prefix('+').unwrap_or(t);
            t.parse::<BigInt>()
                .map_err(|_| Signal::error("bigint: invalid numeric string"))?
        }
        _ => return Err(Signal::error("bigint: expected a number or string")),
    };
    Ok(Value::Bigint(b))
}

/// `(gcd a b …)` — greatest common divisor by Euclid on `BigInt` (ADR-0022).
/// Returns a bigint if any argument is one, else an int.
#[cfg(feature = "bigint")]
fn b_gcd(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Err(Signal::error("gcd: expected at least one argument"));
    }
    let mut acc = to_bigint(&args[0])?.abs();
    for a in &args[1..] {
        let mut b = to_bigint(a)?.abs();
        while !b.is_zero() {
            let t = &acc % &b;
            acc = b;
            b = t;
        }
    }
    if any_bigint(args) {
        Ok(Value::Bigint(acc))
    } else {
        // The gcd of i64 inputs fits i64 except gcd(i64::MIN, 0); fall back to
        // the bigint form in that rare case rather than wrapping.
        Ok(acc.to_i64().map_or(Value::Bigint(acc), Value::Int))
    }
}

// ---- comparison ----------------------------------------------------------

pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) | (Value::True, Value::True) => true,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => (*x as f64) == *y,
        // Cross-type numeric equality with bigint (ADR-0022): exact against an
        // int (lift to BigInt), approximate against a float (as newLISP does).
        #[cfg(feature = "bigint")]
        (Value::Bigint(x), Value::Bigint(y)) => x == y,
        #[cfg(feature = "bigint")]
        (Value::Bigint(x), Value::Int(y)) | (Value::Int(y), Value::Bigint(x)) => {
            *x == BigInt::from(*y)
        }
        #[cfg(feature = "bigint")]
        (Value::Bigint(x), Value::Float(y)) | (Value::Float(y), Value::Bigint(x)) => {
            x.to_f64().is_some_and(|xf| xf == *y)
        }
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Symbol(x), Value::Symbol(y)) => x == y,
        (Value::Context(x), Value::Context(y)) => x == y,
        // A context and the symbol of the same name compare equal, so FOOP
        // objects (context-headed) match quoted symbol-headed literals.
        (Value::Context(x), Value::Symbol(y)) | (Value::Symbol(y), Value::Context(x)) => x == y,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| values_equal(p, q))
        }
        // An array equals an array element-wise, but never a list (ADR-0023).
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| values_equal(p, q))
        }
        _ => false,
    }
}

fn eq(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    for w in args.windows(2) {
        if !values_equal(&w[0], &w[1]) {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::True)
}

fn ne(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    for w in args.windows(2) {
        if values_equal(&w[0], &w[1]) {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::True)
}

fn chain(args: &[Value], accept: impl Fn(Ordering) -> bool) -> Result<Value, Signal> {
    for w in args.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        match compare_num(a, b) {
            // NaN is unordered: any comparison involving it is false (-> nil),
            // not an error (qa-float).
            Some(Some(o)) if accept(o) => {}
            Some(_) => return Ok(Value::Nil),
            None => {
                if let (Value::Str(x), Value::Str(y)) = (a, b) {
                    if !accept(x.cmp(y)) {
                        return Ok(Value::Nil);
                    }
                } else {
                    return Err(Signal::error("cannot compare these values"));
                }
            }
        }
    }
    Ok(Value::True)
}

/// Numerically order two values. `None` when they are not both numeric (the
/// caller falls back to string comparison); `Some(None)` when a NaN makes them
/// unordered. If a float is involved the comparison is in `f64`; otherwise
/// int/bigint operands are compared exactly via `BigInt` (ADR-0022).
fn compare_num(a: &Value, b: &Value) -> Option<Option<Ordering>> {
    let a_float = matches!(a, Value::Float(_));
    let b_float = matches!(b, Value::Float(_));
    if a_float || b_float {
        return match (as_f64_opt(a), as_f64_opt(b)) {
            (Some(x), Some(y)) => Some(x.partial_cmp(&y)),
            _ => None,
        };
    }
    #[cfg(feature = "bigint")]
    if matches!(a, Value::Bigint(_)) || matches!(b, Value::Bigint(_)) {
        return match (to_bigint(a), to_bigint(b)) {
            (Ok(x), Ok(y)) => Some(Some(x.cmp(&y))),
            _ => None,
        };
    }
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(Some(x.cmp(y))),
        _ => None,
    }
}

fn lt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    chain(args, |o| o == Ordering::Less)
}
fn gt(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    chain(args, |o| o == Ordering::Greater)
}
fn le(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    chain(args, |o| o != Ordering::Greater)
}
fn ge(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    chain(args, |o| o != Ordering::Less)
}

// ---- lists ---------------------------------------------------------------

fn b_list(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(Value::list(args.to_vec()))
}

fn b_cons(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("cons: expected 2 arguments"));
    }
    // No dotted pairs (ADR-0005): (cons x list) prepends; otherwise a 2-list.
    match &args[1] {
        Value::List(tail) => {
            let mut out = Vec::with_capacity(tail.len() + 1);
            out.push(args[0].clone());
            out.extend_from_slice(tail);
            Ok(Value::list(out))
        }
        other => Ok(Value::list(vec![args[0].clone(), other.clone()])),
    }
}

/// The `idx`-th character of a byte string as a one-character string, or `nil`
/// out of range. Character-based (ADR-0025), so multi-byte characters stay whole.
fn str_nth_char(bytes: &[u8], idx: i64) -> Value {
    match crate::utf8::char_byte_range(bytes, idx) {
        Some((s, e)) => Value::str(bytes[s..e].to_vec()),
        None => Value::Nil,
    }
}

fn b_first(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) | Some(Value::Array(l)) => l
            .first()
            .cloned()
            .ok_or_else(|| Signal::error("first: empty list")),
        // The first character (ADR-0025), which may be multiple bytes.
        Some(Value::Str(b)) if !b.is_empty() => Ok(str_nth_char(b, 0)),
        _ => Err(Signal::error("first: expected a non-empty list or string")),
    }
}

fn b_rest(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) => Ok(Value::list(l.get(1..).unwrap_or(&[]).to_vec())),
        // The bytes after the first character (ADR-0025).
        Some(Value::Str(b)) => {
            let start = crate::utf8::first_char_end(b);
            Ok(Value::str(b.get(start..).unwrap_or(&[]).to_vec()))
        }
        _ => Err(Signal::error("rest: expected a list or string")),
    }
}

fn b_last(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) | Some(Value::Array(l)) => l
            .last()
            .cloned()
            .ok_or_else(|| Signal::error("last: empty list")),
        // The last character (ADR-0025).
        Some(Value::Str(b)) if !b.is_empty() => Ok(str_nth_char(b, -1)),
        _ => Err(Signal::error("last: expected a list")),
    }
}

fn b_nth(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("nth: expected (nth index list)"));
    }
    let idx = to_i64(&args[0])?;
    match &args[1] {
        Value::List(l) | Value::Array(l) => {
            let i = if idx < 0 { l.len() as i64 + idx } else { idx };
            if i < 0 || i as usize >= l.len() {
                Ok(Value::Nil)
            } else {
                Ok(l[i as usize].clone())
            }
        }
        // Character indexing on a string (ADR-0025).
        Value::Str(b) => Ok(str_nth_char(b, idx)),
        _ => Err(Signal::error("nth: expected a list")),
    }
}

/// `(regex pattern text [option [offset]])` — the first match over the byte
/// string as `(match byte-off byte-len [subN offN lenN…])`, or `nil` (ADR-0028).
#[cfg(feature = "regex")]
fn b_regex(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let pattern = match args.first() {
        Some(Value::Str(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => return Err(Signal::error("regex: pattern must be a string")),
    };
    let text = match args.get(1) {
        Some(Value::Str(b)) => b,
        _ => return Err(Signal::error("regex: text must be a string")),
    };
    let option = match args.get(2) {
        Some(v) => to_i64(v)?,
        None => 0,
    };
    let offset = match args.get(3) {
        Some(v) => to_i64(v)?.max(0) as usize,
        None => 0,
    };
    let re = interp.compiled_regex(&pattern, option)?;
    if offset > text.len() {
        return Ok(Value::Nil);
    }
    match re.captures(&text[offset..]) {
        None => Ok(Value::Nil),
        Some(caps) => {
            // Whole match then each matched subgroup, as (str off len) triples;
            // offsets are byte positions in the original text.
            let mut out = Vec::new();
            for m in caps.iter().flatten() {
                out.push(Value::str(m.as_bytes().to_vec()));
                out.push(Value::Int((offset + m.start()) as i64));
                out.push(Value::Int((m.end() - m.start()) as i64));
            }
            Ok(Value::list(out))
        }
    }
}

/// `(regex-comp pattern [option])` — compile and cache the pattern, returning it
/// on success and erroring on a malformed pattern (ADR-0028).
#[cfg(feature = "regex")]
fn b_regex_comp(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let pattern = match args.first() {
        Some(Value::Str(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => return Err(Signal::error("regex-comp: pattern must be a string")),
    };
    let option = match args.get(1) {
        Some(v) => to_i64(v)?,
        None => 0,
    };
    interp.compiled_regex(&pattern, option)?;
    Ok(args[0].clone())
}

/// `(args)` — the current lambda/fexpr's arguments not bound to a declared
/// parameter (ADR-0027); `(args i)` indexes into them (negative from the end).
fn b_args(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let current = interp.current_args();
    match args.first() {
        None => Ok(Value::list(current)),
        Some(v) => {
            let i = to_i64(v)?;
            let idx = if i < 0 { current.len() as i64 + i } else { i };
            Ok(usize::try_from(idx)
                .ok()
                .and_then(|k| current.get(k).cloned())
                .unwrap_or(Value::Nil))
        }
    }
}

/// A lambda-headed list `(lambda …)` / `(fn …)` / `(lambda-macro …)` — `expand`
/// treats it as opaque so it never rewrites a nested lambda's own parameters
/// (ADR-0027), which is what lets the gist reuse `F`/`X` across nested lambdas.
fn is_lambda_list(interp: &Interp, items: &[Value]) -> bool {
    matches!(items.first(), Some(Value::Symbol(id))
        if matches!(interp.sym_name(*id).as_str(), "lambda" | "fn" | "lambda-macro"))
}

/// Substitute `sym`'s current value into `v`, recursively through lists (but not
/// into a nested lambda).
fn expand_symbols(interp: &Interp, v: &Value, syms: &[SymId]) -> Value {
    match v {
        Value::Symbol(id) if syms.contains(id) => interp.lookup(*id),
        Value::List(items) if !is_lambda_list(interp, items) => Value::list(
            items
                .iter()
                .map(|x| expand_symbols(interp, x, syms))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Substitute the value of every upper-case-initial symbol bound to a **code-like**
/// value (a list or a function), recursively (ADR-0027) — newLISP's
/// `(expand expr)` form. Self-evaluating atoms (numbers, strings) are left as the
/// symbol: substituting a loop variable's number into a parameter position would
/// break the built lambda, which is the gist's documented reused-variable hazard.
fn expand_uppercase(interp: &Interp, v: &Value) -> Value {
    match v {
        Value::Symbol(id) => {
            let starts_upper = interp
                .sym_name(*id)
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase());
            if starts_upper {
                let val = interp.lookup(*id);
                if matches!(
                    val,
                    Value::List(_)
                        | Value::Lambda(_)
                        | Value::Fexpr(_)
                        | Value::Builtin(_)
                        | Value::Foreign(_)
                ) {
                    return val;
                }
            }
            v.clone()
        }
        Value::List(items) if !is_lambda_list(interp, items) => {
            Value::list(items.iter().map(|x| expand_uppercase(interp, x)).collect())
        }
        other => other.clone(),
    }
}

/// `(expand expr sym…)` substitutes the named symbols' values into `expr`;
/// `(expand expr)` substitutes upper-case-initial symbols with a value (ADR-0027).
fn b_expand(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let expr = args.first().cloned().unwrap_or(Value::Nil);
    if args.len() > 1 {
        let syms: Vec<SymId> = args[1..]
            .iter()
            .filter_map(|a| match a {
                Value::Symbol(id) => Some(*id),
                _ => None,
            })
            .collect();
        Ok(expand_symbols(interp, &expr, &syms))
    } else {
        Ok(expand_uppercase(interp, &expr))
    }
}

/// `(term sym)` — the symbol's term (the part after the last `:`) as a symbol;
/// `(term 'L:Albanian)` -> `Albanian` (ADR-0026).
fn b_term(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Symbol(id)) | Some(Value::Context(id)) => {
            let name = interp.sym_name(*id);
            let term = name.rsplit(':').next().unwrap_or(&name);
            Ok(Value::Symbol(interp.intern(term)))
        }
        _ => Err(Signal::error("term: expected a symbol")),
    }
}

/// `(utf8len str)` — the number of UTF-8 characters, vs `length`'s byte count.
fn b_utf8len(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::Int(crate::utf8::char_count(b) as i64)),
        Some(Value::Nil) | None => Ok(Value::Int(0)),
        _ => Err(Signal::error("utf8len: expected a string")),
    }
}

fn b_length(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.first() {
        Some(Value::List(l)) | Some(Value::Array(l)) => l.len() as i64,
        Some(Value::Str(b)) => b.len() as i64, // bytes, per ADR-0013
        // A bigint's length is its decimal digit count (a newLISP quirk, ADR-0022).
        #[cfg(feature = "bigint")]
        Some(Value::Bigint(b)) => b.to_str_radix(10).trim_start_matches('-').len() as i64,
        Some(Value::Nil) | None => 0,
        _ => 0,
    };
    Ok(Value::Int(n))
}

fn b_append(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // Strings concatenate; lists concatenate. `append` always returns a copy.
    if args.iter().all(|a| matches!(a, Value::Str(_))) {
        let mut out = Vec::new();
        for a in args {
            if let Value::Str(b) = a {
                out.extend_from_slice(b);
            }
        }
        return Ok(Value::str(out));
    }
    let mut out = Vec::new();
    for a in args {
        match a {
            Value::List(l) => out.extend_from_slice(l),
            other => {
                return Err(Signal::Error(format!(
                    "append: expected a list, got {}",
                    type_name(other)
                )))
            }
        }
    }
    Ok(Value::list(out))
}

// ---- higher-order and sequence ------------------------------------------

fn b_sequence(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() < 2 {
        return Err(Signal::error(
            "sequence: expected (sequence from to [step])",
        ));
    }
    let integral = args.iter().all(|v| matches!(v, Value::Int(_)));
    let from = to_f64(&args[0])?;
    let to = to_f64(&args[1])?;
    let step = match args.get(2) {
        Some(v) => to_f64(v)?.abs(),
        None => 1.0,
    };
    if step == 0.0 {
        return Err(Signal::error("sequence: step must be non-zero"));
    }
    let ascending = to >= from;
    let signed = if ascending { step } else { -step };
    let mut out = Vec::new();
    let mut v = from;
    while (ascending && v <= to + 1e-12) || (!ascending && v >= to - 1e-12) {
        out.push(if integral {
            Value::Int(v as i64)
        } else {
            Value::Float(v)
        });
        v += signed;
    }
    Ok(Value::list(out))
}

fn b_map(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let f = args
        .first()
        .ok_or_else(|| Signal::error("map: missing function"))?;
    let lists: Vec<&Vec<Value>> = args[1..]
        .iter()
        .map(|v| match v {
            Value::List(l) => Ok(&**l),
            _ => Err(Signal::error("map: expected list arguments")),
        })
        .collect::<Result<_, _>>()?;
    if lists.is_empty() {
        return Ok(Value::list(Vec::new()));
    }
    let n = lists.iter().map(|l| l.len()).min().unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let call_args: Vec<Value> = lists.iter().map(|l| l[k].clone()).collect();
        out.push(i.call(f, call_args)?);
    }
    Ok(Value::list(out))
}

fn b_apply(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let f = args
        .first()
        .ok_or_else(|| Signal::error("apply: missing function"))?;
    let call_args = match args.get(1) {
        Some(Value::List(l)) => l.to_vec(),
        // `nil` is the empty list: (apply * nil) is the identity, not (* nil).
        Some(Value::Nil) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    };
    i.call(f, call_args)
}

fn b_filter(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("filter: expected (filter predicate list)"));
    }
    let pred = &args[0];
    let list = match &args[1] {
        Value::List(l) => l,
        _ => return Err(Signal::error("filter: expected a list")),
    };
    let mut out = Vec::new();
    for item in list.iter() {
        if i.call(pred, vec![item.clone()])?.is_truthy() {
            out.push(item.clone());
        }
    }
    Ok(Value::list(out))
}

fn b_dup(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.get(1) {
        Some(Value::Int(n)) => (*n).max(0) as usize,
        _ => return Err(Signal::error("dup: expected (dup value count)")),
    };
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::str(b.repeat(n))),
        Some(v) => Ok(Value::list(vec![v.clone(); n])),
        None => Ok(Value::Nil),
    }
}

// ---- predicates ----------------------------------------------------------

fn boolean(b: bool) -> Value {
    if b {
        Value::True
    } else {
        Value::Nil
    }
}

fn is_nil(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::Nil) | None)))
}
fn is_integer(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if matches!(args.first(), Some(Value::Bigint(_))) {
        return Ok(Value::True);
    }
    Ok(boolean(matches!(args.first(), Some(Value::Int(_)))))
}
fn is_float(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::Float(_)))))
}
fn is_number(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if matches!(args.first(), Some(Value::Bigint(_))) {
        return Ok(Value::True);
    }
    Ok(boolean(matches!(
        args.first(),
        Some(Value::Int(_)) | Some(Value::Float(_))
    )))
}
fn is_string(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::Str(_)))))
}
fn is_symbol(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::Symbol(_)))))
}
fn is_list(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::List(_)))))
}
fn is_atom(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // Neither a list nor an array is an atom (ADR-0023).
    Ok(boolean(!matches!(
        args.first(),
        Some(Value::List(_)) | Some(Value::Array(_))
    )))
}
fn is_array(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(matches!(args.first(), Some(Value::Array(_)))))
}
/// `(true? x)` — true unless `x` is nil or the empty list/array (newLISP truthiness).
fn is_true_p(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(args.first().is_some_and(|v| v.is_truthy())))
}
fn is_zero(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if matches!(args.first(), Some(Value::Bigint(b)) if b.is_zero()) {
        return Ok(Value::True);
    }
    Ok(boolean(
        matches!(args.first(), Some(Value::Int(0)))
            || matches!(args.first(), Some(Value::Float(f)) if *f == 0.0),
    ))
}
fn is_empty(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(match args.first() {
        Some(Value::List(l)) => l.is_empty(),
        Some(Value::Str(b)) => b.is_empty(),
        Some(Value::Nil) | None => true,
        _ => false,
    }))
}
fn b_not(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(
        !args.first().map(|v| v.is_truthy()).unwrap_or(false),
    ))
}

fn b_eval(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    i.eval(args.first().unwrap_or(&Value::Nil))
}

// ---- I/O and misc --------------------------------------------------------

fn do_print(i: &Interp, args: &[Value], newline: bool) -> Result<Value, Signal> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    for a in args {
        match a {
            Value::Str(b) => {
                let _ = lock.write_all(b);
            }
            other => {
                let s = to_display(other, &i.interner.borrow());
                let _ = lock.write_all(s.as_bytes());
            }
        }
    }
    if newline {
        let _ = lock.write_all(b"\n");
    }
    let _ = lock.flush();
    Ok(args.last().cloned().unwrap_or(Value::Nil))
}

fn b_print(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    do_print(i, args, false)
}
fn b_println(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    do_print(i, args, true)
}

fn b_string(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Value::Str(b) => out.extend_from_slice(b),
            other => out.extend_from_slice(to_display(other, &i.interner.borrow()).as_bytes()),
        }
    }
    Ok(Value::str(out))
}

// ---- bitwise -------------------------------------------------------------

fn bit_and(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut acc: i64 = -1;
    for a in args {
        acc &= to_i64(a)?;
    }
    Ok(Value::Int(acc))
}
fn bit_or(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut acc: i64 = 0;
    for a in args {
        acc |= to_i64(a)?;
    }
    Ok(Value::Int(acc))
}
fn bit_xor(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut acc: i64 = 0;
    for a in args {
        acc ^= to_i64(a)?;
    }
    Ok(Value::Int(acc))
}
fn shl(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("<<: expected 2 arguments"));
    }
    Ok(Value::Int(
        to_i64(&args[0])?.wrapping_shl(to_i64(&args[1])? as u32),
    ))
}
fn shr(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error(">>: expected 2 arguments"));
    }
    Ok(Value::Int(
        to_i64(&args[0])?.wrapping_shr(to_i64(&args[1])? as u32),
    ))
}

fn time_of_day(_: &Interp, _: &[Value]) -> Result<Value, Signal> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(Value::Int(ms))
}

// ---- format (printf subset) ---------------------------------------------

fn b_format(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let fmt = match args.first() {
        Some(Value::Str(b)) => b.clone(),
        _ => return Err(Signal::error("format: first argument must be a string")),
    };
    let mut out = Vec::new();
    let mut argi = 1usize;
    let mut k = 0usize;
    while k < fmt.len() {
        let c = fmt[k];
        if c != b'%' {
            out.push(c);
            k += 1;
            continue;
        }
        // Collect the conversion spec: %[flags][width][.prec]conv
        let mut spec = String::from("%");
        k += 1;
        if k < fmt.len() && fmt[k] == b'%' {
            out.push(b'%');
            k += 1;
            continue;
        }
        let mut conv = None;
        while k < fmt.len() {
            let ch = fmt[k] as char;
            spec.push(ch);
            k += 1;
            if "diouxXeEfFgGsc".contains(ch) {
                conv = Some(ch);
                break;
            }
        }
        let conv = conv.ok_or_else(|| Signal::error("format: incomplete conversion"))?;
        let arg = args.get(argi).cloned().unwrap_or(Value::Nil);
        argi += 1;
        out.extend_from_slice(format_one(&spec, conv, &arg, i)?.as_bytes());
    }
    Ok(Value::str(out))
}

fn format_one(spec: &str, conv: char, arg: &Value, i: &Interp) -> Result<String, Signal> {
    let inner = &spec[1..spec.len() - 1]; // between '%' and the conversion char
    let mut chars = inner.chars().peekable();
    let (mut left, mut zero, mut plus, mut space) = (false, false, false, false);
    while let Some(&c) = chars.peek() {
        match c {
            '-' => left = true,
            '0' => zero = true,
            '+' => plus = true,
            ' ' => space = true,
            _ => break,
        }
        chars.next();
    }
    let mut width = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            width.push(c);
            chars.next();
        } else {
            break;
        }
    }
    let width: usize = width.parse().unwrap_or(0);
    let mut prec = None;
    if let Some(&'.') = chars.peek() {
        chars.next();
        let mut p = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                p.push(c);
                chars.next();
            } else {
                break;
            }
        }
        prec = Some(p.parse::<usize>().unwrap_or(0));
    }

    let body = match conv {
        'd' | 'i' | 'u' => {
            let n = to_i64(arg)?;
            if n < 0 {
                format!("-{}", n.unsigned_abs())
            } else if plus {
                format!("+{}", n)
            } else if space {
                format!(" {}", n)
            } else {
                n.to_string()
            }
        }
        'f' | 'F' => format!("{:.*}", prec.unwrap_or(6), to_f64(arg)?),
        'e' | 'E' => format!("{:.*e}", prec.unwrap_or(6), to_f64(arg)?),
        'g' | 'G' => format!("{}", to_f64(arg)?),
        'x' => format!("{:x}", to_i64(arg)?),
        'X' => format!("{:X}", to_i64(arg)?),
        'o' => format!("{:o}", to_i64(arg)?),
        'c' => ((to_i64(arg)? as u8) as char).to_string(),
        's' => {
            let s = match arg {
                Value::Str(b) => String::from_utf8_lossy(b).into_owned(),
                other => to_display(other, &i.interner.borrow()),
            };
            match prec {
                Some(p) => s.chars().take(p).collect(),
                None => s,
            }
        }
        _ => return Err(Signal::error("format: unsupported conversion")),
    };

    if body.len() >= width {
        return Ok(body);
    }
    let pad = width - body.len();
    if left {
        Ok(format!("{}{}", body, " ".repeat(pad)))
    } else if zero && !matches!(conv, 's' | 'c') {
        match body.strip_prefix('-') {
            Some(rest) => Ok(format!("-{}{}", "0".repeat(pad), rest)),
            None => Ok(format!("{}{}", "0".repeat(pad), body)),
        }
    } else {
        Ok(format!("{}{}", " ".repeat(pad), body))
    }
}

fn b_assoc(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (assoc key alist) -> the (key ...) pair, or nil.
    let key = args.first().unwrap_or(&Value::Nil);
    if let Some(Value::List(items)) = args.get(1) {
        for item in items.iter() {
            if let Value::List(pair) = item {
                if pair.first().is_some_and(|k| values_equal(k, key)) {
                    return Ok(item.clone());
                }
            }
        }
    }
    Ok(Value::Nil)
}

fn b_lookup(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (lookup key alist [index]) -> element `index` of the matched pair.
    let key = args.first().unwrap_or(&Value::Nil);
    let idx = match args.get(2) {
        Some(Value::Int(n)) => *n,
        _ => -1,
    };
    if let Some(Value::List(items)) = args.get(1) {
        for item in items.iter() {
            if let Value::List(pair) = item {
                if pair.first().is_some_and(|k| values_equal(k, key)) {
                    let i = if idx < 0 {
                        pair.len() as i64 + idx
                    } else {
                        idx
                    };
                    if i >= 0 && (i as usize) < pair.len() {
                        return Ok(pair[i as usize].clone());
                    }
                    return Ok(Value::Nil);
                }
            }
        }
    }
    Ok(Value::Nil)
}

fn sym_id(v: &Value) -> Option<SymId> {
    match v {
        Value::Symbol(id) | Value::Context(id) => Some(*id),
        _ => None,
    }
}

fn b_new(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (new prototype 'name) copies the prototype context's symbols into a new
    // context `name`; (new 'name) makes an empty one (ADR-0030). The default
    // functor `Proto:Proto` maps to `name:name`, so copying `Class` gives the new
    // context a non-nil functor and thus FOOP-class (not Dictionary) behaviour.
    let (proto, name) = match (args.first(), args.get(1)) {
        (Some(p), Some(n)) => (sym_id(p), sym_id(n)),
        (Some(n), None) => (None, sym_id(n)),
        _ => (None, None),
    };
    let name = name.ok_or_else(|| Signal::error("new: expected a context name symbol"))?;
    i.set_global(name, Value::Context(name));
    if let Some(proto) = proto {
        let proto_name = i.sym_name(proto);
        let new_name = i.sym_name(name);
        if proto_name != new_name {
            for (term, val) in i.context_entries(&proto_name) {
                let new_term = if term == proto_name {
                    new_name.clone()
                } else {
                    term
                };
                let sym = i.intern(&format!("{}:{}", new_name, new_term));
                i.set_global(sym, val);
            }
        }
    }
    Ok(Value::Context(name))
}

/// The predefined `Class:Class` default-functor marker (ADR-0030). Never called
/// directly — `construct` treats any non-nil, non-lambda functor as an object
/// constructor and builds the tagged list itself; the marker exists only to give
/// a FOOP class a non-nil functor, distinguishing it from a Dictionary.
fn class_marker(_: &Interp, _: &[Value]) -> Result<Value, Signal> {
    Err(Signal::error(
        "Class is a constructor; apply a derived context to build an object",
    ))
}

/// `(delete 'sym)` — remove a symbol or, for a bare context name, the whole
/// context (all `Ctx:*` members), by clearing the value(s) to nil (ADR-0030).
fn b_delete(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let id = match args.first() {
        Some(Value::Symbol(id)) | Some(Value::Context(id)) => *id,
        _ => return Err(Signal::error("delete: expected a symbol")),
    };
    let name = i.sym_name(id);
    if name.contains(':') {
        i.set_global(id, Value::Nil);
    } else {
        for sym in i.context_symbol_ids(&name) {
            i.set_global(sym, Value::Nil);
        }
        i.set_global(id, Value::Nil);
    }
    Ok(Value::True)
}

/// `(sys-info [n])` — best-effort interpreter statistics (ADR-0030). Element 0 is
/// a rough live-cell count; other slots are 0. Not a compatibility surface.
fn b_sys_info(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let cells = i.global_count() as i64;
    let info = [cells, 0, cells, 0, 0, 0, 0, 0, 0, 0];
    match args.first() {
        Some(v) => {
            let n = to_i64(v)?;
            // Negative indices name process ids (the Cilk API uses them):
            // -3 = this process, -4 = the parent process.
            if n == -3 {
                return Ok(Value::Int(i64::from(std::process::id())));
            }
            if n == -4 {
                #[cfg(all(feature = "mt", unix))]
                return Ok(Value::Int(i64::from(unsafe { libc::getppid() })));
                #[cfg(not(all(feature = "mt", unix)))]
                return Ok(Value::Int(0));
            }
            Ok(usize::try_from(n)
                .ok()
                .and_then(|k| info.get(k))
                .map(|x| Value::Int(*x))
                .unwrap_or(Value::Nil))
        }
        None => Ok(Value::list(info.iter().map(|x| Value::Int(*x)).collect())),
    }
}

/// `(randomize list)` — a Fisher–Yates shuffle over the interpreter RNG,
/// returning a new list (ORO).
fn b_randomize(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut v = match args.first() {
        Some(Value::List(l)) => l.as_ref().clone(),
        Some(Value::Nil) | None => return Ok(Value::list(Vec::new())),
        _ => return Err(Signal::error("randomize: expected a list")),
    };
    for k in (1..v.len()).rev() {
        let j = (i.rng_next_u64() % (k as u64 + 1)) as usize;
        v.swap(k, j);
    }
    Ok(Value::list(v))
}

fn b_starts_with(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(p))) => Ok(boolean(s.starts_with(p.as_slice()))),
        (Some(Value::List(l)), Some(pre)) => {
            Ok(boolean(l.first().is_some_and(|x| values_equal(x, pre))))
        }
        _ => Ok(Value::Nil),
    }
}

fn b_ends_with(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match (args.first(), args.get(1)) {
        (Some(Value::Str(s)), Some(Value::Str(p))) => Ok(boolean(s.ends_with(p.as_slice()))),
        _ => Ok(Value::Nil),
    }
}

/// `(upper-case str)` / `(lower-case str)` — Unicode case folding over the byte
/// string (ADR-0028): each valid UTF-8 character is mapped with Rust's Unicode
/// default case mapping (so `ß` -> `SS`, Cyrillic folds, etc.); an invalid
/// (non-UTF-8) byte passes through unchanged. ASCII is unaffected. New string.
fn unicode_case(args: &[Value], upper: bool) -> Result<Value, Signal> {
    let bytes = match args.first() {
        Some(Value::Str(b)) => b,
        _ => return Err(Signal::error("upper-case/lower-case: expected a string")),
    };
    let mut out = Vec::with_capacity(bytes.len());
    let mut buf = [0u8; 4];
    for (s, e) in crate::utf8::char_ranges(bytes) {
        let chunk = &bytes[s..e];
        match std::str::from_utf8(chunk) {
            Ok(valid) => {
                for c in valid.chars() {
                    if upper {
                        c.to_uppercase().for_each(|u| {
                            out.extend_from_slice(u.encode_utf8(&mut buf).as_bytes())
                        });
                    } else {
                        c.to_lowercase().for_each(|l| {
                            out.extend_from_slice(l.encode_utf8(&mut buf).as_bytes())
                        });
                    }
                }
            }
            // A lenient one-byte "character" that is not valid UTF-8 (ADR-0025).
            Err(_) => out.extend_from_slice(chunk),
        }
    }
    Ok(Value::str(out))
}

fn b_upper_case(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    unicode_case(args, true)
}

fn b_lower_case(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    unicode_case(args, false)
}

/// `(trim str)` — strip leading/trailing spaces; `(trim str ch)` — strip `ch`
/// from both ends; `(trim str l r)` — strip `l` from the left, `r` from the
/// right. Trim characters are single-byte (ADR-0013). Returns a new string.
fn b_trim(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let s = match args.first() {
        Some(Value::Str(b)) => b,
        _ => return Err(Signal::error("trim: expected a string")),
    };
    let first_byte = |v: &Value| -> Result<u8, Signal> {
        match v {
            Value::Str(b) if !b.is_empty() => Ok(b[0]),
            _ => Err(Signal::error("trim: expected a single-character string")),
        }
    };
    let (lc, rc) = match (args.get(1), args.get(2)) {
        (None, _) => (b' ', b' '),
        (Some(c), None) => {
            let x = first_byte(c)?;
            (x, x)
        }
        (Some(l), Some(r)) => (first_byte(l)?, first_byte(r)?),
    };
    let start = s.iter().position(|&b| b != lc).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&b| b != rc)
        .map_or(start, |i| (i + 1).max(start));
    Ok(Value::str(s[start..end].to_vec()))
}

/// Resolve `(start, end)` byte/element indices for `slice` (newLISP semantics):
/// a negative `start` counts from the end; a negative `length` leaves that many
/// elements off the end; an omitted `length` runs to the end.
fn slice_bounds(len: i64, start: i64, length: Option<i64>) -> (usize, usize) {
    let s = if start < 0 {
        (len + start).max(0)
    } else {
        start.min(len)
    };
    let e = match length {
        None => len,
        Some(l) if l >= 0 => (s + l).min(len),
        Some(l) => len + l, // negative: counted from the end
    };
    let e = e.clamp(s, len);
    (s as usize, e as usize)
}

/// `(slice seq start [length])` — a copied sub-range of a string (bytes) or list.
fn b_slice(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let seq = args
        .first()
        .ok_or_else(|| Signal::error("slice: missing sequence"))?;
    let start = to_i64(
        args.get(1)
            .ok_or_else(|| Signal::error("slice: missing start"))?,
    )?;
    let length = match args.get(2) {
        Some(v) => Some(to_i64(v)?),
        None => None,
    };
    match seq {
        Value::Str(b) => {
            let (s, e) = slice_bounds(b.len() as i64, start, length);
            Ok(Value::str(b[s..e].to_vec()))
        }
        Value::List(l) => {
            let (s, e) = slice_bounds(l.len() as i64, start, length);
            Ok(Value::list(l[s..e].to_vec()))
        }
        Value::Nil => Ok(Value::Nil),
        _ => Err(Signal::error("slice: expected a string or list")),
    }
}

/// `(find key seq)` — index of `key` in `seq`, else `nil`. For strings, `key` is
/// a substring and the result is a byte offset (ADR-0013); for lists, `key` is
/// an element compared structurally.
fn b_find(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let key = args
        .first()
        .ok_or_else(|| Signal::error("find: missing key"))?;
    let seq = args
        .get(1)
        .ok_or_else(|| Signal::error("find: missing sequence"))?;
    match (key, seq) {
        (Value::Str(k), Value::Str(s)) => {
            if k.is_empty() {
                return Ok(Value::Int(0));
            }
            match s.windows(k.len()).position(|w| w == k.as_slice()) {
                Some(i) => Ok(Value::Int(i as i64)),
                None => Ok(Value::Nil),
            }
        }
        (_, Value::List(l)) => match l.iter().position(|x| values_equal(x, key)) {
            Some(i) => Ok(Value::Int(i as i64)),
            None => Ok(Value::Nil),
        },
        _ => Err(Signal::error(
            "find: expected a string in a string, or an item in a list",
        )),
    }
}

/// `(chop seq [n])` — a copy of a string or list without its last `n` (default
/// 1) bytes / elements.
fn b_chop(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.get(1) {
        Some(v) => to_i64(v)?.max(0) as usize,
        None => 1,
    };
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::str(b[..b.len().saturating_sub(n)].to_vec())),
        Some(Value::List(l)) => Ok(Value::list(l[..l.len().saturating_sub(n)].to_vec())),
        Some(Value::Nil) | None => Ok(Value::Nil),
        _ => Err(Signal::error("chop: expected a string or list")),
    }
}

/// `(explode seq [n])` — split into a list of `n`-wide pieces (default 1). A
/// string splits on **character** boundaries (ADR-0025), so `(explode "abc")` ->
/// `("a" "b" "c")` and a multi-byte character stays whole; a list splits by
/// element.
fn b_explode(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.get(1) {
        Some(v) => to_i64(v)?.max(1) as usize,
        None => 1,
    };
    match args.first() {
        Some(Value::Str(b)) => {
            // Group the character byte-ranges into chunks of `n` characters.
            let ranges: Vec<(usize, usize)> = crate::utf8::char_ranges(b).collect();
            let pieces = ranges
                .chunks(n)
                .map(|chunk| {
                    let (start, end) = (chunk[0].0, chunk[chunk.len() - 1].1);
                    Value::str(b[start..end].to_vec())
                })
                .collect();
            Ok(Value::list(pieces))
        }
        Some(Value::List(l)) => Ok(Value::list(
            l.chunks(n).map(|c| Value::list(c.to_vec())).collect(),
        )),
        Some(Value::Nil) | None => Ok(Value::list(Vec::new())),
        _ => Err(Signal::error("explode: expected a string or list")),
    }
}

/// `(main-args)` — the process command line as a list of strings; `(main-args i)`
/// returns the `i`th (a negative `i` counts from the end), else `nil`.
fn b_main_args(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let all = interp.main_args();
    match args.first() {
        None => Ok(Value::list(
            all.into_iter()
                .map(|s| Value::str(s.into_bytes()))
                .collect(),
        )),
        Some(v) => {
            let i = to_i64(v)?;
            let idx = if i < 0 { all.len() as i64 + i } else { i };
            match usize::try_from(idx).ok().and_then(|k| all.get(k)) {
                Some(s) => Ok(Value::str(s.clone().into_bytes())),
                None => Ok(Value::Nil),
            }
        }
    }
}

/// `(seed n)` — reseed the RNG, returning the previous seed.
fn b_seed(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = to_i64(args.first().unwrap_or(&Value::Nil))?;
    Ok(Value::Int(interp.rng_seed(n as u64) as i64))
}

/// `(rand max [count])` — a random integer in `[0, max)`, or a list of `count`.
fn b_rand(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let max = to_i64(args.first().unwrap_or(&Value::Nil))?;
    if max <= 0 {
        return Ok(Value::Int(0));
    }
    let m = max as u64;
    let draw = |i: &Interp| Value::Int((i.rng_next_u64() % m) as i64);
    match args.get(1) {
        Some(cnt) => {
            let n = to_i64(cnt)?.max(0) as usize;
            Ok(Value::list((0..n).map(|_| draw(interp)).collect()))
        }
        None => Ok(draw(interp)),
    }
}

/// `(random)` — a uniform float in `[0, 1)`; `(random offset scale [count])` — a
/// uniform float in `[offset, offset+scale)`, or a list of `count` of them.
fn b_random(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let uni = |i: &Interp| i.rng_next_u64() as f64 / (u64::MAX as f64 + 1.0);
    match (args.first(), args.get(1), args.get(2)) {
        (None, _, _) => Ok(Value::Float(uni(interp))),
        (Some(off), Some(scale), None) => {
            Ok(Value::Float(to_f64(off)? + to_f64(scale)? * uni(interp)))
        }
        (Some(off), Some(scale), Some(cnt)) => {
            let (o, s) = (to_f64(off)?, to_f64(scale)?);
            let n = to_i64(cnt)?.max(0) as usize;
            Ok(Value::list(
                (0..n).map(|_| Value::Float(o + s * uni(interp))).collect(),
            ))
        }
        (Some(_), None, _) => Err(Signal::error(
            "random: expected (random) or (random offset scale [count])",
        )),
    }
}

/// The extreme (smallest for `Less`, largest for `Greater`) numeric argument,
/// preserving its type. NaN operands are skipped.
fn extreme(args: &[Value], name: &str, want: Ordering) -> Result<Value, Signal> {
    let mut best = args
        .first()
        .ok_or_else(|| Signal::error(format!("{}: expected at least one argument", name)))?;
    if as_f64_opt(best).is_none() {
        return Err(Signal::error(format!("{}: expected numbers", name)));
    }
    for a in &args[1..] {
        match compare_num(a, best) {
            Some(Some(o)) if o == want => best = a,
            Some(_) => {}
            None => return Err(Signal::error(format!("{}: expected numbers", name))),
        }
    }
    Ok(best.clone())
}

fn b_min(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    extreme(args, "min", Ordering::Less)
}

fn b_max(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    extreme(args, "max", Ordering::Greater)
}

/// Parity test for `even?` / `odd?`. Floats are truncated to integer first.
fn parity_even(v: &Value) -> Result<bool, Signal> {
    match v {
        Value::Int(n) => Ok(n % 2 == 0),
        Value::Float(f) => Ok((*f as i64) % 2 == 0),
        // Parity of a bigint is the parity of its low magnitude digit.
        #[cfg(feature = "bigint")]
        Value::Bigint(b) => Ok(b.to_u64_digits().1.first().copied().unwrap_or(0) % 2 == 0),
        _ => Err(Signal::error("even?/odd?: expected an integer")),
    }
}

fn is_even(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(parity_even(args.first().unwrap_or(&Value::Nil))?))
}

fn is_odd(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(boolean(!parity_even(args.first().unwrap_or(&Value::Nil))?))
}

/// `(flat lst)` — flatten a nested list into a single-level list.
fn b_flat(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    fn go(v: &Value, out: &mut Vec<Value>) {
        match v {
            Value::List(items) => items.iter().for_each(|it| go(it, out)),
            other => out.push(other.clone()),
        }
    }
    match args.first() {
        Some(v @ Value::List(_)) => {
            let mut out = Vec::new();
            go(v, &mut out);
            Ok(Value::list(out))
        }
        Some(Value::Nil) | None => Ok(Value::list(Vec::new())),
        _ => Err(Signal::error("flat: expected a list")),
    }
}

/// `(join list-of-strings [separator])` — concatenate strings with an optional
/// separator between them.
fn b_join(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let list = match args.first() {
        Some(Value::List(l)) => l,
        _ => return Err(Signal::error("join: expected a list of strings")),
    };
    let sep: &[u8] = match args.get(1) {
        Some(Value::Str(s)) => s,
        None => b"",
        _ => return Err(Signal::error("join: separator must be a string")),
    };
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(sep);
        }
        match item {
            Value::Str(b) => out.extend_from_slice(b),
            _ => return Err(Signal::error("join: list elements must be strings")),
        }
    }
    Ok(Value::str(out))
}

/// `(member key seq)` — the tail of a list from the first element equal to `key`
/// (structurally), or the substring of a string from the first match; else `nil`.
fn b_member(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let key = args
        .first()
        .ok_or_else(|| Signal::error("member: missing key"))?;
    match args.get(1) {
        Some(Value::List(l)) => match l.iter().position(|x| values_equal(x, key)) {
            Some(i) => Ok(Value::list(l[i..].to_vec())),
            None => Ok(Value::Nil),
        },
        Some(Value::Str(s)) => {
            let k = match key {
                Value::Str(k) => k,
                _ => {
                    return Err(Signal::error(
                        "member: a string haystack needs a string key",
                    ))
                }
            };
            if k.is_empty() {
                return Ok(Value::Str(s.clone()));
            }
            match s.windows(k.len()).position(|w| w == k.as_slice()) {
                Some(i) => Ok(Value::str(s[i..].to_vec())),
                None => Ok(Value::Nil),
            }
        }
        _ => Err(Signal::error("member: expected a list or string")),
    }
}

/// `(unique lst)` — a copy of the list with duplicate elements removed, keeping
/// the first occurrence of each.
fn b_unique(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) => {
            let mut out: Vec<Value> = Vec::new();
            for item in l.iter() {
                if !out.iter().any(|x| values_equal(x, item)) {
                    out.push(item.clone());
                }
            }
            Ok(Value::list(out))
        }
        Some(Value::Nil) | None => Ok(Value::list(Vec::new())),
        _ => Err(Signal::error("unique: expected a list")),
    }
}

/// `(array size [init])` — a fixed-length array (ADR-0023). With no `init` it is
/// nil-filled; with an `init` list it is cycle-filled to `size` elements. Leading
/// integer arguments are dimensions; two or more is an error (multi-dimensional
/// arrays are deferred).
fn b_array(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let mut dims: Vec<i64> = Vec::new();
    let mut init: Option<&Vec<Value>> = None;
    let last = args.len().wrapping_sub(1);
    for (k, a) in args.iter().enumerate() {
        match a {
            Value::Int(n) => dims.push(*n),
            Value::List(l) if k == last => init = Some(l),
            _ => {
                return Err(Signal::error(
                    "array: dimensions must be integers and the optional init a list",
                ))
            }
        }
    }
    match dims.len() {
        0 => return Err(Signal::error("array: expected a size")),
        1 => {}
        _ => {
            return Err(Signal::error(
                "array: multi-dimensional arrays not yet supported",
            ))
        }
    }
    let size = dims[0].max(0) as usize;
    let elems = match init {
        Some(l) if !l.is_empty() => (0..size).map(|i| l[i % l.len()].clone()).collect(),
        _ => vec![Value::Nil; size],
    };
    Ok(Value::array(elems))
}

/// `(array-list arr)` — a plain list copy of an array's elements.
fn b_array_list(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Array(a)) => Ok(Value::List(a.clone())),
        _ => Err(Signal::error("array-list: expected an array")),
    }
}

fn b_exit(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let code = match args.first() {
        Some(Value::Int(n)) => *n as i32,
        _ => 0,
    };
    std::process::exit(code);
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Nil => "nil",
        Value::True => "true",
        Value::Int(_) => "integer",
        Value::Float(_) => "float",
        Value::Str(_) => "string",
        Value::Symbol(_) => "symbol",
        Value::Context(_) => "context",
        Value::List(_) => "list",
        Value::Array(_) => "array",
        Value::Lambda(_) => "lambda",
        Value::Fexpr(_) => "lambda-macro",
        Value::Builtin(_) => "builtin",
        Value::Foreign(_) => "foreign",
        #[cfg(feature = "bigint")]
        Value::Bigint(_) => "bigint",
    }
}
