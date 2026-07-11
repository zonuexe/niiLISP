//! XML parsing (ADR-0038): `xml-parse` / `xml-type-tags` / `xml-error`. A
//! hand-written recursive-descent parser for the well-formed XML 1.0 subset
//! newLISP supports — elements, attributes, text, `CDATA`, comments — skipping
//! DTDs and processing instructions. The result is newLISP's tagged-list
//! structure: an element is `(ELEMENT name attributes children)`, and text /
//! CDATA / comment nodes carry their type tag and string. The four type tags
//! are customizable via `xml-type-tags` (a `nil` slot drops the tag), and the
//! `int-options` bit flags tune the translation. Malformed input yields `nil`
//! and an `("message" position)` pair for `xml-error`.

use crate::eval::{Interp, Signal};
use crate::value::Value;

pub fn install(interp: &Interp) {
    interp.register_builtin("xml-parse", b_xml_parse);
    interp.register_builtin("xml-type-tags", b_xml_type_tags);
    interp.register_builtin("xml-error", b_xml_error);
}

// Option bit flags (ADR-0038).
const OPT_NO_WHITESPACE: i64 = 1;
const OPT_NO_EMPTY_ATTR: i64 = 2;
const OPT_NO_COMMENTS: i64 = 4;
const OPT_TAGS_AS_SYMS: i64 = 8;
const OPT_SXML_ATTR: i64 = 16;

fn b_xml_parse(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let bytes = match args.first() {
        Some(Value::Str(b)) => b.clone(),
        _ => return Err(Signal::error("xml-parse: expected a string")),
    };
    let options = match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    };
    let tags = i.xml_type_tags();
    let mut p = Parser {
        s: &bytes,
        pos: 0,
        i,
        options,
        tags,
    };
    match p.parse_nodes(None) {
        Ok(nodes) => {
            i.set_xml_error(Value::Nil);
            Ok(Value::list(nodes))
        }
        Err((msg, pos)) => {
            i.set_xml_error(Value::list(vec![
                Value::str(msg.as_bytes().to_vec()),
                Value::Int(pos as i64),
            ]));
            Ok(Value::Nil)
        }
    }
}

fn b_xml_type_tags(i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    if args.is_empty() {
        return Ok(Value::list(i.xml_type_tags().to_vec()));
    }
    // (xml-type-tags text cdata comment element) — a nil slot suppresses that tag.
    let cur = i.xml_type_tags();
    let get = |n: usize| args.get(n).cloned().unwrap_or_else(|| cur[n].clone());
    i.set_xml_type_tags([get(0), get(1), get(2), get(3)]);
    Ok(Value::list(i.xml_type_tags().to_vec()))
}

fn b_xml_error(i: &Interp, _args: &[Value]) -> Result<Value, Signal> {
    Ok(i.get_xml_error())
}

type XErr = (&'static str, usize);

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    i: &'a Interp,
    options: i64,
    tags: [Value; 4],
}

impl Parser<'_> {
    fn opt(&self, bit: i64) -> bool {
        self.options & bit != 0
    }

    fn rest(&self) -> &[u8] {
        &self.s[self.pos..]
    }

    fn starts(&self, lit: &[u8]) -> bool {
        self.rest().starts_with(lit)
    }

    /// A type tag for `slot` (0=text 1=cdata 2=comment 3=element), interning it
    /// as a symbol under option 8; `Nil` means suppressed.
    fn tag(&self, slot: usize) -> Value {
        let t = &self.tags[slot];
        if self.opt(OPT_TAGS_AS_SYMS) {
            if let Value::Str(b) = t {
                return Value::Symbol(self.i.intern(&String::from_utf8_lossy(b)));
            }
        }
        t.clone()
    }

    /// Build a node: `(tag content…)`, or bare content when the tag is
    /// suppressed (a single content item is unwrapped, giving SXML shape).
    fn node(&self, tag: Value, content: Vec<Value>) -> Value {
        if matches!(tag, Value::Nil) {
            if content.len() == 1 {
                content.into_iter().next().unwrap()
            } else {
                Value::list(content)
            }
        } else {
            let mut v = Vec::with_capacity(content.len() + 1);
            v.push(tag);
            v.extend(content);
            Value::list(v)
        }
    }

    /// Parse a run of nodes until EOF (`close` = None) or the matching close tag
    /// `</name>` (`close` = Some(name)).
    fn parse_nodes(&mut self, close: Option<&[u8]>) -> Result<Vec<Value>, XErr> {
        let mut nodes = Vec::new();
        loop {
            if self.pos >= self.s.len() {
                if close.is_some() {
                    return Err(("expected closing tag", self.pos));
                }
                return Ok(nodes);
            }
            if self.starts(b"</") {
                // Closing tag: verify it matches and consume it (the caller that
                // opened the element handles the actual close via return).
                if close.is_some() {
                    return Ok(nodes);
                }
                return Err(("unexpected closing tag", self.pos));
            }
            if self.starts(b"<!--") {
                let text = self.take_comment()?;
                if !self.opt(OPT_NO_COMMENTS) {
                    let tag = self.tag(2);
                    nodes.push(self.node(tag, vec![Value::str(text)]));
                }
            } else if self.starts(b"<![CDATA[") {
                let text = self.take_cdata()?;
                let tag = self.tag(1);
                nodes.push(self.node(tag, vec![Value::str(text)]));
            } else if self.starts(b"<!") || self.starts(b"<?") {
                self.skip_decl()?; // DTD / processing instruction
            } else if self.starts(b"<") {
                nodes.push(self.parse_element()?);
            } else {
                let text = self.take_text();
                let is_ws = text.iter().all(|b| b.is_ascii_whitespace());
                let drop = text.is_empty() || (is_ws && self.opt(OPT_NO_WHITESPACE));
                if !drop {
                    let tag = self.tag(0);
                    nodes.push(self.node(tag, vec![Value::str(text)]));
                }
            }
        }
    }

    fn parse_element(&mut self) -> Result<Value, XErr> {
        self.pos += 1; // '<'
        let name = self.take_name();
        if name.is_empty() {
            return Err(("expected element name", self.pos));
        }
        // Attributes.
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.rest().first() {
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(b'/') if self.starts(b"/>") => {
                    self.pos += 2;
                    return Ok(self.element_node(name, attrs, Vec::new()));
                }
                None => return Err(("unterminated start tag", self.pos)),
                _ => {
                    let (k, v) = self.take_attribute()?;
                    attrs.push(Value::list(vec![Value::str(k), Value::str(v)]));
                }
            }
        }
        // Children until the matching close tag.
        let children = self.parse_nodes(Some(&name))?;
        // Consume `</name>`.
        if !self.starts(b"</") {
            return Err(("expected closing tag", self.pos));
        }
        self.pos += 2;
        let close = self.take_name();
        if close != name {
            return Err(("mismatched closing tag", self.pos));
        }
        self.skip_ws();
        if self.rest().first() != Some(&b'>') {
            return Err(("expected closing tag: >", self.pos));
        }
        self.pos += 1;
        Ok(self.element_node(name, attrs, children))
    }

    fn element_node(&self, name: Vec<u8>, attrs: Vec<Value>, children: Vec<Value>) -> Value {
        let name_val = if self.opt(OPT_TAGS_AS_SYMS) {
            Value::Symbol(self.i.intern(&String::from_utf8_lossy(&name)))
        } else {
            Value::str(name)
        };
        let attr_val = if self.opt(OPT_SXML_ATTR) {
            // SXML: (@ (k v) …). An empty attribute group is omitted.
            if attrs.is_empty() {
                None
            } else {
                let mut g = vec![Value::Symbol(self.i.intern("@"))];
                g.extend(attrs);
                Some(Value::list(g))
            }
        } else if attrs.is_empty() && self.opt(OPT_NO_EMPTY_ATTR) {
            None
        } else {
            Some(Value::list(attrs))
        };
        let mut content = vec![name_val];
        if let Some(a) = attr_val {
            content.push(a);
        }
        content.push(Value::list(children));
        self.node(self.tag(3), content)
    }

    // ---- lexer helpers ----

    fn skip_ws(&mut self) {
        while matches!(self.rest().first(), Some(b) if b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn take_name(&mut self) -> Vec<u8> {
        let start = self.pos;
        while let Some(&b) = self.rest().first() {
            if b.is_ascii_whitespace() || b == b'>' || b == b'/' || b == b'=' {
                break;
            }
            self.pos += 1;
        }
        self.s[start..self.pos].to_vec()
    }

    fn take_attribute(&mut self) -> Result<(Vec<u8>, Vec<u8>), XErr> {
        let key = self.take_name();
        self.skip_ws();
        if self.rest().first() != Some(&b'=') {
            return Err(("expected = in attribute", self.pos));
        }
        self.pos += 1;
        self.skip_ws();
        let quote = match self.rest().first() {
            Some(&q @ (b'"' | b'\'')) => q,
            _ => return Err(("expected quoted attribute value", self.pos)),
        };
        self.pos += 1;
        let start = self.pos;
        while let Some(&b) = self.rest().first() {
            if b == quote {
                break;
            }
            self.pos += 1;
        }
        if self.rest().first() != Some(&quote) {
            return Err(("unterminated attribute value", self.pos));
        }
        let raw = self.s[start..self.pos].to_vec();
        self.pos += 1;
        Ok((key, decode_entities(&raw)))
    }

    fn take_text(&mut self) -> Vec<u8> {
        let start = self.pos;
        while let Some(&b) = self.rest().first() {
            if b == b'<' {
                break;
            }
            self.pos += 1;
        }
        decode_entities(&self.s[start..self.pos])
    }

    fn take_comment(&mut self) -> Result<Vec<u8>, XErr> {
        self.pos += 4; // <!--
        let start = self.pos;
        while self.pos < self.s.len() && !self.starts(b"-->") {
            self.pos += 1;
        }
        if !self.starts(b"-->") {
            return Err(("unterminated comment", self.pos));
        }
        let text = self.s[start..self.pos].to_vec();
        self.pos += 3;
        Ok(text)
    }

    fn take_cdata(&mut self) -> Result<Vec<u8>, XErr> {
        self.pos += 9; // <![CDATA[
        let start = self.pos;
        while self.pos < self.s.len() && !self.starts(b"]]>") {
            self.pos += 1;
        }
        if !self.starts(b"]]>") {
            return Err(("unterminated CDATA", self.pos));
        }
        let text = self.s[start..self.pos].to_vec();
        self.pos += 3;
        Ok(text)
    }

    /// Skip a DTD (`<!DOCTYPE …>`, including an internal `[…]` subset) or a
    /// processing instruction (`<? … ?>`).
    fn skip_decl(&mut self) -> Result<(), XErr> {
        if self.starts(b"<?") {
            self.pos += 2;
            while self.pos < self.s.len() && !self.starts(b"?>") {
                self.pos += 1;
            }
            if !self.starts(b"?>") {
                return Err(("unterminated processing instruction", self.pos));
            }
            self.pos += 2;
            return Ok(());
        }
        // <! … > with a possible [ … ] internal subset.
        self.pos += 2;
        let mut depth = 0i32;
        while let Some(&b) = self.rest().first() {
            self.pos += 1;
            match b {
                b'[' => depth += 1,
                b']' => depth -= 1,
                b'>' if depth <= 0 => return Ok(()),
                _ => {}
            }
        }
        Err(("unterminated declaration", self.pos))
    }
}

/// Decode the five predefined entities and numeric character references.
fn decode_entities(s: &[u8]) -> Vec<u8> {
    if !s.contains(&b'&') {
        return s.to_vec();
    }
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] != b'&' {
            out.push(s[i]);
            i += 1;
            continue;
        }
        // Find the terminating ';' within a short window.
        let semi = s[i + 1..]
            .iter()
            .position(|&b| b == b';')
            .map(|p| i + 1 + p);
        let Some(semi) = semi else {
            out.push(b'&');
            i += 1;
            continue;
        };
        let ent = &s[i + 1..semi];
        let replaced = match ent {
            b"lt" => Some(vec![b'<']),
            b"gt" => Some(vec![b'>']),
            b"amp" => Some(vec![b'&']),
            b"quot" => Some(vec![b'"']),
            b"apos" => Some(vec![b'\'']),
            _ if ent.first() == Some(&b'#') => {
                let (radix, digits) = if ent.get(1) == Some(&b'x') || ent.get(1) == Some(&b'X') {
                    (16, &ent[2..])
                } else {
                    (10, &ent[1..])
                };
                std::str::from_utf8(digits)
                    .ok()
                    .and_then(|d| u32::from_str_radix(d, radix).ok())
                    .and_then(char::from_u32)
                    .map(|c| {
                        let mut buf = [0u8; 4];
                        c.encode_utf8(&mut buf).as_bytes().to_vec()
                    })
            }
            _ => None,
        };
        match replaced {
            Some(bytes) => {
                out.extend_from_slice(&bytes);
                i = semi + 1;
            }
            None => {
                out.push(b'&');
                i += 1;
            }
        }
    }
    out
}
