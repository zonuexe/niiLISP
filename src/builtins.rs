//! Primitive functions.
//!
//! Integer arithmetic (`+ - * / %`) wraps on overflow (ADR-0012); float
//! arithmetic uses `add sub mul div`, matching newLISP's split. Strings are
//! byte buffers, so `length` counts bytes (ADR-0013).

use std::cmp::Ordering;
use std::io::Write;

use crate::eval::{Interp, Signal};
use crate::printer::to_display;
use crate::value::{Builtin, BuiltinFn, Value};

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
    reg("atom?", is_atom);
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
    reg("starts-with", b_starts_with);
    reg("ends-with", b_ends_with);
    reg("upper-case", b_upper_case);
    reg("lower-case", b_lower_case);
    reg("trim", b_trim);
    reg("slice", b_slice);
    reg("find", b_find);
    reg("chop", b_chop);
    reg("explode", b_explode);
    // RNG and process environment.
    reg("seed", b_seed);
    reg("rand", b_rand);
    reg("random", b_random);
    reg("main-args", b_main_args);
    reg("set-locale", |_, _| Ok(Value::Str(b"C".to_vec())));
    reg("print", b_print);
    reg("println", b_println);
    reg("string", b_string);
    reg("exit", b_exit);
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
            Ok(Value::Str(s))
        }
        Some(Value::Str(b)) => match String::from_utf8_lossy(b).chars().next() {
            Some(c) => Ok(Value::Int(c as i64)),
            None => Ok(Value::Nil),
        },
        _ => Err(Signal::error("char: expected an integer or string")),
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
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| values_equal(p, q))
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
    Ok(Value::List(args.to_vec()))
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
            Ok(Value::List(out))
        }
        other => Ok(Value::List(vec![args[0].clone(), other.clone()])),
    }
}

fn b_first(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) => l
            .first()
            .cloned()
            .ok_or_else(|| Signal::error("first: empty list")),
        Some(Value::Str(b)) if !b.is_empty() => Ok(Value::Str(vec![b[0]])),
        _ => Err(Signal::error("first: expected a non-empty list or string")),
    }
}

fn b_rest(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) => Ok(Value::List(l.get(1..).unwrap_or(&[]).to_vec())),
        Some(Value::Str(b)) => Ok(Value::Str(b.get(1..).unwrap_or(&[]).to_vec())),
        _ => Err(Signal::error("rest: expected a list or string")),
    }
}

fn b_last(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::List(l)) => l
            .last()
            .cloned()
            .ok_or_else(|| Signal::error("last: empty list")),
        _ => Err(Signal::error("last: expected a list")),
    }
}

fn b_nth(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.len() != 2 {
        return Err(Signal::error("nth: expected (nth index list)"));
    }
    let idx = to_i64(&args[0])?;
    match &args[1] {
        Value::List(l) => {
            let i = if idx < 0 { l.len() as i64 + idx } else { idx };
            if i < 0 || i as usize >= l.len() {
                Ok(Value::Nil)
            } else {
                Ok(l[i as usize].clone())
            }
        }
        _ => Err(Signal::error("nth: expected a list")),
    }
}

fn b_length(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.first() {
        Some(Value::List(l)) => l.len() as i64,
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
        return Ok(Value::Str(out));
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
    Ok(Value::List(out))
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
    Ok(Value::List(out))
}

fn b_map(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let f = args
        .first()
        .ok_or_else(|| Signal::error("map: missing function"))?;
    let lists: Vec<&Vec<Value>> = args[1..]
        .iter()
        .map(|v| match v {
            Value::List(l) => Ok(l),
            _ => Err(Signal::error("map: expected list arguments")),
        })
        .collect::<Result<_, _>>()?;
    if lists.is_empty() {
        return Ok(Value::List(Vec::new()));
    }
    let n = lists.iter().map(|l| l.len()).min().unwrap_or(0);
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let call_args: Vec<Value> = lists.iter().map(|l| l[k].clone()).collect();
        out.push(i.call(f, call_args)?);
    }
    Ok(Value::List(out))
}

fn b_apply(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let f = args
        .first()
        .ok_or_else(|| Signal::error("apply: missing function"))?;
    let call_args = match args.get(1) {
        Some(Value::List(l)) => l.clone(),
        Some(other) => vec![other.clone()],
        None => Vec::new(),
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
    for item in list {
        if i.call(pred, vec![item.clone()])?.is_truthy() {
            out.push(item.clone());
        }
    }
    Ok(Value::List(out))
}

fn b_dup(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.get(1) {
        Some(Value::Int(n)) => (*n).max(0) as usize,
        _ => return Err(Signal::error("dup: expected (dup value count)")),
    };
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::Str(b.repeat(n))),
        Some(v) => Ok(Value::List(vec![v.clone(); n])),
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
    Ok(boolean(!matches!(args.first(), Some(Value::List(_)))))
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
    Ok(Value::Str(out))
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
    Ok(Value::Str(out))
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
        for item in items {
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
        for item in items {
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

fn b_new(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    // (new prototype 'name) — create a context. Prototype is ignored for now.
    let name = match args.get(1).or_else(|| args.first()) {
        Some(Value::Symbol(id)) | Some(Value::Context(id)) => *id,
        _ => return Err(Signal::error("new: expected a context name symbol")),
    };
    i.set_global(name, Value::Context(name));
    Ok(Value::Context(name))
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

/// `(upper-case str)` / `(lower-case str)` — ASCII case conversion, byte by
/// byte; bytes >= 0x80 are left unchanged (full UTF-8 case folding is deferred,
/// ADR-0013). Returns a new string.
fn ascii_case(args: &[Value], upper: bool) -> Result<Value, Signal> {
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::Str(
            b.iter()
                .map(|&c| {
                    if upper {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect(),
        )),
        _ => Err(Signal::error("upper-case/lower-case: expected a string")),
    }
}

fn b_upper_case(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    ascii_case(args, true)
}

fn b_lower_case(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    ascii_case(args, false)
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
    Ok(Value::Str(s[start..end].to_vec()))
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
            Ok(Value::Str(b[s..e].to_vec()))
        }
        Value::List(l) => {
            let (s, e) = slice_bounds(l.len() as i64, start, length);
            Ok(Value::List(l[s..e].to_vec()))
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
        Some(Value::Str(b)) => Ok(Value::Str(b[..b.len().saturating_sub(n)].to_vec())),
        Some(Value::List(l)) => Ok(Value::List(l[..l.len().saturating_sub(n)].to_vec())),
        Some(Value::Nil) | None => Ok(Value::Nil),
        _ => Err(Signal::error("chop: expected a string or list")),
    }
}

/// `(explode seq [n])` — split a string (by byte) or list into a list of `n`-wide
/// pieces (default 1). `(explode "abc")` -> `("a" "b" "c")`.
fn b_explode(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let n = match args.get(1) {
        Some(v) => to_i64(v)?.max(1) as usize,
        None => 1,
    };
    match args.first() {
        Some(Value::Str(b)) => Ok(Value::List(
            b.chunks(n).map(|c| Value::Str(c.to_vec())).collect(),
        )),
        Some(Value::List(l)) => Ok(Value::List(
            l.chunks(n).map(|c| Value::List(c.to_vec())).collect(),
        )),
        Some(Value::Nil) | None => Ok(Value::List(Vec::new())),
        _ => Err(Signal::error("explode: expected a string or list")),
    }
}

/// `(main-args)` — the process command line as a list of strings; `(main-args i)`
/// returns the `i`th (a negative `i` counts from the end), else `nil`.
fn b_main_args(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let all = interp.main_args();
    match args.first() {
        None => Ok(Value::List(
            all.into_iter()
                .map(|s| Value::Str(s.into_bytes()))
                .collect(),
        )),
        Some(v) => {
            let i = to_i64(v)?;
            let idx = if i < 0 { all.len() as i64 + i } else { i };
            match usize::try_from(idx).ok().and_then(|k| all.get(k)) {
                Some(s) => Ok(Value::Str(s.clone().into_bytes())),
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
            Ok(Value::List((0..n).map(|_| draw(interp)).collect()))
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
            Ok(Value::List(
                (0..n).map(|_| Value::Float(o + s * uni(interp))).collect(),
            ))
        }
        (Some(_), None, _) => Err(Signal::error(
            "random: expected (random) or (random offset scale [count])",
        )),
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
        Value::Lambda(_) => "lambda",
        Value::Fexpr(_) => "lambda-macro",
        Value::Builtin(_) => "builtin",
        Value::Foreign(_) => "foreign",
        #[cfg(feature = "bigint")]
        Value::Bigint(_) => "bigint",
    }
}
