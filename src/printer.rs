//! Rendering values to text.
//!
//! `print`/`println` show strings as raw text; `display` (used for the REPL
//! and `string` on structured values) shows a re-readable form with quotes and
//! braces.

use crate::value::{Interner, Value};

/// Human-facing form: strings are shown verbatim (no surrounding quotes).
/// This is what `print` and `println` emit.
pub fn to_display(v: &Value, it: &Interner) -> String {
    match v {
        Value::Str(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        _ => to_repr(v, it),
    }
}

/// Re-readable form: strings are quoted, lists parenthesised.
pub fn to_repr(v: &Value, it: &Interner) -> String {
    match v {
        Value::Nil => "nil".to_string(),
        Value::True => "true".to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => format_float(*f),
        Value::Str(bytes) => format!("\"{}\"", String::from_utf8_lossy(bytes)),
        Value::Symbol(id) => it.name(*id).to_string(),
        Value::Context(id) => it.name(*id).to_string(),
        Value::List(items) => {
            let mut out = String::from("(");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&to_repr(item, it));
            }
            out.push(')');
            out
        }
        Value::Lambda(l) => format!("(lambda ({} args) ...)", l.params.len()),
        Value::Fexpr(l) => format!("(lambda-macro ({} args) ...)", l.params.len()),
        Value::Builtin(b) => format!("<builtin:{}>", b.name),
        Value::Foreign(f) => format!("<foreign:{}>", f.name),
    }
}

fn format_float(f: f64) -> String {
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if f.is_nan() {
        return "nan".into();
    }
    // `{}` prints integral doubles without a trailing ".0" (e.g. 25.0 -> "25"),
    // matching newLISP's compact float output closely enough for now.
    format!("{}", f)
}
