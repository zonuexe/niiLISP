//! The evaluator: tree-walks the live `Vec`-backed list structure (ADR-0007).
//!
//! Dynamic scoping (ADR-0006) is implemented with per-symbol value slots plus a
//! save/restore rebinding stack; the restore rides a `Drop` guard so bindings
//! are reinstated even when an error unwinds. Non-local exit is `Result<Value,
//! Signal>` (ADR-0011), so error unwinding and scope restoration share one path.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::builtins;
use crate::printer::to_repr;
use crate::value::{Builtin, BuiltinFn, Interner, Lambda, Param, SymId, Value};

#[cfg(feature = "bigint")]
use num_bigint::BigInt;

/// Non-local control flow: `throw` and errors both unwind to the nearest
/// `catch` (ADR-0011).
pub enum Signal {
    Throw(Value),
    Error(String),
}

impl Signal {
    pub fn error(msg: impl Into<String>) -> Signal {
        Signal::Error(msg.into())
    }
}

/// Names dispatched as special forms (evaluated by `try_special_form`). Together
/// with the registered builtins they form the MAIN primitives that the reader
/// keeps unqualified inside a context (ADR-0026). Keep in sync with the match in
/// `try_special_form`.
pub const SPECIAL_FORMS: &[&str] = &[
    "quote",
    "if",
    "if-not",
    "cond",
    "case",
    "and",
    "or",
    "while",
    "when",
    "unless",
    "dotimes",
    "for",
    "dolist",
    "dostring",
    "dotree",
    "context",
    "until",
    "do-until",
    "do-while",
    "amb",
    "extend",
    "local",
    "inc",
    "dec",
    "++",
    "--",
    "push",
    "pop",
    "swap",
    "reverse",
    "sort",
    "rotate",
    "replace",
    "set-ref",
    "set-ref-all",
    "find-all",
    "pop-assoc",
    "begin",
    "define",
    "set",
    "setq",
    "setf",
    "constant",
    "self",
    "time",
    "let",
    "letn",
    "letex",
    "curry",
    "lambda",
    "fn",
    "lambda-macro",
    "define-macro",
    "catch",
    "throw",
    "read-buffer",
    // Cilk special forms (ADR-0032). Real only in the Unix `mt` build; listing
    // them here makes a bare `spawn`/`fork` reference evaluate to a truthy symbol
    // (as `(if (not fork) …)` in `qa-cilk` probes). Their `try_special_form`
    // arms are `mt`-gated, so a call is a plain error in a non-`mt` build.
    "spawn",
    "fork",
    "receive",
    "net-receive",
];

/// The interpreter state. Methods take `&self`; mutable state lives behind
/// `RefCell` (ADR-0006's interior mutability) so scope guards can borrow it.
pub struct Interp {
    pub interner: RefCell<Interner>,
    /// The MAIN context's symbol value slots.
    globals: RefCell<HashMap<SymId, Value>>,
    /// Stack of `self` places for FOOP colon dispatch (ADR-0010).
    self_stack: RefCell<Vec<Place>>,
    /// Symbols made read-only by `constant`.
    protected: RefCell<HashSet<SymId>>,
    /// Loaded shared libraries, kept open for the process lifetime (ADR-0019).
    #[cfg(all(feature = "ffi", unix))]
    libs: RefCell<HashMap<String, libloading::Library>>,
    /// libffi closures for `callback`, kept alive for the process lifetime so C
    /// never holds a dangling function pointer (ADR-0020).
    #[cfg(all(feature = "ffi", unix))]
    callbacks: RefCell<Vec<libffi::middle::Closure<'static>>>,
    /// xorshift64* state for `rand`/`random`/`amb`, reseedable via `seed`.
    rng: RefCell<u64>,
    /// The process command line, for `(main-args)`.
    main_args: RefCell<Vec<String>>,
    /// Per-call stack of the arguments not bound to a declared parameter, for
    /// `(args)` (ADR-0027).
    args_stack: RefCell<Vec<Vec<Value>>>,
    /// Compiled regexes keyed by `(pattern, option)`, so `regex`/`regex-comp`
    /// compile a pattern once (ADR-0028).
    #[cfg(feature = "regex")]
    regex_cache: RefCell<HashMap<(String, i64), regex::bytes::Regex>>,
    /// Open-file registry for the file-I/O handles (ADR-0029).
    files: RefCell<crate::fileio::FileTable>,
    /// The most recent `read-line` result, for `(current-line)` — a single
    /// interpreter-global buffer, as in newLISP (ADR-0029).
    current_line: RefCell<Vec<u8>>,
    /// The current context at eval time, switched by `(context X)` and returned
    /// by the no-arg `(context)` (ADR-0026). Symbol qualification itself is a
    /// read-time concern; this tracks the runtime view for reflection.
    current_ctx: RefCell<SymId>,
    /// MAIN symbols explicitly declared global by `(global …)`; consulted by
    /// `global?` and unioned into the reader's unqualified-name set so later
    /// reads treat them as MAIN-level (ADR-0026).
    global_syms: RefCell<HashSet<SymId>>,
    /// The four XML type tags emitted by `xml-parse` (TEXT/CDATA/COMMENT/ELEMENT),
    /// customizable via `xml-type-tags`; a `nil` slot suppresses that tag (ADR-0038).
    xml_type_tags: RefCell<[Value; 4]>,
    /// `("message" position)` from the last `xml-parse` / `json-parse`, read by
    /// `xml-error` / `json-error`; `Nil` when the last parse succeeded (ADR-0038).
    xml_error: RefCell<Value>,
    json_error: RefCell<Value>,
    /// Cilk API state: the pending `spawn`ed children and `share`d pages
    /// (ADR-0032). Present only in the Unix `mt` build.
    #[cfg(all(feature = "mt", unix))]
    cilk: RefCell<crate::process::CilkState>,
    /// The Lisp handler installed for each OS signal number by `signal`
    /// (ADR-0032). Fired at safe points by `dispatch_signals`.
    #[cfg(all(feature = "mt", unix))]
    signal_handlers: RefCell<HashMap<i32, Value>>,
}

/// Pops the `args` stack when a call returns (including on error unwind),
/// mirroring `Scope`'s binding restore (ADR-0027).
struct ArgsGuard<'a>(&'a Interp);

impl Drop for ArgsGuard<'_> {
    fn drop(&mut self) {
        self.0.args_stack.borrow_mut().pop();
    }
}

/// A dynamic-binding scope. On drop it restores every slot it changed, in
/// reverse order — including on error unwind (ADR-0006).
pub(crate) struct Scope<'a> {
    interp: &'a Interp,
    saved: Vec<(SymId, Option<Value>)>,
}

impl<'a> Scope<'a> {
    pub(crate) fn new(interp: &'a Interp) -> Self {
        Scope {
            interp,
            saved: Vec::new(),
        }
    }

    pub(crate) fn bind(&mut self, sym: SymId, val: Value) {
        let old = self.interp.globals.borrow_mut().insert(sym, val);
        self.saved.push((sym, old));
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        let mut g = self.interp.globals.borrow_mut();
        while let Some((sym, old)) = self.saved.pop() {
            match old {
                Some(v) => {
                    g.insert(sym, v);
                }
                None => {
                    g.remove(&sym);
                }
            }
        }
    }
}

/// A nonzero starting seed for the RNG, taken from the wall clock so unseeded
/// runs differ (newLISP seeds from time too); `seed` overrides it.
fn default_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos | 1
}

/// Increment a numeric place by `sign * delta` for `++`/`--`. Stays in `i64`
/// (wrapping) unless the place or amount is a bigint, in which case it computes
/// in `BigInt` (ADR-0022). `nil` reads as zero.
fn incr_value(base: &Value, sign: i64, delta: &Value) -> Result<Value, Signal> {
    #[cfg(feature = "bigint")]
    if matches!(base, Value::Bigint(_)) || matches!(delta, Value::Bigint(_)) {
        let b = incr_to_bigint(base)?;
        let d = incr_to_bigint(delta)?;
        let next = if sign < 0 { b - d } else { b + d };
        return Ok(Value::Bigint(next));
    }
    let base_i = match base {
        Value::Int(n) => *n,
        Value::Nil => 0,
        Value::Float(f) => *f as i64,
        _ => return Err(Signal::error("++/--: place is not a number")),
    };
    let delta_i = match delta {
        Value::Int(n) => *n,
        Value::Float(f) => *f as i64,
        _ => return Err(Signal::error("++/--: amount must be a number")),
    };
    Ok(Value::Int(base_i.wrapping_add(sign.wrapping_mul(delta_i))))
}

/// Coerce a value to `BigInt` for `++`/`--`; `nil` is zero, a float truncates.
#[cfg(feature = "bigint")]
fn incr_to_bigint(v: &Value) -> Result<BigInt, Signal> {
    match v {
        Value::Int(n) => Ok(BigInt::from(*n)),
        Value::Nil => Ok(BigInt::from(0)),
        Value::Bigint(b) => Ok(b.clone()),
        Value::Float(f) if f.is_finite() => {
            Ok(num_traits::FromPrimitive::from_f64(f.trunc()).unwrap_or_default())
        }
        _ => Err(Signal::error("++/--: not a number")),
    }
}

/// Mangle a Dictionary key (a string or number) to a `_`-prefixed context
/// symbol term (ADR-0030), mirroring newLISP's `makeSafeSymbol`. A number and
/// its string form collapse to the same key.
fn dict_key(v: &Value) -> String {
    match v {
        Value::Str(b) => format!("_{}", String::from_utf8_lossy(b)),
        Value::Int(n) => format!("_{}", n),
        Value::Float(f) => format!("_{}", *f as i64),
        _ => "_".to_string(),
    }
}

impl Interp {
    pub fn new() -> Self {
        let interp = Interp {
            interner: RefCell::new(Interner::default()),
            globals: RefCell::new(HashMap::new()),
            self_stack: RefCell::new(Vec::new()),
            protected: RefCell::new(HashSet::new()),
            #[cfg(all(feature = "ffi", unix))]
            libs: RefCell::new(HashMap::new()),
            #[cfg(all(feature = "ffi", unix))]
            callbacks: RefCell::new(Vec::new()),
            rng: RefCell::new(default_seed()),
            main_args: RefCell::new(Vec::new()),
            args_stack: RefCell::new(Vec::new()),
            #[cfg(feature = "regex")]
            regex_cache: RefCell::new(HashMap::new()),
            files: RefCell::new(crate::fileio::FileTable::new()),
            current_line: RefCell::new(Vec::new()),
            current_ctx: RefCell::new(0),
            global_syms: RefCell::new(HashSet::new()),
            xml_type_tags: RefCell::new([
                Value::str(b"TEXT".to_vec()),
                Value::str(b"CDATA".to_vec()),
                Value::str(b"COMMENT".to_vec()),
                Value::str(b"ELEMENT".to_vec()),
            ]),
            xml_error: RefCell::new(Value::Nil),
            json_error: RefCell::new(Value::Nil),
            #[cfg(all(feature = "mt", unix))]
            cilk: RefCell::new(crate::process::CilkState::default()),
            #[cfg(all(feature = "mt", unix))]
            signal_handlers: RefCell::new(HashMap::new()),
        };
        *interp.current_ctx.borrow_mut() = interp.intern("MAIN");
        builtins::install(&interp);
        crate::ffi::install(&interp);
        crate::fileio::install(&interp);
        crate::process::install(&interp);
        crate::net::install(&interp);
        crate::date::install(&interp);
        crate::json::install(&interp);
        crate::xml::install(&interp);
        interp
    }

    /// The open-file registry, for the file-I/O builtins (ADR-0029).
    pub fn files(&self) -> &RefCell<crate::fileio::FileTable> {
        &self.files
    }

    /// Record the line just read by `read-line`, for `(current-line)`.
    pub fn set_current_line(&self, line: Vec<u8>) {
        *self.current_line.borrow_mut() = line;
    }

    /// The most recent `read-line` result (`(current-line)`).
    pub fn current_line(&self) -> Vec<u8> {
        self.current_line.borrow().clone()
    }

    /// The Cilk API state (ADR-0032), for the `spawn`/`sync`/`abort`/`share`
    /// builtins in `process.rs`.
    #[cfg(all(feature = "mt", unix))]
    pub fn cilk(&self) -> &RefCell<crate::process::CilkState> {
        &self.cilk
    }

    /// The registered `signal` handlers (ADR-0032).
    #[cfg(all(feature = "mt", unix))]
    pub fn signal_handlers(&self) -> &RefCell<HashMap<i32, Value>> {
        &self.signal_handlers
    }

    /// Read `src` as code and evaluate all its forms, returning the last result
    /// (`eval-string`; also the guiserver's inbound-event dispatch path).
    pub fn eval_string(&self, src: &[u8]) -> Result<Value, Signal> {
        let forms = {
            let primitives = self.primitive_names();
            let mut interner = self.interner.borrow_mut();
            let mut reader = crate::reader::Reader::new(src, &mut interner, &primitives);
            reader
                .read_all()
                .map_err(|e| Signal::error(format!("eval-string: {}", e)))?
        };
        let mut result = Value::Nil;
        for f in &forms {
            result = self.eval(f)?;
        }
        Ok(result)
    }

    /// Read the first form from `src` as **data** (no evaluation) — used to
    /// deserialise a value transferred across a process boundary (ADR-0032).
    /// Returns `nil` if it does not parse.
    #[cfg(all(feature = "mt", unix))]
    pub fn read_one(&self, src: &[u8]) -> Value {
        let primitives = self.primitive_names();
        let mut interner = self.interner.borrow_mut();
        let mut reader = crate::reader::Reader::new(src, &mut interner, &primitives);
        reader
            .read_all()
            .ok()
            .and_then(|forms| forms.into_iter().next())
            .unwrap_or(Value::Nil)
    }

    /// Record the process command line for `(main-args)`.
    pub fn set_main_args(&self, args: Vec<String>) {
        *self.main_args.borrow_mut() = args;
    }

    /// The recorded command line (`(main-args)`).
    pub fn main_args(&self) -> Vec<String> {
        self.main_args.borrow().clone()
    }

    /// Next 64-bit value from the xorshift64* generator (`rand`/`random`/`amb`).
    pub fn rng_next_u64(&self) -> u64 {
        let mut state = self.rng.borrow_mut();
        let mut x = *state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Reseed the generator (`seed`), returning the previous seed. A zero seed
    /// would freeze xorshift, so it is bumped to 1.
    pub fn rng_seed(&self, seed: u64) -> u64 {
        let mut state = self.rng.borrow_mut();
        let prev = *state;
        *state = if seed == 0 { 1 } else { seed };
        prev
    }

    /// The MAIN primitive names — registered builtins plus special forms — that
    /// the reader keeps unqualified inside a context (ADR-0026). Computed from
    /// the current globals, so call it after `Interp::new` has installed them.
    pub fn primitive_names(&self) -> HashSet<String> {
        let mut set: HashSet<String> = self
            .globals
            .borrow()
            .keys()
            .map(|&id| self.sym_name(id))
            .collect();
        for &sf in SPECIAL_FORMS {
            set.insert(sf.to_string());
        }
        for &id in self.global_syms.borrow().iter() {
            set.insert(self.sym_name(id));
        }
        set
    }

    /// Declare a MAIN symbol global (`(global …)`), so `global?` reports it and
    /// later reads keep it unqualified inside contexts (ADR-0026).
    pub fn make_global(&self, sym: SymId) {
        self.global_syms.borrow_mut().insert(sym);
    }

    /// The four XML type tags (`xml-type-tags`, ADR-0038).
    pub fn xml_type_tags(&self) -> [Value; 4] {
        self.xml_type_tags.borrow().clone()
    }

    pub fn set_xml_type_tags(&self, tags: [Value; 4]) {
        *self.xml_type_tags.borrow_mut() = tags;
    }

    /// The `xml-parse`/`json-parse` last-error slots (ADR-0038).
    pub fn set_xml_error(&self, v: Value) {
        *self.xml_error.borrow_mut() = v;
    }

    pub fn get_xml_error(&self) -> Value {
        self.xml_error.borrow().clone()
    }

    pub fn set_json_error(&self, v: Value) {
        *self.json_error.borrow_mut() = v;
    }

    pub fn get_json_error(&self) -> Value {
        self.json_error.borrow().clone()
    }

    /// Whether a symbol is global: a special form, a registered builtin, a
    /// context, or explicitly declared with `(global …)`.
    pub fn is_global_sym(&self, sym: SymId) -> bool {
        if self.global_syms.borrow().contains(&sym) {
            return true;
        }
        if SPECIAL_FORMS.contains(&self.sym_name(sym).as_str()) {
            return true;
        }
        matches!(
            self.lookup(sym),
            Value::Builtin(_) | Value::Context(_) | Value::Foreign(_)
        )
    }

    /// Register a primitive under `name`. Used by the FFI module (ADR-0019).
    #[cfg_attr(not(all(feature = "ffi", unix)), allow(dead_code))]
    pub fn register_builtin(&self, name: &'static str, func: BuiltinFn) {
        let id = self.intern(name);
        self.set_global(id, Value::Builtin(Builtin { name, func }));
    }

    /// Resolve `name` in the shared library at `path`, loading and caching the
    /// library (kept open for the process lifetime). `None` if either is missing.
    #[cfg(all(feature = "ffi", unix))]
    pub fn ffi_resolve(&self, path: &str, name: &str) -> Option<libffi::middle::CodePtr> {
        let mut libs = self.libs.borrow_mut();
        if !libs.contains_key(path) {
            // SAFETY: loading arbitrary shared libraries is inherently unsafe
            // (their init code runs); accepted as the nature of FFI (ADR-0015).
            let lib = unsafe { libloading::Library::new(path).ok()? };
            libs.insert(path.to_string(), lib);
        }
        let lib = libs.get(path)?;
        let mut symbol = name.as_bytes().to_vec();
        symbol.push(0);
        // SAFETY: the library stays loaded for the process lifetime, so the
        // returned code address remains valid.
        let sym: libloading::Symbol<unsafe extern "C" fn()> = unsafe { lib.get(&symbol).ok()? };
        let addr = *sym as usize;
        Some(libffi::middle::CodePtr(addr as *mut std::ffi::c_void))
    }

    /// Keep a `callback` closure alive for the process lifetime (ADR-0020).
    #[cfg(all(feature = "ffi", unix))]
    pub fn keep_callback(&self, closure: libffi::middle::Closure<'static>) {
        self.callbacks.borrow_mut().push(closure);
    }

    pub fn intern(&self, name: &str) -> SymId {
        self.interner.borrow_mut().intern(name)
    }

    pub fn sym_name(&self, id: SymId) -> String {
        self.interner.borrow().name(id).to_string()
    }

    pub fn set_global(&self, sym: SymId, val: Value) {
        self.globals.borrow_mut().insert(sym, val);
    }

    pub fn lookup(&self, sym: SymId) -> Value {
        self.globals
            .borrow()
            .get(&sym)
            .cloned()
            .unwrap_or(Value::Nil)
    }

    /// Run `f` with a borrow of the symbol's stored value (no clone), so the FFI
    /// `address` builtin can read a symbol-held buffer's stable pointer without
    /// the copy that `lookup` would make (ADR-0021).
    #[cfg(all(feature = "ffi", unix))]
    pub fn with_global<R>(&self, sym: SymId, f: impl FnOnce(Option<&Value>) -> R) -> R {
        f(self.globals.borrow().get(&sym))
    }

    pub fn repr(&self, v: &Value) -> String {
        to_repr(v, &self.interner.borrow())
    }

    /// Evaluate one expression.
    pub fn eval(&self, v: &Value) -> Result<Value, Signal> {
        match v {
            Value::Symbol(id) => {
                let val = self.lookup(*id);
                // A special-form name referenced as a value evaluates to itself,
                // so it can be aliased — `(define DEFINE define)` (ADR-0027).
                if matches!(val, Value::Nil) && SPECIAL_FORMS.contains(&self.sym_name(*id).as_str())
                {
                    return Ok(Value::Symbol(*id));
                }
                Ok(val)
            }
            Value::List(items) => self.eval_list(items),
            other => Ok(other.clone()),
        }
    }

    /// Evaluate a body, returning the last value (empty body -> nil).
    fn eval_body(&self, body: &[Value]) -> Result<Value, Signal> {
        let mut result = Value::Nil;
        for form in body {
            // A safe point to run any pending OS signal handlers (ADR-0032); the
            // fast path is a single relaxed atomic load.
            #[cfg(all(feature = "mt", unix))]
            crate::process::dispatch_signals(self);
            result = self.eval(form)?;
        }
        Ok(result)
    }

    fn eval_list(&self, items: &[Value]) -> Result<Value, Signal> {
        let head = match items.first() {
            None => return Ok(Value::Nil),
            Some(h) => h,
        };

        if let Value::Symbol(id) = head {
            let name = self.sym_name(*id);
            // Colon dispatch: (:method obj args...) (ADR-0010).
            if let Some(method) = name.strip_prefix(':') {
                return self.colon_dispatch(method, &items[1..]);
            }
            if let Some(result) = self.try_special_form(&name, &items[1..]) {
                return result;
            }
        }

        let callable = self.eval(head)?;
        match callable {
            // Implicit indexing: (i list) / (i count list).
            Value::Int(i) => self.implicit_index(i, &items[1..]),
            // A lambda-headed list `(lambda …)` in function position is called as
            // a function (ADR-0027); any other list/array/string indexes itself:
            // (seq i) / (seq i j) — a string yields its i-th character
            // (ADR-0023/0025).
            Value::List(l) if self.lambda_head(&l).is_some() => {
                let is_fexpr = self.lambda_head(&l).unwrap();
                self.call_lambda_list(&l, is_fexpr, &items[1..])
            }
            seq @ (Value::List(_) | Value::Array(_) | Value::Str(_)) => {
                let mut v = seq;
                for idx in &items[1..] {
                    let i = self.eval_index(idx)?;
                    v = index_one(i, &v);
                }
                Ok(v)
            }
            // Default functor: applying a context constructs an object (ADR-0010).
            Value::Context(ctx) => self.construct(ctx, &items[1..]),
            // An operator that evaluated to a special-form name (an alias, e.g.
            // `DEFINE` bound to `define`) dispatches that special form with the
            // raw arguments (ADR-0027).
            Value::Symbol(id) => {
                let name = self.sym_name(id);
                if let Some(r) = self.try_special_form(&name, &items[1..]) {
                    r
                } else {
                    self.apply(Value::Symbol(id), &items[1..])
                }
            }
            other => self.apply(other, &items[1..]),
        }
    }

    // ---- FOOP dispatch (ADR-0010) ----------------------------------------

    /// `(:method obj args...)`: resolve `obj` to a place, dispatch on its class
    /// tag, and run the method with `self` bound to that place (write-back).
    fn colon_dispatch(&self, method: &str, args: &[Value]) -> Result<Value, Signal> {
        let obj_expr = args
            .first()
            .ok_or_else(|| Signal::error("colon dispatch: missing object"))?;
        // Prefer a place (so mutations write back); else stash in a temp slot.
        let place = match self.resolve_place(obj_expr) {
            Ok(p) => p,
            Err(_) => {
                let v = self.eval(obj_expr)?;
                let tmp = self.intern("$self-tmp");
                self.set_global(tmp, v);
                Place {
                    root: tmp,
                    path: Vec::new(),
                }
            }
        };
        let obj = self.read_place(&place)?;
        let class = class_of(&obj)
            .ok_or_else(|| Signal::error("colon dispatch: argument is not a FOOP object"))?;
        let msym = self.intern(&format!("{}:{}", self.sym_name(class), method));
        let func = self.lookup(msym);

        // Evaluate method args in the *caller's* self, then bind the new self.
        let call_args = self.eval_args(&args[1..])?;
        self.self_stack.borrow_mut().push(place);
        let result = match func {
            Value::Lambda(l) => self.call_lambda(&l, call_args),
            Value::Fexpr(l) => self.call_lambda(&l, call_args),
            _ => Err(Signal::Error(format!(
                "no method :{} on {}",
                method,
                self.sym_name(class)
            ))),
        };
        self.self_stack.borrow_mut().pop();
        result
    }

    /// Applying a context, dispatched on its default functor `Ctx:Ctx`
    /// (ADR-0030): a **lambda** is called (a FOOP constructor); **nil** makes the
    /// context a Dictionary (hash access); any other non-nil value (the
    /// predefined `Class` marker, copied to a FOOP class by `new`) builds a
    /// symbol-tagged object list.
    fn construct(&self, ctx: SymId, arg_exprs: &[Value]) -> Result<Value, Signal> {
        let ctx_name = self.sym_name(ctx);
        let functor = self.intern(&format!("{}:{}", ctx_name, ctx_name));
        match self.lookup(functor) {
            Value::Lambda(l) => {
                let args = self.eval_args(arg_exprs)?;
                self.call_lambda(&l, args)
            }
            Value::Nil => self.namespace_hash(ctx, arg_exprs),
            _ => {
                let mut obj = Vec::with_capacity(arg_exprs.len() + 1);
                obj.push(Value::Symbol(ctx));
                for e in arg_exprs {
                    obj.push(self.eval(e)?);
                }
                Ok(Value::list(obj))
            }
        }
    }

    /// Dictionary access on a nil-default-functor context (ADR-0030), mirroring
    /// newLISP's `evaluateNamespaceHash`. Dispatches on the first argument's
    /// type: a string/number key gets (one arg) or sets (two; a nil value
    /// deletes); a list bulk-loads `(key value)` pairs; no args (or a nil first
    /// arg) returns all associations, sorted.
    fn namespace_hash(&self, ctx: SymId, arg_exprs: &[Value]) -> Result<Value, Signal> {
        let ctx_name = self.sym_name(ctx);
        if arg_exprs.is_empty() {
            return Ok(self.dict_associations(&ctx_name));
        }
        match self.eval(&arg_exprs[0])? {
            key @ (Value::Str(_) | Value::Int(_) | Value::Float(_)) => {
                let sym = self.intern(&format!("{}:{}", ctx_name, dict_key(&key)));
                if arg_exprs.len() >= 2 {
                    let val = self.eval(&arg_exprs[1])?;
                    // Storing nil removes the key (newLISP semantics).
                    self.set_global(sym, val.clone());
                    Ok(val)
                } else {
                    Ok(self.lookup(sym))
                }
            }
            Value::List(pairs) => {
                for pair in pairs.iter() {
                    if let Value::List(kv) = pair {
                        if kv.len() >= 2 {
                            let sym = self.intern(&format!("{}:{}", ctx_name, dict_key(&kv[0])));
                            self.set_global(sym, kv[1].clone());
                        }
                    }
                }
                Ok(Value::Context(ctx))
            }
            Value::Nil => Ok(self.dict_associations(&ctx_name)),
            other => Err(Signal::Error(format!(
                "dictionary: invalid key {}",
                self.repr(&other)
            ))),
        }
    }

    /// All `(key value)` pairs of a Dictionary context, sorted by key (ADR-0030).
    /// Keys are the `_`-prefixed member symbols with the prefix stripped, and
    /// only live (non-nil) entries are included.
    fn dict_associations(&self, ctx_name: &str) -> Value {
        let mut out = Vec::new();
        for (term, val) in self.context_entries(ctx_name) {
            if let Some(key) = term.strip_prefix('_') {
                out.push(Value::list(vec![Value::str(key.as_bytes().to_vec()), val]));
            }
        }
        Value::list(out)
    }

    /// The live `(term, value)` members of a context, sorted by symbol name,
    /// with the `Ctx:` prefix stripped from the term. Skips deleted (nil) slots.
    pub fn context_entries(&self, ctx: &str) -> Vec<(String, Value)> {
        let prefix = format!("{}:", ctx);
        let ids = self.interner.borrow().context_symbols(ctx);
        let mut out = Vec::new();
        for id in ids {
            let val = self.lookup(id);
            if matches!(val, Value::Nil) {
                continue;
            }
            let name = self.sym_name(id);
            let term = name.strip_prefix(&prefix).unwrap_or(&name).to_string();
            out.push((term, val));
        }
        out
    }

    /// The member symbol ids of a context (for `delete`).
    pub fn context_symbol_ids(&self, ctx: &str) -> Vec<SymId> {
        self.interner.borrow().context_symbols(ctx)
    }

    /// The MAIN-level symbol ids (for `symbols`).
    pub fn main_symbol_ids(&self) -> Vec<SymId> {
        self.interner.borrow().main_symbols()
    }

    /// The number of bound global symbols, a rough cell count for `sys-info`.
    pub fn global_count(&self) -> usize {
        self.globals.borrow().len()
    }

    /// Whether a symbol is write-protected by `constant` (`protected?`).
    pub fn is_protected(&self, sym: SymId) -> bool {
        self.protected.borrow().contains(&sym)
    }

    /// Read and evaluate a source buffer (for `load`), returning the last form's
    /// value. Uses the same reader setup as the main loop, so top-level
    /// `(context …)` switches are honoured (ADR-0026).
    pub fn read_and_eval(&self, src: &[u8]) -> Result<Value, Signal> {
        let primitives = self.primitive_names();
        let forms = {
            let mut interner = self.interner.borrow_mut();
            let mut reader = crate::reader::Reader::new(src, &mut interner, &primitives);
            reader.read_all().map_err(Signal::Error)?
        };
        let mut result = Value::Nil;
        for form in &forms {
            result = self.eval(form)?;
        }
        Ok(result)
    }

    /// Serialise a symbol or whole context to loadable niiLISP source (ADR-0030,
    /// for `save`/`source`): a context emits `(context 'C)(context MAIN)` then a
    /// sorted `(set 'C:term value)` per live member, so re-saving an unchanged
    /// context is byte-identical; a plain symbol emits a single `set`.
    pub fn source_of(&self, id: SymId) -> String {
        let name = self.sym_name(id);
        if !name.contains(':') && matches!(self.lookup(id), Value::Context(_)) {
            let mut s = format!("(context '{})\n(context MAIN)\n", name);
            let functor = format!("{}:{}", name, name);
            for sym in self.interner.borrow().context_symbols(&name) {
                let val = self.lookup(sym);
                if matches!(val, Value::Nil) {
                    continue;
                }
                let sn = self.sym_name(sym);
                if sn == functor {
                    continue;
                }
                s.push_str(&format!("(set '{} {})\n", sn, self.repr(&val)));
            }
            s
        } else {
            format!("(set '{} {})\n", name, self.repr(&self.lookup(id)))
        }
    }

    fn current_self(&self) -> Result<Place, Signal> {
        self.self_stack
            .borrow()
            .last()
            .cloned()
            .ok_or_else(|| Signal::error("self used outside a method"))
    }

    fn read_place(&self, place: &Place) -> Result<Value, Signal> {
        // Reading never triggers write protection (a constant can be read).
        let mut g = self.globals.borrow_mut();
        let root = g.entry(place.root).or_insert(Value::Nil);
        let loc = place_navigate(root, &place.path)
            .ok_or_else(|| Signal::error("place index out of range"))?;
        Ok(loc.clone())
    }

    /// Whether the value at `place` is indexable (list/array/string), without
    /// cloning it — used by the place guard, which would otherwise copy a whole
    /// container on every indexed write (O(n) per `setf`, O(n²) in a loop).
    fn place_is_indexable(&self, place: &Place) -> Result<bool, Signal> {
        let mut g = self.globals.borrow_mut();
        let root = g.entry(place.root).or_insert(Value::Nil);
        let loc = place_navigate(root, &place.path)
            .ok_or_else(|| Signal::error("place index out of range"))?;
        Ok(matches!(
            loc,
            Value::List(_) | Value::Array(_) | Value::Str(_)
        ))
    }

    /// If `name` is context-qualified (`Ctx:member`), ensure the bare context
    /// symbol evaluates to its Context value.
    fn ensure_context(&self, qualified: SymId) {
        let name = self.sym_name(qualified);
        if let Some((ctx, _)) = name.split_once(':') {
            if !ctx.is_empty() {
                let cid = self.intern(ctx);
                if !matches!(self.lookup(cid), Value::Context(_)) {
                    self.set_global(cid, Value::Context(cid));
                }
            }
        }
    }

    /// A number in functor position is implicit **rest/slice** (not element
    /// access — that is `(seq i)`, a list/array in functor position): `(i seq)`
    /// is the sub-sequence from offset `i` to the end; `(i len seq)` takes `len`
    /// elements (a negative `len` counts from the end). newLISP manual,
    /// "Implicit indexing for rest and slice".
    fn implicit_index(&self, i: i64, rest: &[Value]) -> Result<Value, Signal> {
        match rest.len() {
            1 => {
                let target = self.eval(&rest[0])?;
                Ok(slice_seq(i, None, &target))
            }
            2 => {
                let len = match self.eval(&rest[0])? {
                    Value::Int(n) => n,
                    _ => return Err(Signal::error("implicit slice: length must be an integer")),
                };
                let target = self.eval(&rest[1])?;
                Ok(slice_seq(i, Some(len), &target))
            }
            _ => Err(Signal::error(
                "implicit index: expected (i seq) or (i len seq)",
            )),
        }
    }

    /// Call an already-resolved callable with already-evaluated arguments.
    /// Used by higher-order builtins (`map`, `apply`, `filter`).
    pub fn call(&self, callable: &Value, args: Vec<Value>) -> Result<Value, Signal> {
        match callable {
            Value::Builtin(b) => (b.func)(self, &args),
            Value::Lambda(l) => self.call_lambda(l, args),
            Value::Fexpr(l) => self.call_lambda(l, args),
            Value::Foreign(f) => f.call(&args),
            // A lambda-headed list built as data (ADR-0027); args are already
            // evaluated, so parse the params/body and call directly.
            Value::List(l) if self.lambda_head(l).is_some() => {
                let params = match l.get(1) {
                    Some(Value::List(p)) => self.parse_params(p)?,
                    _ => Vec::new(),
                };
                let lam = Lambda {
                    params,
                    body: l.get(2..).unwrap_or(&[]).to_vec(),
                };
                self.call_lambda(&lam, args)
            }
            // A special-form name applied as a function (e.g. `(apply and list)`
            // or `(map set '(a b) '(1 2))`): dispatch it with the already-
            // evaluated operands wrapped in `quote`, so the form's own
            // evaluation yields each value back rather than re-evaluating it
            // (which would treat a symbol operand as a variable) (ADR-0027).
            Value::Symbol(id) => {
                let name = self.sym_name(*id);
                let quote = self.intern("quote");
                let quoted: Vec<Value> = args
                    .iter()
                    .map(|v| Value::list(vec![Value::Symbol(quote), v.clone()]))
                    .collect();
                match self.try_special_form(&name, &quoted) {
                    Some(r) => r,
                    None => Err(Signal::Error(format!("not a function: {}", name))),
                }
            }
            other => Err(Signal::Error(format!(
                "not a function: {}",
                self.repr(other)
            ))),
        }
    }

    /// Apply a callable to unevaluated argument expressions, evaluating them
    /// (or not, for a fexpr) as appropriate.
    fn apply(&self, callable: Value, arg_exprs: &[Value]) -> Result<Value, Signal> {
        match callable {
            Value::Builtin(b) => {
                let args = self.eval_args(arg_exprs)?;
                (b.func)(self, &args)
            }
            Value::Lambda(l) => {
                let args = self.eval_args(arg_exprs)?;
                self.call_lambda(&l, args)
            }
            Value::Fexpr(l) => {
                let args: Vec<Value> = arg_exprs.to_vec();
                self.call_lambda(&l, args)
            }
            Value::Foreign(f) => {
                let args = self.eval_args(arg_exprs)?;
                f.call(&args)
            }
            other => Err(Signal::Error(format!(
                "not a function: {}",
                self.repr(&other)
            ))),
        }
    }

    fn eval_args(&self, arg_exprs: &[Value]) -> Result<Vec<Value>, Signal> {
        let mut args = Vec::with_capacity(arg_exprs.len());
        for e in arg_exprs {
            args.push(self.eval(e)?);
        }
        Ok(args)
    }

    /// Bind parameters (dynamically) and evaluate the body. Missing arguments
    /// fall back to the parameter's default (e.g. `(r 0)`), else nil.
    fn call_lambda(&self, l: &Lambda, args: Vec<Value>) -> Result<Value, Signal> {
        let mut scope = Scope::new(self);
        let mut args = args.into_iter();
        for p in &l.params {
            let v = match args.next() {
                Some(v) => v,
                None => match &p.default {
                    Some(d) => self.eval(d)?,
                    None => Value::Nil,
                },
            };
            scope.bind(p.sym, v);
        }
        // Arguments past the declared parameters are available via `(args)`
        // (ADR-0027); the guard pops them as the call unwinds.
        self.args_stack.borrow_mut().push(args.collect());
        let _args_guard = ArgsGuard(self);
        self.eval_body(&l.body)
        // `_args_guard` then `scope` drop here, restoring call state.
    }

    /// The current call's arguments not bound to a parameter, for `(args)`.
    pub fn current_args(&self) -> Vec<Value> {
        self.args_stack.borrow().last().cloned().unwrap_or_default()
    }

    /// Compile (and cache) a regex over the byte string, mapping the relevant
    /// PCRE option bits (ADR-0028). Errors on a malformed pattern.
    #[cfg(feature = "regex")]
    pub fn compiled_regex(
        &self,
        pattern: &str,
        option: i64,
    ) -> Result<regex::bytes::Regex, Signal> {
        let key = (pattern.to_string(), option);
        if let Some(re) = self.regex_cache.borrow().get(&key) {
            return Ok(re.clone());
        }
        let mut builder = regex::bytes::RegexBuilder::new(pattern);
        builder.case_insensitive(option & 1 != 0); // PCRE_CASELESS
        builder.multi_line(option & 0x2 != 0); // PCRE_MULTILINE
        builder.dot_matches_new_line(option & 0x4 != 0); // PCRE_DOTALL
                                                         // 0x800 (PCRE_UTF8) is a no-op: the crate is Unicode by default.
        let re = builder
            .build()
            .map_err(|e| Signal::Error(format!("regex: {}", e)))?;
        self.regex_cache.borrow_mut().insert(key, re.clone());
        Ok(re)
    }

    /// Bind the regex system variables `$0`..`$N` to the whole match and each
    /// capture group, matching newLISP. They are ordinary globals that persist
    /// until the next regex operation. A non-participating group binds to nil.
    #[cfg(feature = "regex")]
    pub(crate) fn set_regex_captures(&self, caps: &regex::bytes::Captures) {
        for (i, m) in caps.iter().enumerate() {
            let sym = self.intern(&format!("${}", i));
            let v = m
                .map(|mm| Value::str(mm.as_bytes().to_vec()))
                .unwrap_or(Value::Nil);
            self.set_global(sym, v);
        }
    }

    // ---- Special forms ---------------------------------------------------

    fn try_special_form(&self, name: &str, args: &[Value]) -> Option<Result<Value, Signal>> {
        let r = match name {
            "quote" => Ok(args.first().cloned().unwrap_or(Value::Nil)),
            "if" => self.sf_if(args),
            "if-not" => self.sf_if_not(args),
            "cond" => self.sf_cond(args),
            "case" => self.sf_case(args),
            "and" => self.sf_and(args),
            "or" => self.sf_or(args),
            "while" => self.sf_while(args),
            "when" => self.sf_when(args, true),
            "unless" => self.sf_when(args, false),
            "dotimes" => self.sf_dotimes(args),
            "for" => self.sf_for(args),
            "dolist" => self.sf_dolist(args),
            "dostring" => self.sf_dostring(args),
            "dotree" => self.sf_dotree(args),
            "context" => self.sf_context(args),
            "until" => self.sf_until(args),
            "do-until" => self.sf_do_loop(args, true),
            "do-while" => self.sf_do_loop(args, false),
            "amb" => self.sf_amb(args),
            "extend" => self.sf_extend(args),
            "local" => self.sf_local(args),
            "++" | "inc" => self.sf_incr(args, 1),
            "--" | "dec" => self.sf_incr(args, -1),
            "push" => self.sf_push(args),
            "pop" => self.sf_pop(args),
            "swap" => self.sf_swap(args),
            "reverse" => self.sf_reverse(args),
            "sort" => self.sf_sort(args),
            "rotate" => self.sf_rotate(args),
            "replace" => self.sf_replace(args),
            "set-ref" => self.sf_setref(args, false),
            "set-ref-all" => self.sf_setref(args, true),
            "find-all" => self.sf_find_all(args),
            "pop-assoc" => self.sf_pop_assoc(args),
            "begin" => self.eval_body(args),
            "define" => self.sf_define(args),
            "set" => self.sf_set(args, true),
            "setq" | "setf" => self.sf_setf(args),
            "constant" => self.sf_constant(args),
            "self" => self.sf_self(args),
            "time" => self.sf_time(args),
            "let" => self.sf_let(args),
            "letn" => self.sf_letn(args),
            "letex" => self.sf_letex(args),
            "curry" => self.sf_curry(args),
            "lambda" | "fn" => self.sf_lambda(args, false),
            "lambda-macro" => self.sf_lambda(args, true),
            "define-macro" => self.sf_define_macro(args),
            "catch" => self.sf_catch(args),
            "throw" => self.sf_throw(args),
            "read-buffer" => self.sf_read_buffer(args),
            #[cfg(all(feature = "mt", unix))]
            "spawn" => crate::process::sf_spawn(self, args),
            #[cfg(all(feature = "mt", unix))]
            "fork" => crate::process::sf_fork(self, args),
            #[cfg(all(feature = "mt", unix))]
            "receive" => self.sf_receive(args),
            // `net-receive` shares `read-buffer`'s place-taking semantics.
            #[cfg(all(feature = "net", unix))]
            "net-receive" => self.sf_read_buffer(args),
            _ => return None,
        };
        Some(r)
    }

    /// `(receive)` returns the peer pids with a message ready; `(receive pid
    /// place)` reads one message from `pid` into `place` (unevaluated, hence a
    /// special form), returning `true`, or `nil` if none is waiting (ADR-0032).
    #[cfg(all(feature = "mt", unix))]
    fn sf_receive(&self, args: &[Value]) -> Result<Value, Signal> {
        if args.is_empty() {
            return Ok(Value::list(crate::process::receive_ready(self)));
        }
        let pid = match self.eval(&args[0])? {
            Value::Int(n) => n,
            Value::Float(f) => f as i64,
            _ => return Err(Signal::error("receive: pid must be an integer")),
        };
        let place = self.resolve_place(
            args.get(1)
                .ok_or_else(|| Signal::error("receive: missing target place"))?,
        )?;
        match crate::process::receive_one(self, pid) {
            Some(v) => {
                self.with_place_mut(&place, move |loc| {
                    *loc = v;
                    Ok(Value::Nil)
                })?;
                Ok(Value::True)
            }
            None => Ok(Value::Nil),
        }
    }

    /// `(read-buffer handle place size [wait-str])` — reads up to `size` bytes
    /// (or until `wait-str`) from `handle`, assigns the string into `place` (a
    /// symbol or other place), and returns the byte count (ADR-0029). `place` is
    /// unevaluated, which is why this is a special form.
    fn sf_read_buffer(&self, args: &[Value]) -> Result<Value, Signal> {
        let handle = self.eval(
            args.first()
                .ok_or_else(|| Signal::error("read-buffer: missing handle"))?,
        )?;
        let h = match handle {
            Value::Int(n) => n,
            Value::Float(f) => f as i64,
            _ => return Err(Signal::error("read-buffer: handle must be an integer")),
        };
        let place = self.resolve_place(
            args.get(1)
                .ok_or_else(|| Signal::error("read-buffer: missing target place"))?,
        )?;
        let size = match args.get(2) {
            Some(e) => match self.eval(e)? {
                Value::Int(n) => n,
                Value::Float(f) => f as i64,
                _ => return Err(Signal::error("read-buffer: size must be an integer")),
            },
            None => return Err(Signal::error("read-buffer: missing size")),
        };
        let wait = match args.get(3) {
            Some(e) => match self.eval(e)? {
                Value::Str(b) => Some(b.to_vec()),
                Value::Nil => None,
                _ => return Err(Signal::error("read-buffer: wait must be a string")),
            },
            None => None,
        };
        let (bytes, n) = crate::fileio::read_buffer(self, h, size, wait)?;
        let v = Value::str(bytes);
        self.with_place_mut(&place, move |loc| {
            *loc = v;
            Ok(Value::Nil)
        })?;
        Ok(Value::Int(n as i64))
    }

    fn sf_if(&self, args: &[Value]) -> Result<Value, Signal> {
        // (if c1 e1 c2 e2 ... [else]) — newLISP multiway if.
        let mut i = 0;
        while i + 1 < args.len() {
            if self.eval(&args[i])?.is_truthy() {
                return self.eval(&args[i + 1]);
            }
            i += 2;
        }
        // Optional trailing else expression.
        if i < args.len() {
            self.eval(&args[i])
        } else {
            Ok(Value::Nil)
        }
    }

    fn sf_if_not(&self, args: &[Value]) -> Result<Value, Signal> {
        // (if-not cond then [else]) — the inverse of the two/three-arg `if`.
        let cond = self.eval(
            args.first()
                .ok_or_else(|| Signal::error("if-not: no condition"))?,
        )?;
        if !cond.is_truthy() {
            self.eval(args.get(1).unwrap_or(&Value::Nil))
        } else {
            match args.get(2) {
                Some(e) => self.eval(e),
                None => Ok(Value::Nil),
            }
        }
    }

    fn sf_cond(&self, args: &[Value]) -> Result<Value, Signal> {
        for clause in args {
            if let Value::List(parts) = clause {
                if let Some(test) = parts.first() {
                    if self.eval(test)?.is_truthy() {
                        return self.eval_body(&parts[1..]);
                    }
                }
            } else {
                return Err(Signal::error("cond: each clause must be a list"));
            }
        }
        Ok(Value::Nil)
    }

    fn sf_case(&self, args: &[Value]) -> Result<Value, Signal> {
        // (case key (label body...) ... (true default...)) — labels are literal.
        let key = self.eval(args.first().ok_or_else(|| Signal::error("case: no key"))?)?;
        for clause in &args[1..] {
            if let Value::List(parts) = clause {
                if let Some(label) = parts.first() {
                    if self.case_label_matches(label, &key) {
                        return self.eval_body(&parts[1..]);
                    }
                }
            }
        }
        Ok(Value::Nil)
    }

    /// Whether a `case` clause label matches the key. `true`/`t` is the default.
    fn case_label_matches(&self, label: &Value, key: &Value) -> bool {
        matches!(label, Value::True)
            || matches!(label, Value::Symbol(s) if {
                let n = self.sym_name(*s);
                n == "true" || n == "t"
            })
            || crate::builtins::values_equal(label, key)
    }

    fn sf_and(&self, args: &[Value]) -> Result<Value, Signal> {
        let mut result = Value::True;
        for a in args {
            result = self.eval(a)?;
            if !result.is_truthy() {
                return Ok(Value::Nil);
            }
        }
        Ok(result)
    }

    fn sf_or(&self, args: &[Value]) -> Result<Value, Signal> {
        for a in args {
            let v = self.eval(a)?;
            if v.is_truthy() {
                return Ok(v);
            }
        }
        Ok(Value::Nil)
    }

    fn sf_while(&self, args: &[Value]) -> Result<Value, Signal> {
        let cond = args
            .first()
            .ok_or_else(|| Signal::error("while: missing condition"))?;
        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(idx_sym, Value::Int(0));
        let mut result = Value::Nil;
        let mut i: i64 = 0;
        loop {
            self.set_global(idx_sym, Value::Int(i));
            if !self.eval(cond)?.is_truthy() {
                break;
            }
            result = self.eval_body(&args[1..])?;
            i += 1;
        }
        Ok(result)
    }

    fn sf_until(&self, args: &[Value]) -> Result<Value, Signal> {
        // (until cond body...) — the inverse of `while`: loop while cond is false.
        let cond = args
            .first()
            .ok_or_else(|| Signal::error("until: missing condition"))?;
        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(idx_sym, Value::Int(0));
        let mut result = Value::Nil;
        let mut i: i64 = 0;
        loop {
            self.set_global(idx_sym, Value::Int(i));
            if self.eval(cond)?.is_truthy() {
                break;
            }
            result = self.eval_body(&args[1..])?;
            i += 1;
        }
        Ok(result)
    }

    /// `(do-until cond body...)` / `(do-while cond body...)` — post-test loops
    /// that run the body at least once, then repeat until `cond` is true
    /// (`do-until`) or while `cond` is true (`do-while`). The condition is the
    /// first form and is checked after each body pass.
    fn sf_do_loop(&self, args: &[Value], until: bool) -> Result<Value, Signal> {
        let cond = args
            .first()
            .ok_or_else(|| Signal::error("do-until/do-while: missing condition"))?;
        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(idx_sym, Value::Int(0));
        let mut result;
        let mut i: i64 = 0;
        loop {
            self.set_global(idx_sym, Value::Int(i));
            result = self.eval_body(&args[1..])?;
            i += 1;
            if self.eval(cond)?.is_truthy() == until {
                break;
            }
        }
        Ok(result)
    }

    fn sf_amb(&self, args: &[Value]) -> Result<Value, Signal> {
        // (amb expr...) — evaluate exactly one argument, chosen at random.
        if args.is_empty() {
            return Ok(Value::Nil);
        }
        let idx = (self.rng_next_u64() % args.len() as u64) as usize;
        self.eval(&args[idx])
    }

    fn sf_extend(&self, args: &[Value]) -> Result<Value, Signal> {
        // (extend place rest...) — destructively append to a string or list
        // place: strings concatenate, lists splice each argument's elements.
        let place = self.resolve_place(
            args.first()
                .ok_or_else(|| Signal::error("extend: expected a place"))?,
        )?;
        let mut additions = Vec::with_capacity(args.len().saturating_sub(1));
        for e in &args[1..] {
            additions.push(self.eval(e)?);
        }
        self.with_place_mut(&place, move |loc| {
            // An unset place adopts a string if every addition is a string.
            if matches!(loc, Value::Nil) {
                *loc = if additions.iter().all(|a| matches!(a, Value::Str(_))) {
                    Value::str(Vec::new())
                } else {
                    Value::list(Vec::new())
                };
            }
            match loc {
                Value::Str(buf) => {
                    let buf = Rc::make_mut(buf);
                    for a in &additions {
                        match a {
                            Value::Str(b) => buf.extend_from_slice(b),
                            _ => {
                                return Err(Signal::error(
                                    "extend: a string place takes string arguments",
                                ))
                            }
                        }
                    }
                }
                Value::List(list) => {
                    let list = Rc::make_mut(list);
                    for a in &additions {
                        match a {
                            Value::List(items) => list.extend(items.iter().cloned()),
                            other => list.push(other.clone()),
                        }
                    }
                }
                _ => return Err(Signal::error("extend: place is not a string or list")),
            }
            Ok(loc.clone())
        })
    }

    fn sf_when(&self, args: &[Value], positive: bool) -> Result<Value, Signal> {
        let cond = args
            .first()
            .ok_or_else(|| Signal::error("when/unless: missing condition"))?;
        if self.eval(cond)?.is_truthy() == positive {
            self.eval_body(&args[1..])
        } else {
            Ok(Value::Nil)
        }
    }

    fn sf_dotimes(&self, args: &[Value]) -> Result<Value, Signal> {
        // (dotimes (var count) body...)
        let spec = match args.first() {
            Some(Value::List(s)) => s,
            _ => return Err(Signal::error("dotimes: expected (var count)")),
        };
        let var = match spec.first() {
            Some(Value::Symbol(id)) => *id,
            _ => return Err(Signal::error("dotimes: expected a loop variable")),
        };
        let count = match self.eval(spec.get(1).unwrap_or(&Value::Nil))? {
            Value::Int(n) => n,
            Value::Float(f) => f as i64,
            _ => return Err(Signal::error("dotimes: count must be a number")),
        };

        let mut scope = Scope::new(self);
        scope.bind(var, Value::Int(0));
        let mut result = Value::Nil;
        let mut i = 0;
        while i < count {
            self.set_global(var, Value::Int(i));
            result = self.eval_body(&args[1..])?;
            i += 1;
        }
        Ok(result)
    }

    fn sf_incr(&self, args: &[Value], sign: i64) -> Result<Value, Signal> {
        // (++ place [amount]) / (inc place [amount]) — mutate a numeric place.
        let place = self.resolve_place(
            args.first()
                .ok_or_else(|| Signal::error("++/--: expected a place"))?,
        )?;
        let delta = match args.get(1) {
            Some(e) => self.eval(e)?,
            None => Value::Int(1),
        };
        let apply = |v: &mut Value| -> Result<Value, Signal> {
            let next = incr_value(v, sign, &delta)?;
            *v = next.clone();
            Ok(next)
        };
        self.with_place_mut(&place, apply)
    }

    fn sf_setf(&self, args: &[Value]) -> Result<Value, Signal> {
        // (setf place value [place value ...]) — assign into places.
        if args.len() % 2 != 0 {
            return Err(Signal::error("setf: expected place/value pairs"));
        }
        let it_sym = self.intern("$it");
        let mut last = Value::Nil;
        let mut i = 0;
        while i + 1 < args.len() {
            let place = self.resolve_place(&args[i])?;
            // Bind `$it` to the place's current value while evaluating the new one.
            let old = self.read_place(&place)?;
            let val = {
                let mut scope = Scope::new(self);
                scope.bind(it_sym, old);
                self.eval(&args[i + 1])?
            };
            let v = val.clone();
            self.with_place_mut(&place, move |loc| {
                *loc = v;
                Ok(Value::Nil)
            })?;
            last = val;
            i += 2;
        }
        Ok(last)
    }

    fn sf_for(&self, args: &[Value]) -> Result<Value, Signal> {
        // (for (var from to [step]) body...) — inclusive; direction auto.
        let spec = match args.first() {
            Some(Value::List(s)) if s.len() >= 3 => s,
            _ => return Err(Signal::error("for: expected (var from to [step])")),
        };
        let var = match &spec[0] {
            Value::Symbol(id) => *id,
            _ => return Err(Signal::error("for: expected a loop variable")),
        };
        let from = self.eval(&spec[1])?;
        let to = self.eval(&spec[2])?;
        let step = match spec.get(3) {
            Some(e) => Some(self.eval(e)?),
            None => None,
        };

        let integral = is_int(&from) && is_int(&to) && step.as_ref().map(is_int).unwrap_or(true);
        let fromf = num(&from)?;
        let tof = num(&to)?;
        let stepf = match &step {
            Some(v) => num(v)?.abs(),
            None => 1.0,
        };
        if stepf == 0.0 {
            return Err(Signal::error("for: step must be non-zero"));
        }
        let ascending = tof >= fromf;
        let signed = if ascending { stepf } else { -stepf };

        let mut scope = Scope::new(self);
        scope.bind(var, Value::Nil);
        let mut result = Value::Nil;
        let mut v = fromf;
        while (ascending && v <= tof) || (!ascending && v >= tof) {
            let bound = if integral {
                Value::Int(v as i64)
            } else {
                Value::Float(v)
            };
            self.set_global(var, bound);
            result = self.eval_body(&args[1..])?;
            v += signed;
        }
        Ok(result)
    }

    fn sf_dolist(&self, args: &[Value]) -> Result<Value, Signal> {
        // (dolist (var list [break-cond]) body...)
        let spec = match args.first() {
            Some(Value::List(s)) if s.len() >= 2 => s,
            _ => return Err(Signal::error("dolist: expected (var list)")),
        };
        let var = match &spec[0] {
            Value::Symbol(id) => *id,
            _ => return Err(Signal::error("dolist: expected a loop variable")),
        };
        // Keep the list shared (Rc) and iterate by reference — no full copy
        // (copy-on-write, ADR-0024); only each loop-var binding is cloned.
        let items = match self.eval(&spec[1])? {
            Value::List(l) => l,
            Value::Nil => Rc::new(Vec::new()),
            other => {
                return Err(Signal::Error(format!(
                    "dolist: expected a list, got {}",
                    self.repr(&other)
                )))
            }
        };
        let break_cond = spec.get(2);

        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(var, Value::Nil);
        scope.bind(idx_sym, Value::Int(0));
        let mut result = Value::Nil;
        for (i, item) in items.iter().enumerate() {
            self.set_global(var, item.clone());
            self.set_global(idx_sym, Value::Int(i as i64));
            if let Some(cond) = break_cond {
                if self.eval(cond)?.is_truthy() {
                    break;
                }
            }
            result = self.eval_body(&args[1..])?;
        }
        Ok(result)
    }

    fn sf_dostring(&self, args: &[Value]) -> Result<Value, Signal> {
        // (dostring (var str [break-cond]) body...) — var := each character's
        // Unicode code point. Iteration is character-based (ADR-0025), matching
        // newLISP's UTF-8 build; for an ASCII string a code point is its byte.
        let spec = match args.first() {
            Some(Value::List(s)) if s.len() >= 2 => s,
            _ => return Err(Signal::error("dostring: expected (var string)")),
        };
        let var = match &spec[0] {
            Value::Symbol(id) => *id,
            _ => return Err(Signal::error("dostring: expected a loop variable")),
        };
        let bytes = match self.eval(&spec[1])? {
            Value::Str(b) => b,
            Value::Nil => Rc::new(Vec::new()),
            other => {
                return Err(Signal::Error(format!(
                    "dostring: expected a string, got {}",
                    self.repr(&other)
                )))
            }
        };
        let break_cond = spec.get(2);

        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(var, Value::Nil);
        scope.bind(idx_sym, Value::Int(0));
        let mut result = Value::Nil;
        for (i, cp) in crate::utf8::codepoints(&bytes).enumerate() {
            self.set_global(var, Value::Int(i64::from(cp)));
            self.set_global(idx_sym, Value::Int(i as i64));
            if let Some(cond) = break_cond {
                if self.eval(cond)?.is_truthy() {
                    break;
                }
            }
            result = self.eval_body(&args[1..])?;
        }
        Ok(result)
    }

    fn sf_context(&self, args: &[Value]) -> Result<Value, Signal> {
        // (context) with no argument returns the current context (ADR-0026).
        if args.is_empty() {
            return Ok(Value::Context(*self.current_ctx.borrow()));
        }
        let cid = self.context_target(args.first())?;
        if self.sym_name(cid) != "MAIN" && !matches!(self.lookup(cid), Value::Context(_)) {
            self.set_global(cid, Value::Context(cid));
        }
        // (context ctx word [value]) creates a symbol `ctx:word` (a nil-free
        // way to build data structures) and optionally sets its value, returning
        // the symbol. `word` is a string or a symbol whose term is used.
        if args.len() >= 2 {
            let term = match self.eval(&args[1])? {
                Value::Str(b) => String::from_utf8_lossy(&b).into_owned(),
                Value::Symbol(id) => {
                    let n = self.sym_name(id);
                    n.rsplit(':').next().unwrap_or(&n).to_string()
                }
                other => self.repr(&other),
            };
            let qualified = self.intern(&format!("{}:{}", self.sym_name(cid), term));
            if let Some(v) = args.get(2) {
                let val = self.eval(v)?;
                self.set_global(qualified, val);
            }
            return Ok(Value::Symbol(qualified));
        }
        // (context 'X) / (context X): switch the runtime current context. The
        // reader has already switched it for symbol qualification; here we
        // register the context value so `X` evaluates to it and track it for the
        // no-arg query form.
        *self.current_ctx.borrow_mut() = cid;
        Ok(Value::Context(cid))
    }

    /// The context symbol named by a `(context …)` argument: `'X`, bare `X`, or a
    /// value that evaluates to a context. Context names are MAIN-level, so any
    /// context prefix on the name is stripped to its term (ADR-0026).
    fn context_target(&self, arg: Option<&Value>) -> Result<SymId, Signal> {
        let arg = arg.ok_or_else(|| Signal::error("context: missing name"))?;
        let name = match arg {
            Value::Symbol(id) => self.sym_name(*id),
            Value::List(l)
                if l.len() == 2
                    && matches!(&l[0], Value::Symbol(s) if self.sym_name(*s) == "quote") =>
            {
                match &l[1] {
                    Value::Symbol(id) => self.sym_name(*id),
                    _ => return Err(Signal::error("context: bad name")),
                }
            }
            other => match self.eval(other)? {
                Value::Context(id) => return Ok(id),
                Value::Symbol(id) => self.sym_name(id),
                _ => return Err(Signal::error("context: expected a context name")),
            },
        };
        let term = name.rsplit(':').next().unwrap_or(&name).to_string();
        Ok(self.intern(&term))
    }

    fn sf_dotree(&self, args: &[Value]) -> Result<Value, Signal> {
        // (dotree (var ctx [only-toplevel]) body...) — bind var to each symbol of
        // context ctx (ADR-0026), in name order.
        let spec = match args.first() {
            Some(Value::List(s)) if s.len() >= 2 => s,
            _ => return Err(Signal::error("dotree: expected (var context)")),
        };
        let var = match &spec[0] {
            Value::Symbol(id) => *id,
            _ => return Err(Signal::error("dotree: expected a loop variable")),
        };
        let ctx_name = match self.eval(&spec[1])? {
            Value::Context(id) | Value::Symbol(id) => self.sym_name(id),
            other => {
                return Err(Signal::Error(format!(
                    "dotree: expected a context, got {}",
                    self.repr(&other)
                )))
            }
        };
        let only_toplevel = match spec.get(2) {
            Some(e) => self.eval(e)?.is_truthy(),
            None => false,
        };
        let syms = self.interner.borrow().context_symbols(&ctx_name);

        let idx_sym = self.intern("$idx");
        let mut scope = Scope::new(self);
        scope.bind(var, Value::Nil);
        scope.bind(idx_sym, Value::Int(0));
        let mut result = Value::Nil;
        let mut i: i64 = 0;
        for sym in syms {
            if only_toplevel {
                let name = self.sym_name(sym);
                if name.rsplit(':').next().unwrap_or(&name).starts_with('_') {
                    continue;
                }
            }
            self.set_global(var, Value::Symbol(sym));
            self.set_global(idx_sym, Value::Int(i));
            result = self.eval_body(&args[1..])?;
            i += 1;
        }
        Ok(result)
    }

    fn sf_local(&self, args: &[Value]) -> Result<Value, Signal> {
        // (local (a b c) body...) — bind each symbol to nil for the body.
        let syms = match args.first() {
            Some(Value::List(s)) => s,
            _ => return Err(Signal::error("local: expected a symbol list")),
        };
        let mut scope = Scope::new(self);
        for s in syms.iter() {
            match s {
                Value::Symbol(id) => scope.bind(*id, Value::Nil),
                _ => return Err(Signal::error("local: expected a symbol")),
            }
        }
        self.eval_body(&args[1..])
    }

    /// Collect an index path from a run of arguments, each evaluating to an
    /// integer (one step) or a list of integers (a full path) — so `push`/`pop`
    /// accept both `(pop L 1 0)` and `(pop L (ref key L))` (ADR-0036).
    fn index_path(&self, args: &[Value]) -> Result<Vec<i64>, Signal> {
        let mut path = Vec::new();
        for e in args {
            match self.eval(e)? {
                Value::Int(n) => path.push(n),
                Value::List(l) => {
                    for v in l.iter() {
                        match v {
                            Value::Int(n) => path.push(*n),
                            _ => return Err(Signal::error("index path must be integers")),
                        }
                    }
                }
                Value::Nil => {}
                _ => {
                    return Err(Signal::error(
                        "index must be an integer or a list of integers",
                    ))
                }
            }
        }
        Ok(path)
    }

    fn sf_push(&self, args: &[Value]) -> Result<Value, Signal> {
        // (push value place [index...]) — place designates a list (or nil ->
        // list). Trailing indices navigate a nested list; the last is the
        // insertion position (ADR-0036).
        if args.len() < 2 {
            return Err(Signal::error("push: expected (push value place [index])"));
        }
        let value = self.eval(&args[0])?;
        let mut place = self.resolve_place(&args[1])?;
        let mut idxpath = self.index_path(&args[2..])?;
        let index = idxpath.pop();
        place.path.extend(idxpath);
        self.with_place_mut(&place, move |loc| {
            let list = match loc {
                Value::List(l) => Rc::make_mut(l),
                Value::Nil => {
                    *loc = Value::list(Vec::new());
                    match loc {
                        Value::List(l) => Rc::make_mut(l),
                        _ => unreachable!(),
                    }
                }
                _ => return Err(Signal::error("push: place is not a list")),
            };
            let at = match index {
                None => 0,
                Some(i) if i < 0 => (list.len() as i64 + 1 + i).max(0) as usize,
                Some(i) => (i as usize).min(list.len()),
            };
            list.insert(at.min(list.len()), value);
            Ok(loc.clone())
        })
    }

    fn sf_swap(&self, args: &[Value]) -> Result<Value, Signal> {
        // (swap place-a place-b) — exchange the values at two places, returning
        // the first place's new value. Writes replace values (no resize), so two
        // places into the same list stay valid across the exchange.
        if args.len() != 2 {
            return Err(Signal::error("swap: expected (swap place-a place-b)"));
        }
        let pa = self.resolve_place(&args[0])?;
        let pb = self.resolve_place(&args[1])?;
        let va = self.with_place_mut(&pa, |v| Ok(v.clone()))?;
        let vb = self.with_place_mut(&pb, |v| Ok(v.clone()))?;
        self.with_place_mut(&pa, |v| {
            *v = vb.clone();
            Ok(())
        })?;
        self.with_place_mut(&pb, |v| {
            *v = va;
            Ok(())
        })?;
        Ok(vb)
    }

    fn sf_pop(&self, args: &[Value]) -> Result<Value, Signal> {
        // (pop place [index...]) — remove and return an element. Trailing indices
        // navigate a nested list; the last is the position (ADR-0036).
        let mut place = self.resolve_place(
            args.first()
                .ok_or_else(|| Signal::error("pop: expected a place"))?,
        )?;
        let mut idxpath = self.index_path(&args[1..])?;
        let index = idxpath.pop().unwrap_or(0);
        place.path.extend(idxpath);
        self.with_place_mut(&place, move |loc| {
            let list = match loc {
                Value::List(l) => Rc::make_mut(l),
                _ => return Err(Signal::error("pop: place is not a list")),
            };
            if list.is_empty() {
                return Ok(Value::Nil);
            }
            let at = if index < 0 {
                (list.len() as i64 + index).max(0) as usize
            } else {
                (index as usize).min(list.len() - 1)
            };
            Ok(list.remove(at))
        })
    }

    // ---- destructive, place-aware operations (qa-ref) --------------------

    /// Apply an in-place transform to a place, or to a fresh value if the
    /// argument is not a place (e.g. the copy returned by `append`).
    fn place_or_value(&self, target: &Value, op: impl Fn(&mut Value)) -> Result<Value, Signal> {
        match self.resolve_place(target) {
            Ok(place) => self.with_place_mut(&place, |loc| {
                op(loc);
                Ok(loc.clone())
            }),
            Err(_) => {
                let mut v = self.eval(target)?;
                op(&mut v);
                Ok(v)
            }
        }
    }

    fn sf_reverse(&self, args: &[Value]) -> Result<Value, Signal> {
        let target = args
            .first()
            .ok_or_else(|| Signal::error("reverse: missing argument"))?;
        self.place_or_value(target, |loc| match loc {
            Value::List(l) => Rc::make_mut(l).reverse(),
            Value::Str(b) => Rc::make_mut(b).reverse(),
            _ => {}
        })
    }

    fn sf_rotate(&self, args: &[Value]) -> Result<Value, Signal> {
        let target = args
            .first()
            .ok_or_else(|| Signal::error("rotate: missing argument"))?;
        let n = match args.get(1) {
            Some(e) => self.eval_index(e)?,
            None => 1,
        };
        self.place_or_value(target, |loc| rotate_value(loc, n))
    }

    fn sf_sort(&self, args: &[Value]) -> Result<Value, Signal> {
        let target = args
            .first()
            .ok_or_else(|| Signal::error("sort: missing argument"))?;
        match self.resolve_place(target) {
            Ok(place) => self.with_place_mut(&place, |loc| {
                if let Value::List(l) = loc {
                    Rc::make_mut(l).sort_by(|a, b| self.value_cmp(a, b));
                }
                Ok(loc.clone())
            }),
            Err(_) => {
                let mut v = self.eval(target)?;
                if let Value::List(l) = &mut v {
                    Rc::make_mut(l).sort_by(|a, b| self.value_cmp(a, b));
                }
                Ok(v)
            }
        }
    }

    fn sf_replace(&self, args: &[Value]) -> Result<Value, Signal> {
        if args.len() < 3 {
            return Err(Signal::error(
                "replace: expected (replace target place new)",
            ));
        }
        let target = self.eval(&args[0])?;
        let place = self.resolve_place(&args[1]).ok();
        let data_val = match &place {
            Some(p) => self.read_place(p)?,
            None => self.eval(&args[1])?,
        };
        // String replacement (newLISP): the replacement expression is
        // re-evaluated per match with $0..$N bound. A literal key matches
        // literally; a 4th argument (a regex option) switches the key to a
        // regular expression.
        #[cfg(feature = "regex")]
        if let (Value::Str(key), Value::Str(data)) = (&target, &data_val) {
            let (pattern, option) = if args.len() >= 4 {
                let opt = match self.eval(&args[3])? {
                    Value::Int(n) => n,
                    Value::Float(f) => f as i64,
                    _ => 0,
                };
                (String::from_utf8_lossy(key).into_owned(), opt)
            } else {
                (regex::escape(&String::from_utf8_lossy(key)), 0)
            };
            return self.replace_regex(place, data.to_vec(), &pattern, option, &args[2]);
        }
        // List (or other) replacement: a single evaluation of `new`.
        let newval = self.eval(&args[2])?;
        let mut v = data_val;
        do_replace(&mut v, &target, &newval);
        if let Some(p) = place {
            let r = v.clone();
            self.with_place_mut(&p, move |loc| {
                *loc = r;
                Ok(Value::Nil)
            })?;
        }
        Ok(v)
    }

    /// String `replace` inner loop: for each match of `pattern` over `data`,
    /// bind $0..$N, re-evaluate `repl_expr`, and splice its (string) result in;
    /// a nil result deletes the match. Writes the result back to `place` if the
    /// data came from one, and returns the new string.
    #[cfg(feature = "regex")]
    fn replace_regex(
        &self,
        place: Option<Place>,
        data: Vec<u8>,
        pattern: &str,
        option: i64,
        repl_expr: &Value,
    ) -> Result<Value, Signal> {
        let re = self.compiled_regex(pattern, option)?;
        let mut out: Vec<u8> = Vec::new();
        let mut last = 0usize;
        for caps in re.captures_iter(&data) {
            let m0 = caps.get(0).expect("group 0 always present");
            out.extend_from_slice(&data[last..m0.start()]);
            self.set_regex_captures(&caps);
            match self.eval(repl_expr)? {
                Value::Str(b) => out.extend_from_slice(&b),
                Value::Nil => {}
                other => out.extend_from_slice(self.repr(&other).as_bytes()),
            }
            last = m0.end();
        }
        out.extend_from_slice(&data[last..]);
        let result = Value::str(out);
        if let Some(p) = place {
            let r = result.clone();
            self.with_place_mut(&p, move |loc| {
                *loc = r;
                Ok(Value::Nil)
            })?;
        }
        Ok(result)
    }

    /// A total order over values, for `sort` (symbols compare by name).
    fn value_cmp(&self, a: &Value, b: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        fn rank(v: &Value) -> u8 {
            match v {
                Value::Nil => 0,
                Value::True => 1,
                Value::Int(_) | Value::Float(_) => 2,
                Value::Str(_) => 3,
                Value::Symbol(_) | Value::Context(_) => 4,
                Value::List(_) => 5,
                _ => 6,
            }
        }
        match (a, b) {
            (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
                let (x, y) = (num(a).unwrap_or(0.0), num(b).unwrap_or(0.0));
                x.partial_cmp(&y).unwrap_or(Ordering::Equal)
            }
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            (Value::Symbol(x), Value::Symbol(y)) => self.sym_name(*x).cmp(&self.sym_name(*y)),
            (Value::List(x), Value::List(y)) => {
                for (p, q) in x.iter().zip(y.iter()) {
                    let c = self.value_cmp(p, q);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                x.len().cmp(&y.len())
            }
            _ => rank(a).cmp(&rank(b)),
        }
    }

    // ---- place resolution (ORO reference model, ADR-0006) ----------------

    /// Resolve a place expression to a rooted path into a symbol's stored value.
    /// Supports: `sym`, `(place idx...)` implicit indexing, `(nth i place)`,
    /// `(first place)`, `(last place)`.
    fn resolve_place(&self, expr: &Value) -> Result<Place, Signal> {
        match expr {
            Value::Symbol(id) => {
                if self.sym_name(*id) == "self" {
                    return self.current_self();
                }
                Ok(Place {
                    root: *id,
                    path: Vec::new(),
                })
            }
            Value::List(items) if !items.is_empty() => {
                if let Value::Symbol(op) = &items[0] {
                    let rest = &items[1..];
                    match self.sym_name(*op).as_str() {
                        "nth" if items.len() == 3 => {
                            let i = self.eval_index(&items[1])?;
                            let mut p = self.resolve_place(&items[2])?;
                            p.path.push(i);
                            return Ok(p);
                        }
                        "first" if items.len() == 2 => {
                            let mut p = self.resolve_place(&items[1])?;
                            p.path.push(0);
                            return Ok(p);
                        }
                        "last" if items.len() == 2 => {
                            let mut p = self.resolve_place(&items[1])?;
                            p.path.push(-1);
                            return Ok(p);
                        }
                        // Reference-returning control forms (qa-ref): the place
                        // flows out of the branch that is taken.
                        "if" => return self.place_if(rest, false),
                        "if-not" => return self.place_if(rest, true),
                        "when" => return self.place_guard(rest, true),
                        "unless" => return self.place_guard(rest, false),
                        "begin" => return self.place_last(rest),
                        "and" => return self.place_and(rest),
                        "or" => return self.place_or(rest),
                        "cond" => return self.place_cond(rest),
                        "case" => return self.place_case(rest),
                        "set" | "setq" | "setf" => return self.place_after_set(rest),
                        "assoc" if items.len() >= 3 => return self.place_assoc(rest),
                        // Destructive ops also return a reference to their
                        // target, so they can be chained: (pop (sort s)).
                        "reverse" if !rest.is_empty() => {
                            self.sf_reverse(rest)?;
                            return self.resolve_place(&rest[0]);
                        }
                        "sort" if !rest.is_empty() => {
                            self.sf_sort(rest)?;
                            return self.resolve_place(&rest[0]);
                        }
                        "rotate" if !rest.is_empty() => {
                            self.sf_rotate(rest)?;
                            return self.resolve_place(&rest[0]);
                        }
                        "replace" if rest.len() >= 3 => {
                            self.sf_replace(rest)?;
                            return self.resolve_place(&rest[1]);
                        }
                        "push" if rest.len() >= 2 => {
                            self.sf_push(rest)?;
                            return self.resolve_place(&rest[1]);
                        }
                        "set-ref" if rest.len() >= 3 => {
                            self.sf_setref(rest, false)?;
                            return self.resolve_place(&rest[1]);
                        }
                        "set-ref-all" if rest.len() >= 3 => {
                            self.sf_setref(rest, true)?;
                            return self.resolve_place(&rest[1]);
                        }
                        "lookup" if rest.len() >= 2 => return self.place_lookup(rest),
                        _ => {}
                    }
                }
                // Implicit indexing place: (container idx idx ...). Only valid
                // when the container is an indexable value (list/array/string);
                // this rules out ordinary function calls like (list 1 2 3).
                let mut p = self.resolve_place(&items[0])?;
                if items.len() > 1 && !self.place_is_indexable(&p)? {
                    return Err(Signal::error("not an indexable place"));
                }
                for idx in &items[1..] {
                    p.path.push(self.eval_index(idx)?);
                }
                Ok(p)
            }
            _ => Err(Signal::error("not a valid place")),
        }
    }

    fn eval_index(&self, expr: &Value) -> Result<i64, Signal> {
        match self.eval(expr)? {
            Value::Int(n) => Ok(n),
            _ => Err(Signal::error("place index must be an integer")),
        }
    }

    fn place_last(&self, forms: &[Value]) -> Result<Place, Signal> {
        // begin: evaluate all but the last, then take the last as the place.
        if forms.is_empty() {
            return Err(Signal::error("place: empty body has no reference"));
        }
        for f in &forms[..forms.len() - 1] {
            self.eval(f)?;
        }
        self.resolve_place(&forms[forms.len() - 1])
    }

    fn place_if(&self, args: &[Value], invert: bool) -> Result<Place, Signal> {
        // (if c then [else]) / (if-not c then [else]) -> place of the taken branch.
        let cond = self.eval(
            args.first()
                .ok_or_else(|| Signal::error("if: no condition"))?,
        )?;
        let taken = if cond.is_truthy() ^ invert {
            args.get(1)
        } else {
            args.get(2)
        };
        match taken {
            Some(e) => self.resolve_place(e),
            None => Err(Signal::error("if: no reference in the untaken branch")),
        }
    }

    fn place_guard(&self, args: &[Value], positive: bool) -> Result<Place, Signal> {
        // (when c body...) / (unless c body...) -> place of the last body form.
        let cond = self.eval(
            args.first()
                .ok_or_else(|| Signal::error("when: no condition"))?,
        )?;
        if cond.is_truthy() == positive {
            self.place_last(&args[1..])
        } else {
            Err(Signal::error("when/unless: guard false, no reference"))
        }
    }

    fn place_and(&self, args: &[Value]) -> Result<Place, Signal> {
        if args.is_empty() {
            return Err(Signal::error("and: no reference"));
        }
        for a in &args[..args.len() - 1] {
            if !self.eval(a)?.is_truthy() {
                return Err(Signal::error("and: short-circuited, no reference"));
            }
        }
        self.resolve_place(&args[args.len() - 1])
    }

    fn place_or(&self, args: &[Value]) -> Result<Place, Signal> {
        for a in args {
            if self.eval(a)?.is_truthy() {
                return self.resolve_place(a);
            }
        }
        Err(Signal::error("or: no truthy reference"))
    }

    fn place_cond(&self, clauses: &[Value]) -> Result<Place, Signal> {
        for clause in clauses {
            if let Value::List(parts) = clause {
                if let Some(test) = parts.first() {
                    if self.eval(test)?.is_truthy() {
                        return self.place_last(&parts[1..]);
                    }
                }
            }
        }
        Err(Signal::error("cond: no clause matched"))
    }

    fn place_case(&self, args: &[Value]) -> Result<Place, Signal> {
        let key = self.eval(args.first().ok_or_else(|| Signal::error("case: no key"))?)?;
        for clause in &args[1..] {
            if let Value::List(parts) = clause {
                if let Some(label) = parts.first() {
                    if self.case_label_matches(label, &key) {
                        return self.place_last(&parts[1..]);
                    }
                }
            }
        }
        Err(Signal::error("case: no clause matched"))
    }

    fn place_after_set(&self, args: &[Value]) -> Result<Place, Signal> {
        // (setq place val ...) / (set 'sym val ...) evaluated for its effect;
        // the reference is the last assigned place.
        self.sf_setf(args)?;
        self.resolve_place(
            args.first()
                .ok_or_else(|| Signal::error("set: no target"))?,
        )
    }

    fn sf_setref(&self, args: &[Value], all: bool) -> Result<Value, Signal> {
        // (set-ref key place new) — deep-replace key with new in the place.
        let key = self.eval(&args[0])?;
        let new = self.eval(&args[2])?;
        self.place_or_value(&args[1], |loc| {
            deep_replace(loc, &key, &new, all);
        })
    }

    fn sf_find_all(&self, args: &[Value]) -> Result<Value, Signal> {
        // (find-all pattern data [exp [option|compare]]) — collect every match.
        // `exp` (deferred) transforms each hit with `$it`/`$0..$N` bound; without
        // it, the hit itself (the match string / the element) is collected. Sets
        // `$count`. Three forms (ADR-0036): regex over a string, a list-pattern
        // over a list, or a key over a list.
        if args.len() < 2 {
            return Err(Signal::error(
                "find-all: expected (find-all pattern data [exp])",
            ));
        }
        let pattern = self.eval(&args[0])?;
        let data = self.eval(&args[1])?;
        let exp = args.get(2);
        let it_sym = self.intern("$it");
        let mut out = Vec::new();

        // Evaluate `exp` (or fall back to `hit`) with `$it` bound to `hit`.
        let collect = |this: &Self, hit: Value, out: &mut Vec<Value>| -> Result<(), Signal> {
            let v = match exp {
                Some(e) => {
                    let mut scope = Scope::new(this);
                    scope.bind(it_sym, hit);
                    this.eval(e)?
                }
                None => hit,
            };
            out.push(v);
            Ok(())
        };

        match (&pattern, &data) {
            // Regex over text.
            (Value::Str(pat), Value::Str(text)) => {
                #[cfg(feature = "regex")]
                {
                    let opt = match args.get(3) {
                        Some(e) => match self.eval(e)? {
                            Value::Int(n) => n,
                            _ => 0,
                        },
                        None => 0,
                    };
                    let pat = String::from_utf8_lossy(pat).into_owned();
                    let re = self.compiled_regex(&pat, opt)?;
                    // Collect match spans first so the borrow on `text` ends
                    // before evaluating `exp`.
                    let hits: Vec<(Vec<u8>, regex::bytes::Captures)> = re
                        .captures_iter(text)
                        .map(|c| (c.get(0).unwrap().as_bytes().to_vec(), c))
                        .collect();
                    for (whole, caps) in hits {
                        self.set_regex_captures(&caps);
                        collect(self, Value::str(whole), &mut out)?;
                    }
                }
                #[cfg(not(feature = "regex"))]
                {
                    let _ = (pat, text);
                    return Err(Signal::error("find-all: regex support is not built in"));
                }
            }
            // Pattern or key over a list.
            (_, Value::List(items)) => {
                let cmp = match args.get(3) {
                    Some(e) => Some(self.eval(e)?),
                    None => None,
                };
                for el in items.iter() {
                    let hit = if matches!(pattern, Value::List(_)) {
                        crate::builtins::pattern_matches(self, &pattern, el)
                    } else if let Some(f) = &cmp {
                        self.call(f, vec![pattern.clone(), el.clone()])?.is_truthy()
                    } else {
                        crate::builtins::values_equal(&pattern, el)
                    };
                    if hit {
                        collect(self, el.clone(), &mut out)?;
                    }
                }
            }
            _ => {
                return Err(Signal::error(
                    "find-all: expected a string or a list to search",
                ))
            }
        }

        self.set_global(self.intern("$count"), Value::Int(out.len() as i64));
        Ok(Value::list(out))
    }

    fn sf_pop_assoc(&self, args: &[Value]) -> Result<Value, Signal> {
        // (pop-assoc key assoc-list) — remove and return the (key …) pair from
        // an association list place (ADR-0036).
        if args.len() < 2 {
            return Err(Signal::error(
                "pop-assoc: expected (pop-assoc key assoc-list)",
            ));
        }
        let key = self.eval(&args[0])?;
        let place = self.resolve_place(&args[1])?;
        self.with_place_mut(&place, move |loc| {
            let list = match loc {
                Value::List(l) => Rc::make_mut(l),
                _ => return Ok(Value::Nil),
            };
            let found = list.iter().position(|item| {
                matches!(item, Value::List(pair)
                    if pair.first().is_some_and(|k| crate::builtins::values_equal(k, &key)))
            });
            match found {
                Some(idx) => Ok(list.remove(idx)),
                None => Ok(Value::Nil),
            }
        })
    }

    fn place_lookup(&self, args: &[Value]) -> Result<Place, Signal> {
        // (lookup key assoc-list [index]) -> reference to the matched element.
        let key = self.eval(&args[0])?;
        let mut place = self.resolve_place(&args[1])?;
        let idx = match args.get(2) {
            Some(e) => self.eval_index(e)?,
            None => -1,
        };
        if let Value::List(items) = self.read_place(&place)? {
            for (i, item) in items.iter().enumerate() {
                if let Value::List(pair) = item {
                    if pair
                        .first()
                        .is_some_and(|k| crate::builtins::values_equal(k, &key))
                    {
                        place.path.push(i as i64);
                        place.path.push(idx);
                        return Ok(place);
                    }
                }
            }
        }
        Err(Signal::error("lookup: key not found"))
    }

    fn place_assoc(&self, args: &[Value]) -> Result<Place, Signal> {
        // (assoc key place) -> reference to the matching (key ...) sub-list.
        let key = self.eval(&args[0])?;
        let place = self.resolve_place(&args[1])?;
        let list = self.read_place(&place)?;
        if let Value::List(items) = list {
            for (i, item) in items.iter().enumerate() {
                if let Value::List(pair) = item {
                    if pair
                        .first()
                        .is_some_and(|k| crate::builtins::values_equal(k, &key))
                    {
                        let mut p = place;
                        p.path.push(i as i64);
                        return Ok(p);
                    }
                }
            }
        }
        Err(Signal::error("assoc: key not found"))
    }

    /// Mutate the value at a place, creating the root slot if absent.
    fn with_place_mut<R>(
        &self,
        place: &Place,
        f: impl FnOnce(&mut Value) -> Result<R, Signal>,
    ) -> Result<R, Signal> {
        self.check_writable(place.root)?;
        let mut g = self.globals.borrow_mut();
        let root = g.entry(place.root).or_insert(Value::Nil);
        let loc = place_navigate(root, &place.path)
            .ok_or_else(|| Signal::error("place index out of range"))?;
        f(loc)
    }

    fn sf_define(&self, args: &[Value]) -> Result<Value, Signal> {
        match args.first() {
            // (define (f a b) body...) — function definition sugar.
            Some(Value::List(sig)) => {
                let fname = match sig.first() {
                    Some(Value::Symbol(id)) => *id,
                    _ => return Err(Signal::error("define: malformed function name")),
                };
                let params = self.parse_params(&sig[1..])?;
                let lambda = Value::Lambda(Rc::new(Lambda {
                    params,
                    body: args[1..].to_vec(),
                }));
                self.ensure_context(fname);
                self.set_global(fname, lambda.clone());
                Ok(lambda)
            }
            // (define sym value)
            Some(Value::Symbol(id)) => {
                let val = self.eval(args.get(1).unwrap_or(&Value::Nil))?;
                self.set_global(*id, val.clone());
                Ok(val)
            }
            _ => Err(Signal::error("define: expected a symbol or (name args...)")),
        }
    }

    fn sf_set(&self, args: &[Value], eval_target: bool) -> Result<Value, Signal> {
        // set: (set 'a 1 'b 2 ...) — targets are evaluated to symbols.
        // setq: (setq a 1 b 2 ...) — targets are literal symbols.
        let mut last = Value::Nil;
        let mut i = 0;
        while i + 1 < args.len() {
            let sym = if eval_target {
                match self.eval(&args[i])? {
                    Value::Symbol(id) => id,
                    other => {
                        return Err(Signal::Error(format!(
                            "set: target is not a symbol: {}",
                            self.repr(&other)
                        )))
                    }
                }
            } else {
                match &args[i] {
                    Value::Symbol(id) => *id,
                    other => {
                        return Err(Signal::Error(format!(
                            "setq: target is not a symbol: {}",
                            self.repr(other)
                        )))
                    }
                }
            };
            self.check_writable(sym)?;
            last = self.eval(&args[i + 1])?;
            self.set_global(sym, last.clone());
            i += 2;
        }
        Ok(last)
    }

    fn sf_constant(&self, args: &[Value]) -> Result<Value, Signal> {
        // (constant 'sym val ...) — like set, but marks the symbol read-only.
        let mut last = Value::Nil;
        let mut i = 0;
        while i + 1 < args.len() {
            let sym = match self.eval(&args[i])? {
                Value::Symbol(id) => id,
                other => {
                    return Err(Signal::Error(format!(
                        "constant: target is not a symbol: {}",
                        self.repr(&other)
                    )))
                }
            };
            last = self.eval(&args[i + 1])?;
            self.set_global(sym, last.clone());
            self.protected.borrow_mut().insert(sym);
            i += 2;
        }
        Ok(last)
    }

    fn check_writable(&self, sym: SymId) -> Result<(), Signal> {
        if self.protected.borrow().contains(&sym) {
            // When the protected symbol is the container of the current `self`,
            // newLISP names it "container of (self)".
            let name = if self
                .self_stack
                .borrow()
                .last()
                .is_some_and(|p| p.root == sym)
            {
                "container of (self)".to_string()
            } else {
                self.sym_name(sym)
            };
            Err(Signal::Error(format!(
                "ERR: symbol is protected : MAIN:{}",
                name
            )))
        } else {
            Ok(())
        }
    }

    /// Parse a `let`/`letn`/`letex` binding spec into `(symbol, optional
    /// init-expr)` pairs, supporting newLISP's two syntaxes: the flat form
    /// `(s1 e1 s2 e2 …)` (a lone trailing symbol defaults to `nil`), and the
    /// fully-parenthesized form `((s1 e1) (s2) …)` where each initializer is
    /// optional (`nil` if missing) and a bare symbol is allowed.
    fn let_bindings<'a>(
        &self,
        spec: &'a [Value],
        who: &str,
    ) -> Result<Vec<(SymId, Option<&'a Value>)>, Signal> {
        let as_sym = |v: Option<&Value>| -> Result<SymId, Signal> {
            match v {
                Some(Value::Symbol(id)) => Ok(*id),
                other => Err(Signal::Error(format!(
                    "{}: binding name is not a symbol: {}",
                    who,
                    other.map(|v| self.repr(v)).unwrap_or_else(|| "()".into())
                ))),
            }
        };
        // Parenthesized form when the first element is itself a list.
        if matches!(spec.first(), Some(Value::List(_))) {
            let mut out = Vec::with_capacity(spec.len());
            for b in spec {
                match b {
                    Value::List(pair) => out.push((as_sym(pair.first())?, pair.get(1))),
                    Value::Symbol(id) => out.push((*id, None)),
                    other => {
                        return Err(Signal::Error(format!(
                            "{}: bad binding {}",
                            who,
                            self.repr(other)
                        )))
                    }
                }
            }
            Ok(out)
        } else {
            // Flat form: positional symbol/init pairs.
            let mut out = Vec::with_capacity(spec.len().div_ceil(2));
            let mut i = 0;
            while i < spec.len() {
                out.push((as_sym(spec.get(i))?, spec.get(i + 1)));
                i += 2;
            }
            Ok(out)
        }
    }

    fn sf_let(&self, args: &[Value]) -> Result<Value, Signal> {
        // (let ((s1 e1) …) body…) / (let (s1 e1 …) body…). newLISP `let` is
        // parallel: all inits evaluate in the outer scope before any bind.
        let spec = match args.first() {
            Some(Value::List(b)) => b,
            _ => return Err(Signal::error("let: expected a binding list")),
        };
        let bindings = self.let_bindings(spec, "let")?;
        let mut pending: Vec<(SymId, Value)> = Vec::with_capacity(bindings.len());
        for (sym, init) in bindings {
            let val = match init {
                Some(e) => self.eval(e)?,
                None => Value::Nil,
            };
            pending.push((sym, val));
        }
        let mut scope = Scope::new(self);
        for (sym, val) in pending {
            scope.bind(sym, val);
        }
        self.eval_body(&args[1..])
    }

    fn sf_letn(&self, args: &[Value]) -> Result<Value, Signal> {
        // (letn ((s1 e1) …) body…) — like nested `let`s: each initializer sees
        // the bindings made so far, so `e2` can refer to `s1`.
        let spec = match args.first() {
            Some(Value::List(b)) => b,
            _ => return Err(Signal::error("letn: expected a binding list")),
        };
        let bindings = self.let_bindings(spec, "letn")?;
        let mut scope = Scope::new(self);
        for (sym, init) in bindings {
            let val = match init {
                Some(e) => self.eval(e)?,
                None => Value::Nil,
            };
            scope.bind(sym, val);
        }
        self.eval_body(&args[1..])
    }

    fn sf_letex(&self, args: &[Value]) -> Result<Value, Signal> {
        // (letex ((s1 e1) …) body…) — combine `let` and `expand`: evaluate the
        // initializers (in the outer scope, like `let`), substitute each symbol's
        // value into the body forms, then evaluate the expanded body.
        let spec = match args.first() {
            Some(Value::List(b)) => b,
            _ => return Err(Signal::error("letex: expected a binding list")),
        };
        let bindings = self.let_bindings(spec, "letex")?;
        let mut pending: Vec<(SymId, Value)> = Vec::with_capacity(bindings.len());
        for (sym, init) in bindings {
            let val = match init {
                Some(e) => self.eval(e)?,
                None => Value::Nil,
            };
            pending.push((sym, val));
        }
        let syms: Vec<SymId> = pending.iter().map(|(s, _)| *s).collect();
        let mut scope = Scope::new(self);
        for (sym, val) in pending {
            scope.bind(sym, val);
        }
        // Expand the bound symbols into each body form, then evaluate — the
        // substitution reads the just-bound values via `lookup` (ADR-0027).
        let mut result = Value::Nil;
        for form in &args[1..] {
            let expanded = crate::builtins::expand_symbols(self, form, &syms);
            result = self.eval(&expanded)?;
        }
        Ok(result)
    }

    fn sf_curry(&self, args: &[Value]) -> Result<Value, Signal> {
        // (curry func exp) → (lambda ($x) (func exp $x)). Like a macro, curry
        // does not evaluate its arguments; they are spliced literally into the
        // returned one-argument lambda and evaluated only when it is applied.
        if args.len() != 2 {
            return Err(Signal::error("curry: expected (curry func exp)"));
        }
        let x = self.intern("$x");
        let params = Value::list(vec![Value::Symbol(x)]);
        let body = Value::list(vec![args[0].clone(), args[1].clone(), Value::Symbol(x)]);
        self.sf_lambda(&[params, body], false)
    }

    fn sf_lambda(&self, args: &[Value], is_fexpr: bool) -> Result<Value, Signal> {
        // An empty `(lambda)` self-quotes to the one-element list `(lambda)`, so
        // `(append (lambda) …)` builds a lambda as data (ADR-0027). Anything with
        // a parameter list uses the compact `Value::Lambda`/`Fexpr` form.
        if args.is_empty() {
            let head = self.intern(if is_fexpr { "lambda-macro" } else { "lambda" });
            return Ok(Value::list(vec![Value::Symbol(head)]));
        }
        let params = match args.first() {
            Some(Value::List(p)) => self.parse_params(p)?,
            _ => return Err(Signal::error("lambda: expected a parameter list")),
        };
        let lam = Rc::new(Lambda {
            params,
            body: args[1..].to_vec(),
        });
        Ok(if is_fexpr {
            Value::Fexpr(lam)
        } else {
            Value::Lambda(lam)
        })
    }

    /// If `items` is a lambda-headed list `(lambda …)` / `(fn …)` /
    /// `(lambda-macro …)`, whether it is a fexpr (`lambda-macro`) (ADR-0027).
    fn lambda_head(&self, items: &[Value]) -> Option<bool> {
        match items.first() {
            Some(Value::Symbol(id)) => match self.sym_name(*id).as_str() {
                "lambda" | "fn" => Some(false),
                "lambda-macro" => Some(true),
                _ => None,
            },
            _ => None,
        }
    }

    /// Call a lambda-headed list `(lambda (params…) body…)` built as data
    /// (ADR-0027): parse the parameter list and body on the spot.
    fn call_lambda_list(
        &self,
        items: &[Value],
        is_fexpr: bool,
        arg_exprs: &[Value],
    ) -> Result<Value, Signal> {
        let params = match items.get(1) {
            Some(Value::List(p)) => self.parse_params(p)?,
            Some(Value::Nil) | None => Vec::new(),
            _ => return Err(Signal::error("lambda: parameter list expected")),
        };
        let lam = Lambda {
            params,
            body: items.get(2..).unwrap_or(&[]).to_vec(),
        };
        let args = if is_fexpr {
            arg_exprs.to_vec()
        } else {
            self.eval_args(arg_exprs)?
        };
        self.call_lambda(&lam, args)
    }

    fn sf_self(&self, args: &[Value]) -> Result<Value, Signal> {
        // (self) -> the current object; (self i j ...) -> nested element.
        let mut place = self.current_self()?;
        for a in args {
            place.path.push(self.eval_index(a)?);
        }
        self.read_place(&place)
    }

    fn sf_time(&self, args: &[Value]) -> Result<Value, Signal> {
        // (time expr [count]) -> milliseconds spent evaluating expr `count` times.
        use std::time::Instant;
        let expr = args
            .first()
            .ok_or_else(|| Signal::error("time: missing expression"))?;
        let count = match args.get(1) {
            Some(e) => match self.eval(e)? {
                Value::Int(n) => n.max(1),
                Value::Float(f) => (f as i64).max(1),
                _ => 1,
            },
            None => 1,
        };
        let start = Instant::now();
        for _ in 0..count {
            self.eval(expr)?;
        }
        Ok(Value::Int(start.elapsed().as_millis() as i64))
    }

    fn sf_define_macro(&self, args: &[Value]) -> Result<Value, Signal> {
        // (define-macro (name p1 p2 ...) body...) — a named fexpr (CONTEXT.md: fexpr).
        match args.first() {
            Some(Value::List(sig)) => {
                let fname = match sig.first() {
                    Some(Value::Symbol(id)) => *id,
                    _ => return Err(Signal::error("define-macro: malformed name")),
                };
                let params = self.parse_params(&sig[1..])?;
                let fx = Value::Fexpr(Rc::new(Lambda {
                    params,
                    body: args[1..].to_vec(),
                }));
                self.ensure_context(fname);
                self.set_global(fname, fx.clone());
                Ok(fx)
            }
            _ => Err(Signal::error("define-macro: expected (name args...)")),
        }
    }

    fn sf_catch(&self, args: &[Value]) -> Result<Value, Signal> {
        // (catch expr) or (catch expr 'result-sym)
        let expr = args
            .first()
            .ok_or_else(|| Signal::error("catch: missing expression"))?;
        let result = self.eval(expr);
        match args.get(1) {
            // Two-arg form: bind result-or-thrown into the target symbol; return
            // true on normal completion, nil on a caught throw/error (ADR-0011).
            Some(target_expr) => {
                let sym = match self.eval(target_expr)? {
                    Value::Symbol(id) => id,
                    other => {
                        return Err(Signal::Error(format!(
                            "catch: result target is not a symbol: {}",
                            self.repr(&other)
                        )))
                    }
                };
                match result {
                    Ok(v) => {
                        self.set_global(sym, v);
                        Ok(Value::True)
                    }
                    Err(Signal::Throw(v)) => {
                        self.set_global(sym, v);
                        Ok(Value::Nil)
                    }
                    Err(Signal::Error(msg)) => {
                        self.set_global(sym, Value::str(msg.into_bytes()));
                        Ok(Value::Nil)
                    }
                }
            }
            // One-arg form: return the value, or the caught thrown value / error.
            None => match result {
                Ok(v) => Ok(v),
                Err(Signal::Throw(v)) => Ok(v),
                Err(Signal::Error(msg)) => Ok(Value::str(msg.into_bytes())),
            },
        }
    }

    fn sf_throw(&self, args: &[Value]) -> Result<Value, Signal> {
        let v = self.eval(args.first().unwrap_or(&Value::Nil))?;
        Err(Signal::Throw(v))
    }

    fn parse_params(&self, forms: &[Value]) -> Result<Vec<Param>, Signal> {
        let mut params = Vec::with_capacity(forms.len());
        for f in forms {
            match f {
                Value::Symbol(id) => params.push(Param {
                    sym: *id,
                    default: None,
                }),
                // Default-valued parameter: (sym default-expr)
                Value::List(l) => match l.first() {
                    Some(Value::Symbol(id)) => params.push(Param {
                        sym: *id,
                        default: l.get(1).cloned(),
                    }),
                    _ => return Err(Signal::error("parameter: malformed default binding")),
                },
                other => {
                    return Err(Signal::Error(format!(
                        "parameter is not a symbol: {}",
                        self.repr(other)
                    )))
                }
            }
        }
        Ok(params)
    }
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

/// A location: a root symbol slot plus a path of list indices into it.
#[derive(Clone)]
struct Place {
    root: SymId,
    path: Vec<i64>,
}

/// The class (context) symbol of a FOOP object: the head of its list.
fn class_of(v: &Value) -> Option<SymId> {
    match v {
        Value::List(l) => match l.first() {
            Some(Value::Symbol(s)) | Some(Value::Context(s)) => Some(*s),
            _ => None,
        },
        _ => None,
    }
}

/// Navigate a mutable value along a path of (possibly negative) list indices.
fn place_navigate<'a>(root: &'a mut Value, path: &[i64]) -> Option<&'a mut Value> {
    let mut cur = root;
    for &raw in path {
        match cur {
            // A list or array navigates the same way; setf replaces the element
            // in place, which respects an array's fixed length (ADR-0023).
            Value::List(l) | Value::Array(l) => {
                let i = if raw < 0 { l.len() as i64 + raw } else { raw };
                if i < 0 || i as usize >= l.len() {
                    return None;
                }
                // make_mut clones the shared container so the write stays isolated
                // (copy-on-write, ADR-0024).
                cur = &mut Rc::make_mut(l)[i as usize];
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// Rotate a list/string in place: positive `n` moves the tail to the front.
fn rotate_value(v: &mut Value, n: i64) {
    match v {
        Value::List(l) if !l.is_empty() => {
            let len = l.len() as i64;
            let k = (((n % len) + len) % len) as usize;
            Rc::make_mut(l).rotate_right(k);
        }
        Value::Str(b) if !b.is_empty() => {
            let len = b.len() as i64;
            let k = (((n % len) + len) % len) as usize;
            Rc::make_mut(b).rotate_right(k);
        }
        _ => {}
    }
}

/// Replace occurrences of `target` with `newval` inside `loc`, in place.
fn do_replace(loc: &mut Value, target: &Value, newval: &Value) {
    match loc {
        Value::List(l) => {
            for e in Rc::make_mut(l).iter_mut() {
                if crate::builtins::values_equal(e, target) {
                    *e = newval.clone();
                }
            }
        }
        Value::Str(b) => {
            if let (Value::Str(t), Value::Str(n)) = (target, newval) {
                *b = Rc::new(replace_bytes(b, t, n));
            }
        }
        _ => {}
    }
}

/// Deep-replace occurrences of `key` with `new` within nested lists. With
/// `all == false`, stops after the first replacement. Returns whether it did.
fn deep_replace(v: &mut Value, key: &Value, new: &Value, all: bool) -> bool {
    if let Value::List(l) = v {
        for e in Rc::make_mut(l).iter_mut() {
            if crate::builtins::values_equal(e, key) {
                *e = new.clone();
                if !all {
                    return true;
                }
            } else if deep_replace(e, key, new, all) && !all {
                return true;
            }
        }
    }
    false
}

/// Replace all non-overlapping occurrences of `needle` with `to` in `hay`.
fn replace_bytes(hay: &[u8], needle: &[u8], to: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return hay.to_vec();
    }
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0;
    while i < hay.len() {
        if hay[i..].starts_with(needle) {
            out.extend_from_slice(to);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

fn is_int(v: &Value) -> bool {
    matches!(v, Value::Int(_))
}

fn num(v: &Value) -> Result<f64, Signal> {
    match v {
        Value::Int(n) => Ok(*n as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(Signal::error("expected a number")),
    }
}

/// Element access with newLISP-style negative indexing; out of range -> nil.
fn index_one(i: i64, target: &Value) -> Value {
    match target {
        // A list or array indexes the same way (ADR-0023).
        Value::List(l) | Value::Array(l) => {
            let idx = if i < 0 { l.len() as i64 + i } else { i };
            if idx < 0 || idx as usize >= l.len() {
                Value::Nil
            } else {
                l[idx as usize].clone()
            }
        }
        // Implicit indexing of a string is character-based (ADR-0025); the
        // byte-based path is the implicit slice `(i str)`, in `slice_seq`.
        Value::Str(b) => match crate::utf8::char_byte_range(b, i) {
            Some((s, e)) => Value::str(b[s..e].to_vec()),
            None => Value::Nil,
        },
        _ => Value::Nil,
    }
}

/// Implicit rest/slice: the sub-sequence of `target` from offset `start`
/// (negative from the end); `len` omitted runs to the end, a negative `len`
/// counts from the end. Slicing an array yields a list (a transform, ADR-0023).
fn slice_seq(start: i64, len: Option<i64>, target: &Value) -> Value {
    fn bounds(n: usize, start: i64, len: Option<i64>) -> (usize, usize) {
        let n = n as i64;
        let s = if start < 0 {
            (n + start).max(0)
        } else {
            start.min(n)
        };
        let e = match len {
            None => n,
            Some(l) if l >= 0 => (s + l).min(n),
            Some(l) => n + l, // negative: counted from the end
        };
        (s as usize, e.clamp(s, n) as usize)
    }
    match target {
        Value::List(l) | Value::Array(l) => {
            let (s, e) = bounds(l.len(), start, len);
            Value::list(l[s..e].to_vec())
        }
        Value::Str(b) => {
            let (s, e) = bounds(b.len(), start, len);
            Value::str(b[s..e].to_vec())
        }
        _ => Value::Nil,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Reader;

    /// Read and evaluate `src`, returning the value of the last form.
    fn run(src: &str) -> Value {
        let interp = Interp::new();
        let prims = interp.primitive_names();
        let forms = {
            let mut it = interp.interner.borrow_mut();
            Reader::new(src.as_bytes(), &mut it, &prims)
                .read_all()
                .unwrap()
        };
        let mut last = Value::Nil;
        for f in &forms {
            last = match interp.eval(f) {
                Ok(v) => v,
                Err(Signal::Error(msg)) => panic!("evaluation error: {}", msg),
                Err(Signal::Throw(_)) => panic!("uncaught throw"),
            };
        }
        last
    }

    fn as_int(v: Value) -> i64 {
        match v {
            Value::Int(n) => n,
            _ => panic!("expected Int, got a different value type"),
        }
    }

    fn as_float(v: Value) -> f64 {
        match v {
            Value::Float(f) => f,
            _ => panic!("expected Float, got a different value type"),
        }
    }

    fn as_str(v: Value) -> String {
        match v {
            Value::Str(b) => String::from_utf8_lossy(&b).into_owned(),
            _ => panic!("expected Str, got a different value type"),
        }
    }

    #[test]
    fn integer_arithmetic_wraps() {
        assert_eq!(as_int(run("(+ 1 2 3)")), 6);
        assert_eq!(as_int(run("(- 10 3 2)")), 5);
        assert_eq!(as_int(run("(* 2 3 4)")), 24);
        assert_eq!(as_int(run("(/ 20 2 5)")), 2);
        // Wrapping at the 64-bit boundary (ADR-0012), not a panic.
        assert_eq!(as_int(run("(+ 9223372036854775807 1)")), i64::MIN);
    }

    #[test]
    fn float_arithmetic_is_separate() {
        assert!((as_float(run("(add 3 4)")) - 7.0).abs() < 1e-9);
        assert!((as_float(run("(mul 2.5 4)")) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn define_and_call() {
        assert_eq!(as_int(run("(define (sq x) (* x x)) (sq 9)")), 81);
    }

    #[test]
    fn recursion() {
        let prog = "(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 10)";
        assert_eq!(as_int(run(prog)), 55);
    }

    #[test]
    fn let_is_flat_and_parallel() {
        assert!((as_float(run("(let (a 3 b 4) (add (mul a a) (mul b b)))")) - 25.0).abs() < 1e-9);
    }

    #[test]
    fn dynamic_scope_restores_after_call() {
        // `x` is global; `f` rebinds it as a parameter, then it must restore.
        let prog = "(set 'x 100) (define (f x) x) (f 7) x";
        assert_eq!(as_int(run(prog)), 100);
    }

    #[test]
    fn catch_and_throw() {
        assert_eq!(as_int(run("(catch (throw 42))")), 42);
        // Two-arg catch binds the thrown value and returns nil.
        let prog = "(catch (throw 9) 'r) r";
        assert_eq!(as_int(run(prog)), 9);
    }

    #[test]
    fn list_operations() {
        assert_eq!(as_int(run("(length (list 1 2 3 4))")), 4);
        assert_eq!(as_int(run("(first (cons 5 (list 6 7)))")), 5);
        assert_eq!(as_int(run("(nth 1 (list 10 20 30))")), 20);
    }

    #[test]
    fn incr_and_dotimes() {
        assert_eq!(as_int(run("(set 'c 0) (dotimes (i 5) (++ c)) c")), 5);
        assert_eq!(as_int(run("(set 'n 10) (-- n 3) n")), 7);
    }

    #[test]
    fn when_and_unless() {
        assert_eq!(as_int(run("(when (< 1 2) 41 42)")), 42);
        assert!(matches!(run("(when nil 1)"), Value::Nil));
        assert_eq!(as_int(run("(unless nil 99)")), 99);
    }

    #[test]
    fn bitwise() {
        assert_eq!(as_int(run("(& 6 3)")), 2);
        assert_eq!(as_int(run("(| 4 1)")), 5);
        assert_eq!(as_int(run("(<< 1 4)")), 16);
    }

    #[test]
    fn format_subset() {
        assert_eq!(as_str(run("(format \"%d-%d\" 3 7)")), "3-7");
        assert_eq!(as_str(run("(format \"%05.2f\" 3.14159)")), "03.14");
    }

    #[test]
    fn for_loop_inclusive_and_directional() {
        assert_eq!(
            as_int(run("(set 's 0) (for (i 1 5) (set 's (+ s i))) s")),
            15
        );
        // Descending when from > to.
        assert_eq!(
            as_int(run("(set 's 0) (for (i 5 1) (set 's (+ s 1))) s")),
            5
        );
    }

    #[test]
    fn dolist_iterates() {
        assert_eq!(
            as_int(run(
                "(set 's 0) (dolist (x (list 2 3 4)) (set 's (+ s x))) s"
            )),
            9
        );
    }

    #[test]
    fn push_and_pop_symbol_place() {
        // push at front and at end (-1), then pop from front.
        assert_eq!(as_int(run("(set 'l (list 2 3)) (push 1 l) (first l)")), 1);
        assert_eq!(as_int(run("(set 'l (list 1 2)) (push 9 l -1) (last l)")), 9);
        assert_eq!(as_int(run("(set 'l (list 7 8 9)) (pop l)")), 7);
    }

    #[test]
    fn higher_order() {
        assert_eq!(
            as_int(run("(apply + (map (lambda (x) (* x x)) (sequence 1 4)))")),
            30
        );
        assert_eq!(
            as_int(run(
                "(length (filter (lambda (x) (< x 3)) (list 1 2 3 4 5)))"
            )),
            2
        );
    }

    #[test]
    fn implicit_index_and_slice() {
        // A number in functor position is rest/slice, not element access
        // (element access is `(seq i)`): `(2 lst)` is the tail from offset 2.
        assert!(is_true("(= (2 (list 10 20 30 40)) '(30 40))"));
        // `(offset len seq)` — first two elements; a negative len counts back.
        assert!(is_true("(= (0 2 (list 5 6 7 8)) '(5 6))"));
        assert!(is_true("(= (2 -1 (list 10 20 30 40 50)) '(30 40))"));
        // Element access via a list in functor position still works.
        assert_eq!(as_int(run("((list 10 20 30 40) 2)")), 30);
    }

    #[test]
    fn nan_comparisons_are_nil_not_errors() {
        assert!(matches!(run("(< 1.0 (sqrt -1))"), Value::Nil));
        assert!(matches!(run("(= (sqrt -1) (sqrt -1))"), Value::Nil));
        assert!(matches!(run("(NaN? (mod 10 0))"), Value::True));
        // Saturating int cast: inf assumed as max int.
        assert_eq!(as_int(run("(* 1 (div 1.0 0))")), i64::MAX);
    }

    #[test]
    fn char_roundtrip() {
        assert_eq!(as_str(run("(char 65)")), "A");
        assert_eq!(as_int(run("(char \"A\")")), 65);
        // 2-byte UTF-8 code point.
        assert_eq!(as_int(run("(length (char 956))")), 2);
    }

    #[test]
    fn setf_into_places() {
        assert_eq!(
            as_int(run("(set 'L (list 1 2 3)) (setf (L 1) 99) (nth 1 L)")),
            99
        );
        assert_eq!(
            as_int(run("(set 'L (list 1 2 3)) (setf (nth 0 L) 7) (first L)")),
            7
        );
        // Nested list place.
        let prog = "(set 'M (list (list 1 2) (list 3 4))) (setf (M 1 0) 88) (nth 0 (nth 1 M))";
        assert_eq!(as_int(run(prog)), 88);
    }

    #[test]
    fn inc_push_pop_into_nested_places() {
        assert_eq!(
            as_int(run("(set 'L (list 10 20)) (inc (L 1) 5) (nth 1 L)")),
            25
        );
        assert_eq!(
            as_int(run(
                "(set 'M (list (list 1) (list 2))) (push 9 (M 0) -1) (length (nth 0 M))"
            )),
            2
        );
        assert_eq!(as_int(run("(set 'M (list (list 7 8))) (pop (M 0))")), 7);
    }

    #[test]
    fn object_write_back_through_symbol_place() {
        // The qa-foop `(inc (self 1))` pattern, with an explicit symbol root:
        // destructive ops on a place write back into the stored object.
        let prog = "(set 'obj (list 0 (list 0))) \
                    (inc (obj 0)) \
                    (inc (obj 1 0)) \
                    (+ (nth 0 obj) (nth 0 (nth 1 obj)))";
        assert_eq!(as_int(run(prog)), 2);
    }

    #[test]
    fn foop_construct_dispatch_and_self_writeback() {
        // Nested self must write back into the stored object (ADR-0010).
        let prog = "(new Class 'A) (new Class 'B) \
                    (setq a (A 0 (B 0))) \
                    (define (A:m x) (inc (self 1))) \
                    (define (B:m x) (inc (self 1))) \
                    (define (A:go) (:m (self) (:m (self 2) \"x\")) (self)) \
                    (= (:go a) '(A 1 (B 1)))";
        assert!(matches!(run(prog), Value::True));
    }

    #[test]
    fn dictionary_hash_access() {
        // qa-dictionary logic at small scale: get/set, bulk-load, enumerate,
        // and delete-by-nil, over a nil-default-functor context (ADR-0030).
        assert!(is_true(
            "(context 'D) (context MAIN) \
             (D \"a\" 1) (D \"b\" 2) \
             (D (list (list \"c\" 3) (list \"d\" 4))) \
             (and (= (D \"a\") 1) (= (D \"c\") 3) (= (D \"z\") nil) \
                  (= (length (D)) 4) \
                  (begin (D \"a\" nil) (and (= (D \"a\") nil) (= (length (D)) 3))))"
        ));
    }

    #[test]
    fn dictionary_enumeration_is_sorted() {
        // `(D)` returns pairs sorted by key — the determinism `save` will need.
        assert_eq!(
            as_str(run(
                "(context 'D) (context MAIN) (D \"b\" 2) (D \"a\" 1) (D \"c\" 3) \
                 (join (map (fn (p) (p 0)) (D)) \",\")"
            )),
            "a,b,c"
        );
    }

    #[test]
    fn foop_construction_survives_dict_rewrite() {
        // A FOOP class (non-nil functor via `new Class`) still builds a tagged
        // object list, distinct from Dictionary access (ADR-0030).
        assert!(is_true("(new Class 'P) (= (P 3 4) '(P 3 4))"));
    }

    #[test]
    fn default_parameters() {
        assert_eq!(as_int(run("(define (mk (r 0) (i 9)) (+ r i)) (mk 5)")), 14);
    }

    #[test]
    fn constant_is_protected() {
        // Writing a constant errors and leaves the value unchanged.
        let prog = "(constant 'k 7) (catch (set 'k 9) 'e) k";
        assert_eq!(as_int(run(prog)), 7);
    }

    fn is_true(src: &str) -> bool {
        matches!(run(src), Value::True)
    }

    #[test]
    fn case_and_if_not_in_value_and_place_position() {
        assert_eq!(
            as_str(run("(case 1 (1 \"one\") (2 \"two\") (true \"other\"))")),
            "one"
        );
        assert_eq!(
            as_str(run("(case 5 (1 \"one\") (true \"other\"))")),
            "other"
        );
        assert!(matches!(run("(case 9 (1 \"one\"))"), Value::Nil));
        assert_eq!(as_int(run("(if-not nil 1 2)")), 1);
        assert_eq!(as_int(run("(if-not true 1 2)")), 2);
        assert_eq!(as_int(run("(if-not nil 7)")), 7);
        // Still usable as reference-returning place forms.
        assert!(is_true(
            "(set 'L '(a b c)) (pop (case 1 (1 L))) (= L '(b c))"
        ));
        assert!(is_true(
            "(set 'l '((a b) c)) (pop (if-not nil (l 0))) (= l '((b) c))"
        ));
    }

    #[test]
    fn reference_returning_control_forms() {
        // A place flows out of the taken branch, so destructive ops reach it.
        assert!(is_true(
            "(set 'l '((a b) c)) (pop (if true (l 0) '(x))) (= l '((b) c))"
        ));
        assert!(is_true(
            "(set 'L '(a b c)) (pop (case 1 (1 L))) (= L '(b c))"
        ));
        assert!(is_true(
            "(set 'L '(a b c)) (pop (cond (nil 1) (true L))) (= L '(b c))"
        ));
    }

    #[test]
    fn place_aware_setq_and_it() {
        assert!(is_true(
            "(set 'l '(a b c)) (setq (first l) 99) (= l '(99 b c))"
        ));
        assert!(is_true(
            "(set 'l '((a 1) (b 2))) (setq (assoc 'b l) '(b 3)) (= l '((a 1) (b 3)))"
        ));
        assert_eq!(as_int(run("(setf y 1) (setf y (+ $it 1))")), 2);
    }

    #[test]
    fn destructive_ops_write_back_through_references() {
        assert!(is_true(
            "(set 'L '(a b (c d e f g))) (replace 'f (nth 2 L) 'z) (= L '(a b (c d e z g)))"
        ));
        assert!(is_true(
            "(set 'r '(A B D E F G)) (replace 'D (rotate r) 'Z) (= r '(G A B Z E F))"
        ));
        assert!(is_true(
            "(set 's '(K U Q A J P T)) (pop (sort s)) (= s '(J K P Q T U))"
        ));
        assert!(is_true(
            "(set 'l '(A B C D E F)) (push 'D (reverse l)) (= l '(D F E D C B A))"
        ));
    }

    #[test]
    fn set_ref_and_lookup_references() {
        assert!(is_true(
            "(set 'l '(\"AA\" (\"BB\" \"CC\"))) (pop (set-ref \"BB\" l \"aa\")) (= l '((\"aa\" \"CC\")))"
        ));
        assert!(is_true(
            "(set 'L '((a 1) (b 1 (2 3) 4) (c 3))) (push 99 (lookup 'b L -2)) \
             (= L '((a 1) (b 1 (99 2 3) 4) (c 3)))"
        ));
    }

    #[test]
    fn strings_count_bytes() {
        // "abc" is 3 bytes; a 2-byte UTF-8 char makes the byte length 4 (ADR-0013).
        assert_eq!(as_int(run("(length \"abc\")")), 3);
        assert_eq!(as_int(run("(length \"a\\195\\169\")")), 3);
    }

    #[test]
    fn string_case_and_trim() {
        assert_eq!(as_str(run("(upper-case \"aB9z\")")), "AB9Z");
        assert_eq!(as_str(run("(lower-case \"aB9z\")")), "ab9z");
        assert_eq!(as_str(run("(trim \"  hi  \")")), "hi");
        assert_eq!(as_str(run("(trim \"--hi--\" \"-\")")), "hi");
        assert_eq!(as_str(run("(trim \"xxhiyy\" \"x\" \"y\")")), "hi");
        // Trimming everything yields the empty string, not an underflow.
        assert_eq!(as_str(run("(trim \"    \")")), "");
    }

    #[test]
    fn slice_strings_and_lists() {
        assert_eq!(as_str(run("(slice \"hello world\" 6)")), "world");
        assert_eq!(as_str(run("(slice \"hello world\" 0 5)")), "hello");
        assert_eq!(as_str(run("(slice \"hello world\" -5)")), "world");
        // A negative length drops that many bytes from the end.
        assert_eq!(as_str(run("(slice \"hello world\" 0 -6)")), "hello");
        // Out-of-range bounds clamp rather than panic.
        assert_eq!(as_str(run("(slice \"hi\" 5)")), "");
        assert!(is_true("(= (slice '(1 2 3 4 5) 1 3) '(2 3 4))"));
    }

    #[test]
    fn find_in_strings_and_lists() {
        assert_eq!(as_int(run("(find \"wor\" \"hello world\")")), 6);
        assert!(matches!(run("(find \"z\" \"hello\")"), Value::Nil));
        assert_eq!(as_int(run("(find 3 '(1 2 3 4))")), 2);
        assert!(matches!(run("(find 9 '(1 2 3))"), Value::Nil));
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn bigint_literals_and_type() {
        assert!(is_true("(integer? 12L)"));
        assert!(is_true("(number? 12L)"));
        assert!(is_true("(not (float? 12L))"));
        assert!(is_true("(atom? 12L)"));
        // An over-long decimal literal (no L) is a bigint, not a float.
        assert!(is_true(
            "(integer? 1234567890123456789012345678901234567890)"
        ));
        // Output is plain decimal — the `L` is lexical only.
        assert_eq!(as_str(run("(string 12L)")), "12");
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn bigint_arithmetic_and_promotion() {
        // A large * then / round-trips exactly.
        assert!(is_true(
            "(set 'n 1234567890123456789012345678901234567890) (= (/ (* n n) n) n)"
        ));
        // int + bigint -> bigint; a fitting result stays bigint-equal to the int.
        assert!(is_true(
            "(= (+ 100000000000000000000L 1) 100000000000000000001L)"
        ));
        assert!(is_true("(= (/ 1234567891L 1234567890L) 1)"));
        // Division truncates toward zero; remainder takes the dividend's sign.
        assert!(is_true(
            "(= (/ -100000000000000000007L 13) -7692307692307692308)"
        ));
        assert!(is_true("(= (% -100000000000000000007L 13) -3)"));
        // `+` truncates a float argument (then bigint present -> bigint result).
        assert!(is_true(
            "(= (+ 2.9 100000000000000000000L) 100000000000000000002L)"
        ));
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn bigint_compare_convert_length_gcd() {
        assert!(is_true("(= 1L 1)"));
        assert!(is_true("(> 100000000000000000000L 999)"));
        assert!(is_true("(< 5 10000000000000000000000L)"));
        assert!(is_true("(zero? 0L)"));
        assert!(is_true("(not (zero? 1L))"));
        assert_eq!(as_int(run("(length 1234567890123456789012345)")), 25);
        assert_eq!(as_str(run("(string (bigint 3.99))")), "3");
        assert_eq!(
            as_str(run("(string (bigint \"123456789012345678901234567890L\"))")),
            "123456789012345678901234567890"
        );
        assert_eq!(as_int(run("(int 42L)")), 42);
        assert_eq!(as_int(run("(gcd 48 36)")), 12);
        assert!(is_true("(= (gcd 100000000000000000000L 250) 250)"));
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn bigint_incr_decr() {
        assert!(is_true(
            "(set 'x 100000000000000000000L) (++ x 5) (= x 100000000000000000005L)"
        ));
        assert!(is_true("(set 'x 5L) (-- x 2L) (= x 3L)"));
    }

    #[test]
    fn lambda_as_list_and_church_numerals() {
        // The gist's core (gist.github.com/kosh04/262332): a LAMBDA macro that
        // builds lambdas as data with `append`/`expand`/`args` (ADR-0027).
        let prelude = "(define-macro (LAMBDA) (append (lambda) (expand (args)))) \
             (define DEFINE define) \
             (DEFINE ZERO  (LAMBDA (F) (LAMBDA (X) X))) \
             (DEFINE ONE   (LAMBDA (F) (LAMBDA (X) (F X)))) \
             (DEFINE TWO   (LAMBDA (F) (LAMBDA (X) (F (F X))))) \
             (DEFINE THREE (LAMBDA (F) (LAMBDA (X) (F (F (F X)))))) \
             (DEFINE (PLUS M N) (LAMBDA (F) (LAMBDA (X) ((M F) ((N F) X))))) \
             (DEFINE (MULT M N) (LAMBDA (F) (LAMBDA (X) ((N (M F)) X)))) \
             (define (to-number x) ((x (lambda (n) (+ n 1))) 0)) ";
        assert_eq!(as_int(run(&format!("{prelude} (to-number ZERO)"))), 0);
        assert_eq!(as_int(run(&format!("{prelude} (to-number ONE)"))), 1);
        assert_eq!(as_int(run(&format!("{prelude} (to-number THREE)"))), 3);
        assert_eq!(
            as_int(run(&format!("{prelude} (to-number (PLUS ONE TWO))"))),
            3
        );
        assert_eq!(
            as_int(run(&format!("{prelude} (to-number (MULT TWO THREE))"))),
            6
        );
    }

    #[test]
    fn lambda_list_building_blocks() {
        // An empty (lambda) is the one-element list `(lambda)`.
        assert!(is_true("(= (lambda) '(lambda))"));
        // append builds a callable lambda from data.
        assert_eq!(
            as_int(run("(set 'f (append (lambda) '((x) (+ x 1)))) (f 41)")),
            42
        );
        // (args) is the unbound argument tail of the current fexpr.
        assert!(is_true("(define-macro (m) (args)) (= (m a b c) '(a b c))"));
        // expand substitutes explicit symbols; upper-case auto-expand takes code.
        assert!(is_true(
            "(set 'A '(+ 1 2)) (= (expand '(x A) 'A) '(x (+ 1 2)))"
        ));
        // A special form can be aliased as a first-class value.
        assert_eq!(as_int(run("(define IF if) (IF (= 1 1) 10 20)")), 10);
    }

    #[test]
    fn contexts_as_namespaces() {
        // (context 'L) makes `set` create L:sym; MAIN restore leaves x in MAIN.
        assert!(is_true(
            "(context 'L) (set 'greeting \"hi\") (context MAIN) \
             (and (= L:greeting \"hi\") (symbol? 'L:greeting))"
        ));
        // dotree iterates the context's symbols; term strips the prefix.
        assert!(is_true(
            "(context 'D) (set 'a 1) (set 'b 2) (context MAIN) \
             (set 'sum 0) (dotree (s D) (set 'sum (+ sum (eval s)))) (= sum 3)"
        ));
        assert!(is_true("(= (term 'Foo:bar) 'bar)"));
        assert!(is_true("(= (term 'plain) 'plain)"));
        // A builtin used inside a context stays the MAIN primitive.
        assert_eq!(
            as_int(run("(context 'E) (set 'n (+ 2 3)) (context MAIN) E:n")),
            5
        );
    }

    #[cfg(feature = "regex")]
    #[test]
    fn regex_and_unicode_case() {
        // regex returns (match byte-offset byte-length); offsets are bytes.
        assert!(is_true("(= (regex \"Ω\" \"ΦabcΩdef\") '(\"Ω\" 5 2))"));
        assert!(is_true(
            "(= (regex \"[0-9]+\" \"abc123def\") '(\"123\" 3 3))"
        ));
        assert!(matches!(run("(regex \"zzz\" \"abc\")"), Value::Nil));
        // A capture group appends its own (str off len).
        assert!(is_true(
            "(= (regex \"a(b+)c\" \"xabbbcy\") '(\"abbbc\" 1 5 \"bbb\" 2 3))"
        ));
        // regex-comp returns the pattern on success; a bad pattern errors.
        assert_eq!(as_str(run("(regex-comp \"a+\")")), "a+");
        assert!(matches!(run("(catch (regex-comp \"a(\") 'e)"), Value::Nil));
        // Unicode case folding (Cyrillic); ASCII unchanged.
        assert_eq!(as_str(run("(upper-case \"абв\")")), "АБВ");
        assert_eq!(as_str(run("(lower-case (upper-case \"абв\"))")), "абв");
        assert_eq!(as_str(run("(upper-case \"aB9z\")")), "AB9Z");
    }

    #[test]
    fn utf8_character_operations() {
        // "caf\195\169" is café: 5 bytes, 4 characters (é is 2 bytes).
        assert_eq!(as_int(run("(length \"caf\\195\\169\")")), 5);
        assert_eq!(as_int(run("(utf8len \"caf\\195\\169\")")), 4);
        // nth / implicit index / first / last / rest are character-based.
        assert_eq!(as_str(run("(nth 3 \"caf\\195\\169\")")), "é");
        assert_eq!(as_str(run("(\"caf\\195\\169\" 3)")), "é");
        assert_eq!(as_str(run("(\"caf\\195\\169\" -1)")), "é");
        assert_eq!(as_str(run("(first \"caf\\195\\169\")")), "c");
        assert_eq!(as_str(run("(last \"caf\\195\\169\")")), "é");
        assert_eq!(as_str(run("(rest \"caf\\195\\169\")")), "afé");
        // explode splits on characters; round-trips through char code points.
        assert!(is_true(
            "(= (explode \"caf\\195\\169\") '(\"c\" \"a\" \"f\" \"é\"))"
        ));
        assert!(is_true(
            "(= (map char (explode \"caf\\195\\169\")) '(99 97 102 233))"
        ));
        // Slicing stays byte-based (binary content): (i str) and slice.
        assert_eq!(as_str(run("(slice \"caf\\195\\169\" 0 3)")), "caf");
        assert_eq!(as_str(run("(3 \"caf\\195\\169\")")), "é"); // bytes 3.. = é
                                                               // ASCII is unchanged (character and byte boundaries coincide).
        assert!(is_true("(= (explode \"abc\") '(\"a\" \"b\" \"c\"))"));
        assert_eq!(as_str(run("(first \"abc\")")), "a");
    }

    #[test]
    fn cow_preserves_value_isolation() {
        // Copy-on-write (ADR-0024) must be observationally identical to ORO
        // deep-copy: a shared value, mutated through one owner, leaves the other
        // unchanged.
        assert!(is_true(
            "(set 'a '(1 2 3)) (set 'b a) (push 9 a) (and (= a '(9 1 2 3)) (= b '(1 2 3)))"
        ));
        // Nested: setf into a shared sublist clones only that owner's path.
        assert!(is_true(
            "(set 'c '((1 2) 3)) (set 'd c) (setf (c 0 1) 99) \
             (and (= c '((1 99) 3)) (= d '((1 2) 3)))"
        ));
        // Strings share and copy on write too (via a place mutation).
        assert!(is_true(
            "(set 's \"abc\") (set 't s) (reverse s) (and (= s \"cba\") (= t \"abc\"))"
        ));
        // A function argument is an independent copy: mutating the parameter's
        // list does not reach the caller's binding.
        assert!(is_true(
            "(define (f x) (push 0 x) x) (set 'g '(1 2)) (f g) (= g '(1 2))"
        ));
    }

    #[test]
    fn arrays_construct_index_and_convert() {
        // Cycle-fill and nil-fill (compare via array-list — an array never
        // equals a list).
        assert!(is_true("(= (array-list (array 5 '(1 2 3))) '(1 2 3 1 2))"));
        assert!(is_true("(= (array-list (array 3)) '(nil nil nil))"));
        // Indexing and length reuse the list paths.
        assert_eq!(as_int(run("((array 4 '(10 20 30 40)) 2)")), 30);
        assert_eq!(as_int(run("(length (array 7 '(0)))")), 7);
        // setf replaces an element in place (fixed length preserved).
        assert!(is_true(
            "(set 'a (array 4 '(0))) (setf (a 2) 9) (= (array-list a) '(0 0 9 0))"
        ));
        // array-list yields a genuine list.
        assert!(is_true("(list? (array-list (array 2 '(1))))"));
    }

    #[test]
    fn arrays_are_a_distinct_type() {
        assert!(is_true("(array? (array 2 '(1)))"));
        assert!(is_true("(not (list? (array 2 '(1))))"));
        assert!(is_true("(not (array? '(1 2)))"));
        // Not an atom, like a list.
        assert!(is_true("(not (atom? (array 2 '(1))))"));
        // Equal to an array element-wise, never to a list.
        assert!(is_true("(= (array 2 '(1 2)) (array 2 '(1 2)))"));
        assert!(is_true("(!= (array 2 '(1 2)) '(1 2))"));
        // An empty array is falsy.
        assert_eq!(as_int(run("(if (array 0) 1 2)")), 2);
    }

    #[test]
    fn arrays_are_fixed_length() {
        // Resizing a fixed-length array errors rather than growing it.
        assert!(matches!(
            run("(set 'a (array 2 '(1))) (catch (push 9 a) 'e)"),
            Value::Nil
        ));
        // Multi-dimensional construction is deferred (an error for now).
        assert!(matches!(run("(catch (array 2 3 '(1)) 'e)"), Value::Nil));
    }

    #[test]
    fn min_max_and_parity() {
        assert_eq!(as_int(run("(min 3 1 2)")), 1);
        assert_eq!(as_int(run("(max 3 1 2)")), 3);
        assert_eq!(as_float(run("(min 3 2.5 4)")), 2.5);
        assert!(is_true("(even? 4)"));
        assert!(is_true("(not (even? 3))"));
        assert!(is_true("(odd? 7)"));
    }

    #[test]
    fn flat_join_member_unique() {
        assert!(is_true("(= (flat '(1 (2 (3)) 4)) '(1 2 3 4))"));
        assert_eq!(as_str(run("(join '(\"a\" \"b\" \"c\") \"-\")")), "a-b-c");
        assert_eq!(as_str(run("(join '(\"x\" \"y\"))")), "xy");
        assert!(is_true("(= (member 3 '(1 2 3 4)) '(3 4))"));
        assert_eq!(as_str(run("(member \"l\" \"hello\")")), "llo");
        assert!(matches!(run("(member 9 '(1 2 3))"), Value::Nil));
        assert!(is_true("(= (unique '(1 2 2 3 1 4)) '(1 2 3 4))"));
    }

    #[test]
    fn swap_places() {
        assert!(is_true(
            "(set 'a 10) (set 'b 20) (swap a b) (and (= a 20) (= b 10))"
        ));
        // Swapping two elements of the same list stays valid across the exchange.
        assert!(is_true(
            "(set 'l '(1 2 3)) (swap (nth 0 l) (nth 2 l)) (= l '(3 2 1))"
        ));
    }

    #[test]
    fn explode_chop_and_digit_sum() {
        assert!(is_true("(= (explode \"abc\") '(\"a\" \"b\" \"c\"))"));
        assert!(is_true("(= (explode \"abcd\" 2) '(\"ab\" \"cd\"))"));
        assert_eq!(as_str(run("(chop \"hello\")")), "hell");
        assert_eq!(as_str(run("(chop \"hello\" 3)")), "he");
        // The qa-longnum idiom: sum the digits of a number's string.
        assert_eq!(as_int(run("(apply + (map int (explode \"12345\")))")), 15);
    }

    #[test]
    fn until_loop_and_extend_place() {
        assert_eq!(as_int(run("(set 'i 0) (until (= i 5) (++ i)) i")), 5);
        assert_eq!(
            as_str(run("(set 's \"\") (extend s \"ab\" \"cd\") s")),
            "abcd"
        );
        assert!(is_true(
            "(set 'l '(1 2)) (extend l '(3 4)) (= l '(1 2 3 4))"
        ));
    }

    #[test]
    fn seed_makes_rng_deterministic() {
        // The same seed yields the same draw; `rand` stays within range.
        assert!(is_true(
            "(= (begin (seed 7) (rand 1000000)) (begin (seed 7) (rand 1000000)))"
        ));
        assert!(is_true("(seed 1) (and (>= (rand 10) 0) (< (rand 10) 10))"));
    }

    #[test]
    fn eval_string_reads_and_evaluates() {
        assert_eq!(as_int(run("(eval-string \"(+ 1 2) (* 3 4)\")")), 12);
        // Sees the current dynamic bindings.
        assert_eq!(as_int(run("(set 'x 10) (eval-string \"(* x 2)\")")), 20);
        // A second argument is the error fallback.
        assert_eq!(as_int(run("(eval-string \"(bad\" 42)")), 42);
    }

    #[test]
    fn symbol_reflection() {
        assert_eq!(as_str(run("(name 'Foo:bar)")), "bar");
        assert_eq!(as_str(run("(name (prefix 'Foo:bar))")), "Foo");
        assert!(is_true("(= (sym \"x\" 'Foo) 'Foo:x)"));
        assert!(is_true(
            "(set 'Baz:a 1) (set 'Baz:b 2) (= (symbols 'Baz) '(Baz:a Baz:b))"
        ));
    }

    #[test]
    fn reflection_predicates_and_title_case() {
        assert!(is_true("(context 'Foo) (context MAIN) (context? Foo)"));
        assert!(is_true("(define (f x) x) (lambda? f)"));
        assert!(is_true("(define-macro (m x) x) (macro? m)"));
        assert!(is_true("(primitive? print)"));
        assert!(is_true("(constant 'k 5) (protected? 'k)"));
        assert_eq!(as_str(run("(title-case \"hello WORLD\")")), "Hello WORLD");
        assert_eq!(
            as_str(run("(title-case \"hello WORLD\" true)")),
            "Hello world"
        );
    }

    #[test]
    fn list_count_select_setops() {
        assert!(is_true("(= (count '(a b) '(a b a b b)) '(2 3))"));
        assert!(is_true("(= (select '(a b c d e) '(0 2 -1)) '(a c e))"));
        assert!(is_true("(= (difference '(1 2 3 2 4) '(2 4)) '(1 3))"));
        assert!(is_true("(= (intersect '(1 2 3 2) '(2 3 5)) '(2 3))"));
    }

    #[test]
    fn parse_splits() {
        assert!(is_true("(= (parse \"  a  b c \") '(\"a\" \"b\" \"c\"))"));
        assert!(is_true(
            "(= (parse \"a,b,,c\" \",\") '(\"a\" \"b\" \"\" \"c\"))"
        ));
        assert!(is_true("(= (parse \"abc\" \"\") '(\"a\" \"b\" \"c\"))"));
    }

    #[test]
    fn rounding_sign_bits_base64() {
        assert_eq!(as_int(run("(int (ceil 3.2))")), 4);
        assert_eq!(as_int(run("(int (floor 3.8))")), 3);
        // newLISP: negative digits round decimal places (positive rounds the
        // integer part), so 2-decimal rounding is `-2`.
        assert!(is_true("(= 3.14 (round 3.14159 -2))"));
        assert!(is_true("(= 100 (round 123.49 2))"));
        assert_eq!(as_int(run("(sgn -5)")), -1);
        assert_eq!(as_int(run("(sgn 0)")), 0);
        assert_eq!(as_int(run("(sgn 9)")), 1);
        assert_eq!(as_str(run("(bits 5)")), "101");
        assert_eq!(as_str(run("(bits 21534)")), "101010000011110");
        assert_eq!(
            as_str(run("(base64-enc \"hello world\")")),
            "aGVsbG8gd29ybGQ="
        );
        // Binary round-trips (byte 9 = tab) through encode/decode.
        assert!(is_true(
            "(= \"a\\009b\" (base64-dec (base64-enc \"a\\009b\")))"
        ));
    }

    #[test]
    fn dostring_iterates_code_points() {
        // For ASCII a code point is its byte: sum of "ABC" = 65 + 66 + 67.
        assert_eq!(
            as_int(run("(set 'a 0) (dostring (c \"ABC\") (set 'a (+ a c))) a")),
            198
        );
        // The break condition stops before running the body for that character.
        assert_eq!(
            as_int(run(
                "(set 'n 0) (dostring (c \"hello\" (= c 108)) (set 'n (+ n 1))) n"
            )),
            2
        );
        // A multi-byte string iterates whole characters, binding each code
        // point — "我能" is two characters (25105, 33021), not six bytes.
        assert_eq!(
            as_int(run("(set 'n 0) (dostring (c \"我能\") (set 'n (+ n 1))) n")),
            2
        );
        assert_eq!(
            as_int(run("(set 'a 0) (dostring (c \"我能\") (set 'a (+ a c))) a")),
            58126
        );
    }
}
