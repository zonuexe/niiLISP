//! Core value representation.
//!
//! Data containers (`List`/`Array`/`Str`) are `Rc`-wrapped and copy-on-write
//! (ADR-0024): `Clone` on store/pass shares in O(1), and a write clones only if
//! the value is still shared (`Rc::make_mut`), so the observable semantics are
//! newLISP's ORO deep-copy (CONTEXT.md: ORO, ADR-0005) with none of the eager
//! copying. Lists/arrays are `Vec`-backed (ADR-0005) and strings are byte
//! buffers (ADR-0013). Callable code (lambdas, fexprs, builtins) is shared via
//! `Rc` too; making live code independently mutable per ORO is a later
//! refinement tied to the dispatch cache (ADR-0007).

use std::collections::HashMap;
use std::rc::Rc;

use crate::eval::{Interp, Signal};

/// An interned symbol name, used as an O(1) key into a Context's value slots.
pub type SymId = usize;

/// A niiLISP value.
#[derive(Clone)]
pub enum Value {
    /// The `nil` constant (also the value of an unset symbol).
    Nil,
    /// The `true` constant.
    True,
    /// 64-bit signed integer. Arithmetic wraps on overflow (ADR-0012).
    Int(i64),
    /// IEEE-754 double.
    Float(f64),
    /// A binary-safe byte buffer (ADR-0013). Not guaranteed valid UTF-8.
    /// Copy-on-write via `Rc` (ADR-0024): shared on store/pass, cloned on write.
    Str(Rc<Vec<u8>>),
    /// An interned symbol.
    Symbol(SymId),
    /// A `Vec`-backed list (ADR-0005). Also the substrate for FOOP objects.
    /// Copy-on-write via `Rc` (ADR-0024).
    List(Rc<Vec<Value>>),
    /// A fixed-length, list-like value (CONTEXT.md: array, ADR-0023). Backed by
    /// the same `Vec`, but a distinct type: `array?`/`list?` tell them apart and
    /// it cannot be resized. Copy-on-write via `Rc` (ADR-0024).
    Array(Rc<Vec<Value>>),
    /// A context (namespace / FOOP class), named by its symbol (CONTEXT.md: Context).
    Context(SymId),
    /// A user function: evaluates its arguments.
    Lambda(Rc<Lambda>),
    /// A fexpr / `lambda-macro`: receives its arguments unevaluated (CONTEXT.md: fexpr).
    Fexpr(Rc<Lambda>),
    /// A primitive function implemented in Rust.
    Builtin(Builtin),
    /// A C function resolved through `import` (CONTEXT.md: Foreign function).
    /// The variant is always present; the libffi machinery behind it is gated on
    /// the `ffi` feature (ADR-0019), so pure builds never construct one.
    #[cfg_attr(not(all(feature = "ffi", unix)), allow(dead_code))]
    Foreign(Rc<crate::ffi::ForeignFn>),
    /// An arbitrary-precision integer (CONTEXT.md: bigint, ADR-0022). Unlike
    /// `Foreign`, the variant itself is gated on the `bigint` feature — its
    /// payload type does not exist without the `num-bigint` dependency — so a
    /// `--no-default-features` build compiles it out entirely.
    #[cfg(feature = "bigint")]
    Bigint(num_bigint::BigInt),
}

/// A structural `Debug` for embedders (`dbg!`, `.unwrap()`, assert failures).
/// It does not resolve symbol names (no interner here) — use [`crate::eval::Interp::repr`]
/// for newLISP-syntax output.
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::True => write!(f, "True"),
            Value::Int(n) => write!(f, "Int({n})"),
            Value::Float(x) => write!(f, "Float({x})"),
            Value::Str(b) => write!(f, "Str({:?})", String::from_utf8_lossy(b)),
            Value::Symbol(id) => write!(f, "Symbol(#{id})"),
            Value::Context(id) => write!(f, "Context(#{id})"),
            Value::List(l) => f.debug_tuple("List").field(&**l).finish(),
            Value::Array(a) => f.debug_tuple("Array").field(&**a).finish(),
            Value::Lambda(_) => write!(f, "Lambda(..)"),
            Value::Fexpr(_) => write!(f, "Fexpr(..)"),
            Value::Builtin(b) => write!(f, "Builtin({})", b.name),
            Value::Foreign(_) => write!(f, "Foreign(..)"),
            #[cfg(feature = "bigint")]
            Value::Bigint(n) => write!(f, "Bigint({n})"),
        }
    }
}

/// One formal parameter, with an optional default value (e.g. `(r 0)`).
pub struct Param {
    pub sym: SymId,
    pub default: Option<Value>,
}

/// The body of a user-defined lambda or fexpr.
pub struct Lambda {
    pub params: Vec<Param>,
    pub body: Vec<Value>,
}

/// Signature of a primitive function: receives already-evaluated arguments.
pub type BuiltinFn = fn(&Interp, &[Value]) -> Result<Value, Signal>;

/// A primitive function value.
#[derive(Clone)]
pub struct Builtin {
    pub name: &'static str,
    pub func: BuiltinFn,
}

impl Value {
    /// Construct a list value, wrapping the elements for copy-on-write (ADR-0024).
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(items))
    }
    /// Construct an array value (copy-on-write, ADR-0024).
    pub fn array(items: Vec<Value>) -> Value {
        Value::Array(Rc::new(items))
    }
    /// Construct a string value (copy-on-write, ADR-0024).
    pub fn str(bytes: Vec<u8>) -> Value {
        Value::Str(Rc::new(bytes))
    }

    /// newLISP truthiness: only `nil` and an empty list/array are false.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil)
            && !matches!(self, Value::List(l) if l.is_empty())
            && !matches!(self, Value::Array(a) if a.is_empty())
    }
}

/// Symbol interner: maps names to stable `SymId`s and back.
#[derive(Default)]
pub struct Interner {
    names: Vec<String>,
    ids: HashMap<String, SymId>,
}

impl Interner {
    pub fn intern(&mut self, name: &str) -> SymId {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len();
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: SymId) -> &str {
        &self.names[id]
    }

    /// The interned symbols of context `ctx` (names `ctx:…`), in name order —
    /// for `dotree` (ADR-0026).
    pub fn context_symbols(&self, ctx: &str) -> Vec<SymId> {
        let prefix = format!("{}:", ctx);
        let mut out: Vec<SymId> = self
            .names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with(&prefix))
            .map(|(i, _)| i as SymId)
            .collect();
        out.sort_by(|&a, &b| self.names[a].cmp(&self.names[b]));
        out
    }

    /// Every interned name, in interning order — used by the REPL to offer
    /// Tab-completion candidates (primitives plus whatever the session has
    /// interned so far). Only the `readline` REPL consumes it.
    #[cfg_attr(not(feature = "readline"), allow(dead_code))]
    pub fn all_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// The MAIN-level symbols (names without a context prefix), name-sorted —
    /// for `(symbols)` / `(symbols MAIN)`.
    pub fn main_symbols(&self) -> Vec<SymId> {
        let mut out: Vec<SymId> = self
            .names
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.contains(':'))
            .map(|(i, _)| i as SymId)
            .collect();
        out.sort_by(|&a, &b| self.names[a].cmp(&self.names[b]));
        out
    }
}
