//! The reader: turns source bytes into `Value`s (ADR-0012).
//!
//! Reproduces newLISP's lexical surface: three string syntaxes (`"..."`,
//! `{...}`, `[text]...[/text]`), `'` quote, `;` and `#` line comments, symbols
//! that may contain `.`/`:`, and 64-bit int / double numbers. `L`-suffixed
//! bigint literals are recognised as reserved syntax and rejected with a clear
//! error (bigint is deferred past v1).

use std::collections::HashSet;

use crate::value::{Interner, SymId, Value};

pub struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
    interner: &'a mut Interner,
    /// The MAIN primitive names kept unqualified inside a context (ADR-0026).
    primitives: &'a HashSet<String>,
    /// The current context: bare symbols read while this is not `MAIN` are
    /// qualified with it. Switched by top-level `(context …)` forms.
    current_ctx: String,
    /// Set while reading the arguments of a `(context …)` form, so a context
    /// name is read unqualified (context names are MAIN-level).
    suppress_qualify: bool,
}

impl<'a> Reader<'a> {
    pub fn new(src: &'a [u8], interner: &'a mut Interner, primitives: &'a HashSet<String>) -> Self {
        Reader {
            src,
            pos: 0,
            interner,
            primitives,
            current_ctx: "MAIN".to_string(),
            suppress_qualify: false,
        }
    }

    /// Read every top-level form in the source. A top-level `(context …)` form
    /// switches the current context for the forms that follow (ADR-0026).
    pub fn read_all(&mut self) -> Result<Vec<Value>, String> {
        let mut forms = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.pos >= self.src.len() {
                break;
            }
            let form = self.read_form()?;
            if let Some(ctx) = self.context_switch(&form) {
                self.current_ctx = ctx;
            }
            forms.push(form);
        }
        Ok(forms)
    }

    /// If `form` is a `(context 'X)` / `(context X)` call, the new current
    /// context name (the term, since context names are MAIN-level).
    fn context_switch(&self, form: &Value) -> Option<String> {
        let items = match form {
            Value::List(items) if items.len() == 2 => items,
            _ => return None,
        };
        match &items[0] {
            Value::Symbol(id) if self.interner.name(*id) == "context" => {}
            _ => return None,
        }
        let name_sym = match &items[1] {
            Value::Symbol(id) => *id,
            // (context 'X) -> (quote X)
            Value::List(q)
                if q.len() == 2
                    && matches!(&q[0], Value::Symbol(s) if self.interner.name(*s) == "quote") =>
            {
                match &q[1] {
                    Value::Symbol(id) => *id,
                    _ => return None,
                }
            }
            _ => return None,
        };
        let name = self.interner.name(name_sym);
        Some(name.rsplit(':').next().unwrap_or(name).to_string())
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn skip_ws_and_comments(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\r' | b'\n' | b',' => {
                    self.pos += 1;
                }
                b';' | b'#' => {
                    // Line comment to end of line (covers `#!` shebang too).
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn read_form(&mut self) -> Result<Value, String> {
        self.skip_ws_and_comments();
        match self.peek() {
            None => Err("unexpected end of input".to_string()),
            Some(b'(') => self.read_list(),
            Some(b')') => Err("unexpected ')'".to_string()),
            Some(b'\'') => {
                self.pos += 1;
                let quoted = self.read_form()?;
                let q = self.interner.intern("quote");
                Ok(Value::list(vec![Value::Symbol(q), quoted]))
            }
            Some(b'"') => self.read_dquote_string(),
            Some(b'{') => self.read_brace_string(),
            Some(b'[') => self.read_maybe_tag_string(),
            Some(_) => self.read_atom(),
        }
    }

    fn read_list(&mut self) -> Result<Value, String> {
        self.pos += 1; // consume '('
        let saved_suppress = self.suppress_qualify;
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments();
            match self.peek() {
                None => {
                    self.suppress_qualify = saved_suppress;
                    return Err("unterminated list: missing ')'".to_string());
                }
                Some(b')') => {
                    self.pos += 1;
                    self.suppress_qualify = saved_suppress;
                    return Ok(Value::list(items));
                }
                Some(_) => {
                    let item = self.read_form()?;
                    // A `(context …)` form's arguments name a MAIN-level context,
                    // so read them unqualified (ADR-0026).
                    if items.is_empty() {
                        if let Value::Symbol(id) = &item {
                            if self.interner.name(*id) == "context" {
                                self.suppress_qualify = true;
                            }
                        }
                    }
                    items.push(item);
                }
            }
        }
    }

    fn read_dquote_string(&mut self) -> Result<Value, String> {
        self.pos += 1; // consume opening '"'
        let mut bytes = Vec::new();
        while let Some(b) = self.bump() {
            match b {
                b'"' => return Ok(Value::str(bytes)),
                b'\\' => {
                    let e = self.bump().ok_or("unterminated string escape")?;
                    match e {
                        b'n' => bytes.push(b'\n'),
                        b't' => bytes.push(b'\t'),
                        b'r' => bytes.push(b'\r'),
                        b'\\' => bytes.push(b'\\'),
                        b'"' => bytes.push(b'"'),
                        b'0'..=b'9' => {
                            // `\ddd` decimal byte escape (e.g. \195), as in qa-utf8.
                            let mut n = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'9') => {
                                        n = n * 10 + (d - b'0') as u32;
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            bytes.push((n & 0xff) as u8);
                        }
                        other => bytes.push(other),
                    }
                }
                other => bytes.push(other),
            }
        }
        Err("unterminated string: missing '\"'".to_string())
    }

    /// Brace string `{...}`: raw, nestable, may span lines, no escapes.
    fn read_brace_string(&mut self) -> Result<Value, String> {
        self.pos += 1; // consume '{'
        let mut bytes = Vec::new();
        let mut depth = 1u32;
        while let Some(b) = self.bump() {
            match b {
                b'{' => {
                    depth += 1;
                    bytes.push(b);
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Value::str(bytes));
                    }
                    bytes.push(b);
                }
                other => bytes.push(other),
            }
        }
        Err("unterminated brace string: missing '}'".to_string())
    }

    /// `[text]...[/text]` tag string, or a bare `[` atom if not the tag opener.
    fn read_maybe_tag_string(&mut self) -> Result<Value, String> {
        const OPEN: &[u8] = b"[text]";
        const CLOSE: &[u8] = b"[/text]";
        if self.src[self.pos..].starts_with(OPEN) {
            self.pos += OPEN.len();
            let rest = &self.src[self.pos..];
            match find_subslice(rest, CLOSE) {
                Some(end) => {
                    let bytes = rest[..end].to_vec();
                    self.pos += end + CLOSE.len();
                    Ok(Value::str(bytes))
                }
                None => Err("unterminated tag string: missing [/text]".to_string()),
            }
        } else {
            // Not a tag string; treat '[' as an ordinary atom constituent.
            self.read_atom()
        }
    }

    fn read_atom(&mut self) -> Result<Value, String> {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if is_delimiter(b) {
                break;
            }
            self.pos += 1;
        }
        let tok = &self.src[start..self.pos];
        let text = String::from_utf8_lossy(tok).into_owned();
        self.classify_atom(&text)
    }

    fn classify_atom(&mut self, text: &str) -> Result<Value, String> {
        match text {
            "nil" => return Ok(Value::Nil),
            "true" => return Ok(Value::True),
            _ => {}
        }
        match parse_number(text) {
            NumParse::Num(v) => Ok(v),
            NumParse::Bigint(digits) => classify_bigint(&digits, text),
            NumParse::No => Ok(Value::Symbol(self.intern_symbol(text))),
        }
    }

    /// Intern a symbol name, qualifying it with the current context (ADR-0026)
    /// unless we are in `MAIN`, reading a context name, the token is already
    /// context-qualified, or it names a MAIN primitive.
    fn intern_symbol(&mut self, text: &str) -> SymId {
        if self.current_ctx != "MAIN"
            && !self.suppress_qualify
            && !text.contains(':')
            && !self.primitives.contains(text)
        {
            let qualified = format!("{}:{}", self.current_ctx, text);
            self.interner.intern(&qualified)
        } else {
            self.interner.intern(text)
        }
    }
}

fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\r' | b'\n' | b',' | b'(' | b')' | b'"' | b'{' | b'\'' | b';' | b'#'
    )
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

enum NumParse {
    Num(Value),
    /// A bigint token, carrying its canonical decimal digits (optional leading
    /// `-`, no `L`). Either an `L`-suffixed literal or a decimal literal too
    /// large for `i64` (ADR-0022).
    Bigint(String),
    No,
}

/// Turn a bigint token's decimal digits into a `Value::Bigint`, or a clear
/// error when the `bigint` feature is off (ADR-0022 revises ADR-0012).
#[cfg(feature = "bigint")]
fn classify_bigint(digits: &str, text: &str) -> Result<Value, String> {
    digits
        .parse::<num_bigint::BigInt>()
        .map(Value::Bigint)
        .map_err(|_| format!("invalid bigint literal `{}`", text))
}

#[cfg(not(feature = "bigint"))]
fn classify_bigint(_digits: &str, text: &str) -> Result<Value, String> {
    Err(format!(
        "bigint literal `{}` requires the `bigint` feature (ADR-0022)",
        text
    ))
}

/// Classify a token as an int64/float number, a reserved bigint literal, or a
/// non-number (symbol).
fn parse_number(text: &str) -> NumParse {
    let bytes = text.as_bytes();
    let first = match bytes.first() {
        Some(&b) => b,
        None => return NumParse::No,
    };
    let looks_numeric = first.is_ascii_digit()
        || ((first == b'+' || first == b'-' || first == b'.') && bytes.len() > 1);
    if !looks_numeric {
        return NumParse::No;
    }

    // Bigint literal `123L` — an `L`-suffixed decimal, any magnitude (ADR-0022).
    if let Some(prefix) = text.strip_suffix('L') {
        let digits = prefix.strip_prefix(['+', '-']).unwrap_or(prefix);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            // Normalise to sign + digits (drop a leading `+`), no `L`.
            return NumParse::Bigint(prefix.strip_prefix('+').unwrap_or(prefix).to_string());
        }
    }

    // Hex integer, with optional sign.
    let (sign, hexbody) = match text.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, text),
    };
    if let Some(hex) = hexbody
        .strip_prefix("0x")
        .or_else(|| hexbody.strip_prefix("0X"))
    {
        if let Ok(n) = i64::from_str_radix(hex, 16) {
            return NumParse::Num(Value::Int(sign.wrapping_mul(n)));
        }
    }

    if let Ok(n) = text.parse::<i64>() {
        return NumParse::Num(Value::Int(n));
    }
    // A pure decimal integer that overflowed `i64` is a bigint, not a float
    // (ADR-0022) — check before the float fallback, which would otherwise
    // accept it as an approximate `f64`.
    let intbody = text.strip_prefix(['+', '-']).unwrap_or(text);
    if !intbody.is_empty() && intbody.bytes().all(|b| b.is_ascii_digit()) {
        return NumParse::Bigint(text.strip_prefix('+').unwrap_or(text).to_string());
    }
    if let Ok(f) = text.parse::<f64>() {
        return NumParse::Num(Value::Float(f));
    }
    NumParse::No
}
