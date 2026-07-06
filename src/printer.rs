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
        Value::Str(bytes) => repr_string(bytes),
        Value::Symbol(id) => it.name(*id).to_string(),
        Value::Context(id) => it.name(*id).to_string(),
        // An array prints exactly like a list (ADR-0023).
        Value::List(items) | Value::Array(items) => {
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
        // A lambda prints as its list form (ADR-0027): (lambda (params…) body…).
        Value::Lambda(l) => format_lambda("lambda", l, it),
        Value::Fexpr(l) => format_lambda("lambda-macro", l, it),
        Value::Builtin(b) => format!("<builtin:{}>", b.name),
        Value::Foreign(f) => format!("<foreign:{}>", f.name),
        // Plain decimal, no `L` suffix — the suffix is lexical only (ADR-0022).
        #[cfg(feature = "bigint")]
        Value::Bigint(n) => n.to_string(),
    }
}

/// Render a byte string as a re-readable double-quoted literal (ADR-0032): `"`,
/// `\`, and the whitespace controls get named escapes, other control/invalid
/// bytes get a zero-padded `\NNN` decimal escape (so a following digit is not
/// absorbed), and valid UTF-8 text stays literal and readable. The reader's
/// `\n`/`\t`/`\r`/`\\`/`\"`/`\ddd` handling parses all of these back.
fn repr_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('"');
    let push_escaped = |out: &mut String, b: u8| match b {
        b'"' => out.push_str("\\\""),
        b'\\' => out.push_str("\\\\"),
        b'\n' => out.push_str("\\n"),
        b'\t' => out.push_str("\\t"),
        b'\r' => out.push_str("\\r"),
        _ => out.push_str(&format!("\\{:03}", b)),
    };
    match std::str::from_utf8(bytes) {
        // Readable UTF-8: keep multi-byte characters literal; escape only quotes,
        // backslashes, and controls.
        Ok(s) => {
            for c in s.chars() {
                match c {
                    '"' | '\\' | '\n' | '\t' | '\r' => push_escaped(&mut out, c as u8),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\{:03}", c as u32)),
                    c => out.push(c),
                }
            }
        }
        // Binary / invalid UTF-8: byte-wise, printable ASCII literal, rest escaped.
        Err(_) => {
            for &b in bytes {
                match b {
                    0x20..=0x7e => {
                        if b == b'"' || b == b'\\' {
                            push_escaped(&mut out, b);
                        } else {
                            out.push(b as char);
                        }
                    }
                    _ => push_escaped(&mut out, b),
                }
            }
        }
    }
    out.push('"');
    out
}

/// Render a lambda/fexpr as its list form `(head (params…) body…)` (ADR-0027).
fn format_lambda(head: &str, l: &crate::value::Lambda, it: &Interner) -> String {
    let mut out = format!("({} (", head);
    for (i, p) in l.params.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        match &p.default {
            None => out.push_str(it.name(p.sym)),
            Some(d) => out.push_str(&format!("({} {})", it.name(p.sym), to_repr(d, it))),
        }
    }
    out.push(')');
    for b in &l.body {
        out.push(' ');
        out.push_str(&to_repr(b, it));
    }
    out.push(')');
    out
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
