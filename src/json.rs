//! JSON parsing (ADR-0038): `json-parse` / `json-error`. A hand-written
//! recursive-descent parser (ECMA-262) that emits newLISP's S-expression
//! representation directly: objects become association lists `(("k" v) …)`,
//! arrays become lists, `true` becomes the true value, and `false`/`null` become
//! the symbols `false`/`null`. On malformed input it returns `nil` and records a
//! `("message" position)` pair for `json-error`.

use crate::eval::{Interp, Signal};
use crate::value::Value;

pub fn install(interp: &Interp) {
    interp.register_builtin("json-parse", b_json_parse);
    interp.register_builtin("json-error", b_json_error);
}

fn b_json_parse(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let bytes = match args.first() {
        Some(Value::Str(b)) => b.clone(),
        _ => return Err(Signal::error("json-parse: expected a string")),
    };
    let mut p = Parser {
        s: &bytes,
        pos: 0,
        i,
    };
    p.skip_ws();
    match p.parse_value() {
        Ok(v) => {
            p.skip_ws();
            if p.pos != p.s.len() {
                i.set_json_error(err_value("extra data after JSON value", p.pos));
                Ok(Value::Nil)
            } else {
                i.set_json_error(Value::Nil);
                Ok(v)
            }
        }
        Err((msg, pos)) => {
            i.set_json_error(err_value(msg, pos));
            Ok(Value::Nil)
        }
    }
}

fn b_json_error(i: &Interp, _args: &[Value]) -> Result<Value, Signal> {
    Ok(i.get_json_error())
}

fn err_value(msg: &str, pos: usize) -> Value {
    Value::list(vec![
        Value::str(msg.as_bytes().to_vec()),
        Value::Int(pos as i64),
    ])
}

type PResult = Result<Value, (&'static str, usize)>;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    i: &'a Interp,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> PResult {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::str(self.parse_string()?)),
            Some(b't') => self.parse_lit(b"true", Value::True),
            Some(b'f') => {
                let f = Value::Symbol(self.i.intern("false"));
                self.parse_lit(b"false", f)
            }
            Some(b'n') => {
                let n = Value::Symbol(self.i.intern("null"));
                self.parse_lit(b"null", n)
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            _ => Err(("unexpected token", self.pos)),
        }
    }

    fn parse_lit(&mut self, lit: &[u8], val: Value) -> PResult {
        if self.s[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(val)
        } else {
            Err(("invalid literal", self.pos))
        }
    }

    fn parse_object(&mut self) -> PResult {
        self.pos += 1; // '{'
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::list(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(("expected string key", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(("missing : colon", self.pos));
            }
            self.pos += 1;
            let val = self.parse_value()?;
            pairs.push(Value::list(vec![Value::str(key), val]));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::list(pairs));
                }
                _ => return Err(("expected , or }", self.pos)),
            }
        }
    }

    fn parse_array(&mut self) -> PResult {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::list(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::list(items));
                }
                _ => return Err(("expected , or ]", self.pos)),
            }
        }
    }

    fn parse_string(&mut self) -> Result<Vec<u8>, (&'static str, usize)> {
        self.pos += 1; // opening quote
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return Err(("unterminated string", self.pos)),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => out.push(b'"'),
                        Some(b'\\') => out.push(b'\\'),
                        Some(b'/') => out.push(b'/'),
                        Some(b'b') => out.push(0x08),
                        Some(b'f') => out.push(0x0c),
                        Some(b'n') => out.push(b'\n'),
                        Some(b'r') => out.push(b'\r'),
                        Some(b't') => out.push(b'\t'),
                        Some(b'u') => {
                            let cp = self.parse_hex4()?;
                            // Surrogate pair for astral code points.
                            let scalar = if (0xD800..=0xDBFF).contains(&cp) {
                                if self.s[self.pos..].starts_with(b"\\u") {
                                    self.pos += 2;
                                    let lo = self.parse_hex4()?;
                                    0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    cp
                                }
                            } else {
                                cp
                            };
                            let ch = char::from_u32(scalar).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            continue;
                        }
                        _ => return Err(("invalid escape", self.pos)),
                    }
                    self.pos += 1;
                }
                Some(c) => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, (&'static str, usize)> {
        self.pos += 1; // consume 'u'
        if self.pos + 4 > self.s.len() {
            return Err(("bad \\u escape", self.pos));
        }
        let hex = &self.s[self.pos..self.pos + 4];
        let mut v: u32 = 0;
        for &b in hex {
            let d = (b as char)
                .to_digit(16)
                .ok_or(("bad \\u escape", self.pos))?;
            v = v * 16 + d;
        }
        self.pos += 4;
        Ok(v)
    }

    fn parse_number(&mut self) -> PResult {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.pos += 1,
                b'.' | b'e' | b'E' | b'+' | b'-' => {
                    is_float = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let tok = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("");
        if is_float {
            tok.parse::<f64>()
                .map(Value::Float)
                .map_err(|_| ("invalid number", start))
        } else {
            match tok.parse::<i64>() {
                Ok(n) => Ok(Value::Int(n)),
                // Out of i64 range falls back to a float, as newLISP does.
                Err(_) => tok
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| ("invalid number", start)),
            }
        }
    }
}
