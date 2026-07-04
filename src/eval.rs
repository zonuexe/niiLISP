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
        self.apply(callable, &items[1..])
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
            "begin" => self.eval_body(args),
            "define" => self.sf_define(args),
            "set" => self.sf_set(args, true),
            "setq" => self.sf_set(args, false),
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
    fn strings_count_bytes() {
        // "abc" is 3 bytes; a 2-byte UTF-8 char makes the byte length 4 (ADR-0013).
        assert_eq!(as_int(run("(length \"abc\")")), 3);
        assert_eq!(as_int(run("(length \"a\\195\\169\")")), 3);
    }
}
