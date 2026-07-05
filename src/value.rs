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
}
