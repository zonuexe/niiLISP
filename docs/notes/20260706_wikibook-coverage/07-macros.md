# Ch. 7 — Macros

niiLISP implements the core newLISP fexpr/macro model (`define-macro`, `eval`, `args`, `expand`) correctly for ordinary (non-context-qualified) macros. The one remaining gap is the book's own recommended fix for the symbol-capture problem — defining the macro with a context-qualified name (`context:context`) — which silently breaks argument-deferral.

**Coverage: 6 ✅ / 0 ⚠️ / 1 ❌**  *(updated 2026-07-06: `letex` implemented; `let` now accepts a bare-symbol binding)*

| Feature | Status | Notes |
|---|---|---|
| `define-macro` (basic fexpr, unevaluated args) | ✅ | `(define-macro (my-if t a b) (if (eval t) (eval a) (eval b)))` defers evaluation correctly; only the taken branch's side effects fire. |
| `eval` inside macro body | ✅ | Explicitly evaluating a deferred argument works as documented. |
| `args` (all-arguments accessor, with/without index) | ✅ | `(args)`, `(args 0)`, `(args 0 0)` all return the expected unevaluated argument list/sub-forms. |
| `expand` (symbol-value substitution into a quoted expression) | ✅ | `(expand '(+ x 1) 'x)` substitutes the current value of `x`, matching newLISP's documented behavior. |
| Evaluated-case pattern (`ecase`-style macro built from `case`+`map`+`eval`) | ✅ | The book's evaluated-case idiom (map over `(args)`, eval each test key, splice into `case`, then `eval`) reproduces correctly. |
| Context-qualified `define-macro` (`(define-macro (ctx:ctx ...) ...)`) — the book's fix for symbol capture | ❌ | Registers as a macro (`macro?` → true, printed body shows `lambda-macro`), but calling it evaluates **all** arguments eagerly at the call site before the macro body runs, defeating deferred evaluation entirely. This is the chapter's headline solution to the capture problem, and it doesn't work. |
| `letex` (local symbol-into-expression expansion) | ✅ | Implemented 2026-07-06 (`(letex (x 1 y 2) '(x y))` → `(1 2)`); both flat and parenthesized binding syntaxes, optional initializers. |
| `let` with a bare-symbol binding (`(let (y) …)`) | ✅ | Fixed 2026-07-06 — a bare symbol now defaults to `nil` (and `let` also accepts the fully-parenthesized `((s e) …)` form). |

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

### ~~`letex` is unimplemented~~ — FIXED 2026-07-06

`letex` (and its sibling `letn`) are now implemented as special forms — see the "binding forms" slice. Both accept the flat (`(letex (x 1 y 2) …)`) and fully-parenthesized (`(letex ((x 1) (y 2)) …)`) syntaxes with optional initializers, matching newLISP:

```
$ niilisp -e "(let (x 10) (letex (y x) (println 'y)))"
10
```

### ~~`let` rejects an odd-length/unpaired binding list~~ — FIXED 2026-07-06

`let` now accepts a bare symbol (defaulting to `nil`) and the fully-parenthesized binding form, so the book's `dolist-while` macro pattern's `(let (y) …)` runs:

```
$ niilisp -e '(println (let (y) y))'
nil
```
