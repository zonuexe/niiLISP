//! The evaluator: tree-walks the live `Vec`-backed list structure (ADR-0007).
//!
//! Dynamic scoping (ADR-0006) is implemented with per-symbol value slots plus a
//! save/restore rebinding stack; the restore rides a `Drop` guard so bindings
//! are reinstated even when an error unwinds. Non-local exit is `Result<Value,
//! Signal>` (ADR-0011), so error unwinding and scope restoration share one path.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins;
use crate::printer::to_repr;
use crate::value::{Interner, Lambda, SymId, Value};

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
}

/// A dynamic-binding scope. On drop it restores every slot it changed, in
/// reverse order — including on error unwind (ADR-0006).
struct Scope<'a> {
    interp: &'a Interp,
    saved: Vec<(SymId, Option<Value>)>,
}

impl<'a> Scope<'a> {
    fn new(interp: &'a Interp) -> Self {
        Scope { interp, saved: Vec::new() }
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
        };
        builtins::install(&interp);
        interp
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
            if let Some(result) = self.try_special_form(&name, &items[1..]) {
                return result;
            }
        }

        let callable = self.eval(head)?;
        // Implicit indexing: (i list) or (i count list) when the head is an
        // integer (newLISP's leading-integer list access).
        if let Value::Int(i) = callable {
            return self.implicit_index(i, &items[1..]);
        }
        self.apply(callable, &items[1..])
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
            _ => Err(Signal::error("implicit index: expected (i list) or (i count list)")),
        }
    }

    /// Call an already-resolved callable with already-evaluated arguments.
    /// Used by higher-order builtins (`map`, `apply`, `filter`).
    pub fn call(&self, callable: &Value, args: Vec<Value>) -> Result<Value, Signal> {
        match callable {
            Value::Builtin(b) => (b.func)(self, &args),
            Value::Lambda(l) => self.call_lambda(l, args),
            Value::Fexpr(l) => self.call_lambda(l, args),
            other => Err(Signal::Error(format!("not a function: {}", self.repr(other)))),
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

    /// Bind parameters (dynamically) and evaluate the body.
    fn call_lambda(&self, l: &Lambda, args: Vec<Value>) -> Result<Value, Signal> {
        let mut scope = Scope::new(self);
        let mut args = args.into_iter();
        for &p in &l.params {
            scope.bind(p, args.next().unwrap_or(Value::Nil));
        }
        self.eval_body(&l.body)
        // `scope` drops here, restoring the outer bindings.
    }

    // ---- Special forms ---------------------------------------------------

    fn try_special_form(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Signal>> {
        let r = match name {
            "quote" => Ok(args.first().cloned().unwrap_or(Value::Nil)),
            "if" => self.sf_if(args),
            "cond" => self.sf_cond(args),
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
            "begin" => self.eval_body(args),
            "define" => self.sf_define(args),
            "set" => self.sf_set(args, true),
            "setq" => self.sf_set(args, false),
            "setf" => self.sf_setf(args),
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
        let mut last = Value::Nil;
        let mut i = 0;
        while i + 1 < args.len() {
            let val = self.eval(&args[i + 1])?;
            let place = self.resolve_place(&args[i])?;
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

        let integral = is_int(&from)
            && is_int(&to)
            && step.as_ref().map(is_int).unwrap_or(true);
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

    // ---- place resolution (ORO reference model, ADR-0006) ----------------

    /// Resolve a place expression to a rooted path into a symbol's stored value.
    /// Supports: `sym`, `(place idx...)` implicit indexing, `(nth i place)`,
    /// `(first place)`, `(last place)`.
    fn resolve_place(&self, expr: &Value) -> Result<Place, Signal> {
        match expr {
            Value::Symbol(id) => Ok(Place {
                root: *id,
                path: Vec::new(),
            }),
            Value::List(items) if !items.is_empty() => {
                if let Value::Symbol(op) = &items[0] {
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
                        _ => {}
                    }
                }
                // Implicit indexing place: (container idx idx ...)
                let mut p = self.resolve_place(&items[0])?;
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

    /// Mutate the value at a place, creating the root slot if absent.
    fn with_place_mut<R>(
        &self,
        place: &Place,
        f: impl FnOnce(&mut Value) -> Result<R, Signal>,
    ) -> Result<R, Signal> {
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
            last = self.eval(&args[i + 1])?;
            self.set_global(sym, last.clone());
            i += 2;
        }
        Ok(last)
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

    fn parse_params(&self, forms: &[Value]) -> Result<Vec<SymId>, Signal> {
        let mut params = Vec::with_capacity(forms.len());
        for f in forms {
            match f {
                Value::Symbol(id) => params.push(*id),
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
struct Place {
    root: SymId,
    path: Vec<i64>,
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
        assert_eq!(as_int(run("(set 's 0) (for (i 1 5) (set 's (+ s i))) s")), 15);
        // Descending when from > to.
        assert_eq!(as_int(run("(set 's 0) (for (i 5 1) (set 's (+ s 1))) s")), 5);
    }

    #[test]
    fn dolist_iterates() {
        assert_eq!(
            as_int(run("(set 's 0) (dolist (x (list 2 3 4)) (set 's (+ s x))) s")),
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
            as_int(run("(length (filter (lambda (x) (< x 3)) (list 1 2 3 4 5)))")),
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
        assert_eq!(as_int(run("(set 'L (list 1 2 3)) (setf (L 1) 99) (nth 1 L)")), 99);
        assert_eq!(as_int(run("(set 'L (list 1 2 3)) (setf (nth 0 L) 7) (first L)")), 7);
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
            as_int(run("(set 'M (list (list 1) (list 2))) (push 9 (M 0) -1) (length (nth 0 M))")),
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
    fn strings_count_bytes() {
        // "abc" is 3 bytes; a 2-byte UTF-8 char makes the byte length 4 (ADR-0013).
        assert_eq!(as_int(run("(length \"abc\")")), 3);
        assert_eq!(as_int(run("(length \"a\\195\\169\")")), 3);
    }
}
