# Ch. 7 — Macros

niiLISP implements the core newLISP fexpr/macro model (`define-macro`, `eval`, `args`, `expand`) correctly for ordinary (non-context-qualified) macros, but the book's own recommended fix for the symbol-capture problem — defining the macro with a context-qualified name (`context:context`) — silently breaks argument-deferral, and `letex` is entirely unimplemented.

**Coverage: 4 ✅ / 1 ⚠️ / 2 ❌**

| Feature | Status | Notes |
|---|---|---|
| `define-macro` (basic fexpr, unevaluated args) | ✅ | `(define-macro (my-if t a b) (if (eval t) (eval a) (eval b)))` defers evaluation correctly; only the taken branch's side effects fire. |
| `eval` inside macro body | ✅ | Explicitly evaluating a deferred argument works as documented. |
| `args` (all-arguments accessor, with/without index) | ✅ | `(args)`, `(args 0)`, `(args 0 0)` all return the expected unevaluated argument list/sub-forms. |
| `expand` (symbol-value substitution into a quoted expression) | ✅ | `(expand '(+ x 1) 'x)` substitutes the current value of `x`, matching newLISP's documented behavior. |
| Evaluated-case pattern (`ecase`-style macro built from `case`+`map`+`eval`) | ✅ | The book's evaluated-case idiom (map over `(args)`, eval each test key, splice into `case`, then `eval`) reproduces correctly. |
| Context-qualified `define-macro` (`(define-macro (ctx:ctx ...) ...)`) — the book's fix for symbol capture | ❌ | Registers as a macro (`macro?` → true, printed body shows `lambda-macro`), but calling it evaluates **all** arguments eagerly at the call site before the macro body runs, defeating deferred evaluation entirely. This is the chapter's headline solution to the capture problem, and it doesn't work. |
| `letex` (local symbol-into-expression expansion) | ❌ | Completely unbound — `(println letex)` → `nil`, and calling it errors with `not a function: nil`. Not present anywhere in the builtin table. Blocks the `dolist-while` and `tracer` macro examples verbatim from the book. |

## Divergences & gaps

### ❌ Context-qualified `define-macro` silently loses argument-deferral

The book's canonical fix for macro variable capture is to define the macro inside its own context, using the `name:name` form:

```
(context 'mymacro)
(define-macro (mymacro:mymacro test true-action false-action)
  (if (eval test) (eval true-action) (eval false-action)))
(context MAIN)
```

Repro — condition is false, so only `"FALSE-BRANCH"` should ever print:

```
$ niilisp -e "(context 'mymacro) (define-macro (mymacro:mymacro test true-action false-action) (if (eval test) (eval true-action) (eval false-action))) (context MAIN) (mymacro (< 3 2) (println \"TRUE-BRANCH\") (println \"FALSE-BRANCH\"))"
```

Actual output:

```
TRUE-BRANCH
FALSE-BRANCH
```

Both branches execute regardless of the test. Isolating further — a macro body that never touches its arguments still triggers their side effects at call time:

```
$ niilisp -e "(context 'mymacro) (define-macro (mymacro:mymacro test true-action false-action) 'ok) (context MAIN) (mymacro (println \"SIDE-EFFECT-TEST\") 1 2)"
```

Actual output:

```
SIDE-EFFECT-TEST
```

(`(println "SIDE-EFFECT-TEST")` fires even though the macro body is just the literal `'ok` and never calls `eval` on anything.) For comparison, the identical body under a plain, non-context-qualified name behaves correctly and suppresses the side effect entirely:

```
$ niilisp -e "(define-macro (m test true-action false-action) 'ok) (m (println \"SIDE-EFFECT-TEST2\") 1 2)"
```

produces no output at all, as expected. `macro?` confirms the context-qualified definition genuinely registers as a fexpr (`(macro? mymacro:mymacro)` → `true`, and printing it shows `(lambda-macro (mymacro:test mymacro:true-action mymacro:false-action) ...)`), so the bug is specifically in call-site dispatch for context-qualified macro symbols, not in `define-macro` itself.

### ❌ `letex` is unimplemented

```
$ niilisp -e '(let (x 10) (letex (y x) (println (quote y))))'
```

Actual output:

```
niilisp: not a function: nil
```

`(println letex)` returns `nil` (unbound symbol), confirming it is simply missing rather than misbehaving. This blocks the book's `dolist-while` (combined `dolist`+`do-while` macro) and `tracer` (function-call logger) examples from working verbatim, since both rely on `letex` to splice extracted `args` sub-forms into a template before evaluation.

### ⚠️ Related: `let` rejects an odd-length/unpaired binding list

Noticed while testing macro-adjacent `let` usage (used in the book's `dolist-while` example as `(let (y) ...)` for a single unbound local):

```
$ niilisp -e '(let (y) (println y))'
```

Actual output:

```
niilisp: let: binding list must have an even length
```

newLISP allows a bare symbol in a `let` binding to default to `nil`. This isn't a macro-chapter feature per se, but it stops the book's `dolist-while` macro pattern from running as written even setting `letex` aside.
