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
}

/// A dynamic-binding scope. On drop it restores every slot it changed, in
/// reverse order — including on error unwind (ADR-0006).
struct Scope<'a> {
    interp: &'a Interp,
    saved: Vec<(SymId, Option<Value>)>,
}

impl<'a> Scope<'a> {
    fn new(interp: &'a Interp) -> Self {
        Scope {
            interp,
            saved: Vec::new(),
        }
    }

    fn bind(&mut self, sym: SymId, val: Value) {
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
        };
        builtins::install(&interp);
        crate::ffi::install(&interp);
        interp
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

    pub fn repr(&self, v: &Value) -> String {
        to_repr(v, &self.interner.borrow())
    }

    /// Evaluate one expression.
    pub fn eval(&self, v: &Value) -> Result<Value, Signal> {
        match v {
            Value::Symbol(id) => Ok(self.lookup(*id)),
            Value::List(items) => self.eval_list(items),
            other => Ok(other.clone()),
        }
    }

    /// Evaluate a body, returning the last value (empty body -> nil).
    fn eval_body(&self, body: &[Value]) -> Result<Value, Signal> {
        let mut result = Value::Nil;
        for form in body {
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
            // A list in function position indexes itself: (lst i) / (lst i j).
            list @ Value::List(_) => {
                let mut v = list;
                for idx in &items[1..] {
                    let i = self.eval_index(idx)?;
                    v = index_one(i, &v);
                }
                Ok(v)
            }
            // Default functor: applying a context constructs an object (ADR-0010).
            Value::Context(ctx) => self.construct(ctx, &items[1..]),
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

    /// Applying a context: call its default functor `Ctx:Ctx` if defined, else
    /// build a symbol-tagged object list.
    fn construct(&self, ctx: SymId, arg_exprs: &[Value]) -> Result<Value, Signal> {
        let ctx_name = self.sym_name(ctx);
        let functor = self.intern(&format!("{}:{}", ctx_name, ctx_name));
        if let Value::Lambda(l) = self.lookup(functor) {
            let args = self.eval_args(arg_exprs)?;
            return self.call_lambda(&l, args);
        }
        let mut obj = Vec::with_capacity(arg_exprs.len() + 1);
        obj.push(Value::Symbol(ctx));
        for e in arg_exprs {
            obj.push(self.eval(e)?);
        }
        Ok(Value::List(obj))
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

    /// `(i list)` -> element i; `(i count list)` -> slice of `count` from `i`.
    fn implicit_index(&self, i: i64, rest: &[Value]) -> Result<Value, Signal> {
        match rest.len() {
            1 => {
                let target = self.eval(&rest[0])?;
                Ok(index_one(i, &target))
            }
            2 => {
                let count = match self.eval(&rest[0])? {
                    Value::Int(n) => n,
                    _ => return Err(Signal::error("implicit slice: count must be an integer")),
                };
                let target = self.eval(&rest[1])?;
                Ok(slice(i, count, &target))
            }
            _ => Err(Signal::error(
                "implicit index: expected (i list) or (i count list)",
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
        self.eval_body(&l.body)
        // `scope` drops here, restoring the outer bindings.
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
            "local" => self.sf_local(args),
            "++" | "inc" => self.sf_incr(args, 1),
            "--" | "dec" => self.sf_incr(args, -1),
            "push" => self.sf_push(args),
            "pop" => self.sf_pop(args),
            "reverse" => self.sf_reverse(args),
            "sort" => self.sf_sort(args),
            "rotate" => self.sf_rotate(args),
            "replace" => self.sf_replace(args),
            "set-ref" => self.sf_setref(args, false),
            "set-ref-all" => self.sf_setref(args, true),
            "begin" => self.eval_body(args),
            "define" => self.sf_define(args),
            "set" => self.sf_set(args, true),
            "setq" | "setf" => self.sf_setf(args),
            "constant" => self.sf_constant(args),
            "self" => self.sf_self(args),
            "time" => self.sf_time(args),
            "let" => self.sf_let(args),
            "lambda" | "fn" => self.sf_lambda(args, false),
            "lambda-macro" => self.sf_lambda(args, true),
            "define-macro" => self.sf_define_macro(args),
            "catch" => self.sf_catch(args),
            "throw" => self.sf_throw(args),
            _ => return None,
        };
        Some(r)
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
        let mut result = Value::Nil;
        while self.eval(cond)?.is_truthy() {
            result = self.eval_body(&args[1..])?;
        }
        Ok(result)
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
            Some(e) => match self.eval(e)? {
                Value::Int(n) => n,
                Value::Float(f) => f as i64,
                _ => return Err(Signal::error("++/--: amount must be a number")),
            },
            None => 1,
        };
        let apply = |v: &mut Value| -> Result<Value, Signal> {
            let base = match v {
                Value::Int(n) => *n,
                Value::Nil => 0,
                Value::Float(f) => *f as i64,
                _ => return Err(Signal::error("++/--: place is not a number")),
            };
            let next = base.wrapping_add(sign.wrapping_mul(delta));
            *v = Value::Int(next);
            Ok(Value::Int(next))
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
        let items = match self.eval(&spec[1])? {
            Value::List(l) => l,
            Value::Nil => Vec::new(),
            other => {
                return Err(Signal::Error(format!(
                    "dolist: expected a list, got {}",
                    self.repr(&other)
                )))
            }
        };
        let break_cond = spec.get(2);

        let mut scope = Scope::new(self);
        scope.bind(var, Value::Nil);
        let mut result = Value::Nil;
        for item in items {
            self.set_global(var, item);
            if let Some(cond) = break_cond {
                if self.eval(cond)?.is_truthy() {
                    break;
                }
            }
            result = self.eval_body(&args[1..])?;
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
        for s in syms {
            match s {
                Value::Symbol(id) => scope.bind(*id, Value::Nil),
                _ => return Err(Signal::error("local: expected a symbol")),
            }
        }
        self.eval_body(&args[1..])
    }

    fn sf_push(&self, args: &[Value]) -> Result<Value, Signal> {
        // (push value place [index]) — place designates a list (or nil -> list).
        if args.len() < 2 {
            return Err(Signal::error("push: expected (push value place [index])"));
        }
        let value = self.eval(&args[0])?;
        let place = self.resolve_place(&args[1])?;
        let index = match args.get(2) {
            Some(e) => match self.eval(e)? {
                Value::Int(n) => Some(n),
                _ => return Err(Signal::error("push: index must be an integer")),
            },
            None => None,
        };
        self.with_place_mut(&place, move |loc| {
            let list = match loc {
                Value::List(l) => l,
                Value::Nil => {
                    *loc = Value::List(Vec::new());
                    match loc {
                        Value::List(l) => l,
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

    fn sf_pop(&self, args: &[Value]) -> Result<Value, Signal> {
        // (pop place [index]) — remove and return an element from a list place.
        let place = self.resolve_place(
            args.first()
                .ok_or_else(|| Signal::error("pop: expected a place"))?,
        )?;
        let index = match args.get(1) {
            Some(e) => match self.eval(e)? {
                Value::Int(n) => n,
                _ => return Err(Signal::error("pop: index must be an integer")),
            },
            None => 0,
        };
        self.with_place_mut(&place, move |loc| {
            let list = match loc {
                Value::List(l) => l,
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
            Value::List(l) => l.reverse(),
            Value::Str(b) => b.reverse(),
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
                    l.sort_by(|a, b| self.value_cmp(a, b));
                }
                Ok(loc.clone())
            }),
            Err(_) => {
                let mut v = self.eval(target)?;
                if let Value::List(l) = &mut v {
                    l.sort_by(|a, b| self.value_cmp(a, b));
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
        let newval = self.eval(&args[2])?;
        self.place_or_value(&args[1], |loc| do_replace(loc, &target, &newval))
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
                // when the container is an indexable value (list/string); this
                // rules out ordinary function calls like (list 1 2 3).
                let mut p = self.resolve_place(&items[0])?;
                if items.len() > 1
                    && !matches!(self.read_place(&p)?, Value::List(_) | Value::Str(_))
                {
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

    fn sf_let(&self, args: &[Value]) -> Result<Value, Signal> {
        // (let (s1 e1 s2 e2 ...) body...) — flat binding list (see qa-foop).
        let bindings = match args.first() {
            Some(Value::List(b)) => b,
            _ => return Err(Signal::error("let: expected a binding list")),
        };
        if bindings.len() % 2 != 0 {
            return Err(Signal::error("let: binding list must have an even length"));
        }

        // newLISP `let` is parallel: evaluate all inits in the outer scope first.
        let mut pending: Vec<(SymId, Value)> = Vec::with_capacity(bindings.len() / 2);
        let mut i = 0;
        while i < bindings.len() {
            let sym = match &bindings[i] {
                Value::Symbol(id) => *id,
                other => {
                    return Err(Signal::Error(format!(
                        "let: binding name is not a symbol: {}",
                        self.repr(other)
                    )))
                }
            };
            let val = self.eval(&bindings[i + 1])?;
            pending.push((sym, val));
            i += 2;
        }

        let mut scope = Scope::new(self);
        for (sym, val) in pending {
            scope.bind(sym, val);
        }
        self.eval_body(&args[1..])
    }

    fn sf_lambda(&self, args: &[Value], is_fexpr: bool) -> Result<Value, Signal> {
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

    fn sf_self(&self, args: &[Value]) -> Result<Value, Signal> {
        // (self) -> the current object; (self i j ...) -> nested element.
        let mut place = self.current_self()?;
        for a in args {
            place.path.push(self.eval_index(a)?);
        }
        self.read_place(&place)
    }

    fn sf_time(&self, args: &[Value]) -> Result<Value, Signal> {
        // (time expr) -> milliseconds spent evaluating expr.
        use std::time::Instant;
        let expr = args
            .first()
            .ok_or_else(|| Signal::error("time: missing expression"))?;
        let start = Instant::now();
        self.eval(expr)?;
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
                        self.set_global(sym, Value::Str(msg.into_bytes()));
                        Ok(Value::Nil)
                    }
                }
            }
            // One-arg form: return the value, or the caught thrown value / error.
            None => match result {
                Ok(v) => Ok(v),
                Err(Signal::Throw(v)) => Ok(v),
                Err(Signal::Error(msg)) => Ok(Value::Str(msg.into_bytes())),
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
            Value::List(l) => {
                let i = if raw < 0 { l.len() as i64 + raw } else { raw };
                if i < 0 || i as usize >= l.len() {
                    return None;
                }
                cur = &mut l[i as usize];
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
            l.rotate_right(k);
        }
        Value::Str(b) if !b.is_empty() => {
            let len = b.len() as i64;
            let k = (((n % len) + len) % len) as usize;
            b.rotate_right(k);
        }
        _ => {}
    }
}

/// Replace occurrences of `target` with `newval` inside `loc`, in place.
fn do_replace(loc: &mut Value, target: &Value, newval: &Value) {
    match loc {
        Value::List(l) => {
            for e in l.iter_mut() {
                if crate::builtins::values_equal(e, target) {
                    *e = newval.clone();
                }
            }
        }
        Value::Str(b) => {
            if let (Value::Str(t), Value::Str(n)) = (target, newval) {
                *b = replace_bytes(b, t, n);
            }
        }
        _ => {}
    }
}

/// Deep-replace occurrences of `key` with `new` within nested lists. With
/// `all == false`, stops after the first replacement. Returns whether it did.
fn deep_replace(v: &mut Value, key: &Value, new: &Value, all: bool) -> bool {
    if let Value::List(l) = v {
        for e in l.iter_mut() {
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
        Value::List(l) => {
            let idx = if i < 0 { l.len() as i64 + i } else { i };
            if idx < 0 || idx as usize >= l.len() {
                Value::Nil
            } else {
                l[idx as usize].clone()
            }
        }
        Value::Str(b) => {
            let idx = if i < 0 { b.len() as i64 + i } else { i };
            if idx < 0 || idx as usize >= b.len() {
                Value::Nil
            } else {
                Value::Str(vec![b[idx as usize]])
            }
        }
        _ => Value::Nil,
    }
}

/// `count` elements starting at `start` (negative start counts from the end).
fn slice(start: i64, count: i64, target: &Value) -> Value {
    let take = |len: usize| -> (usize, usize) {
        let s = if start < 0 {
            (len as i64 + start).max(0)
        } else {
            start.min(len as i64)
        } as usize;
        let end = (s as i64 + count.max(0)).min(len as i64) as usize;
        (s, end)
    };
    match target {
        Value::List(l) => {
            let (s, e) = take(l.len());
            Value::List(l[s..e].to_vec())
        }
        Value::Str(b) => {
            let (s, e) = take(b.len());
            Value::Str(b[s..e].to_vec())
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
        let forms = {
            let mut it = interp.interner.borrow_mut();
            Reader::new(src.as_bytes(), &mut it).read_all().unwrap()
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
        assert_eq!(as_int(run("(2 (list 10 20 30 40))")), 30);
        // (0 2 list) -> first two elements.
        assert_eq!(as_int(run("(length (0 2 (list 5 6 7 8)))")), 2);
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
}
