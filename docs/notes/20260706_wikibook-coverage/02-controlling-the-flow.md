# Ch. 2 — Controlling the flow

Most conditionals, loops, and binding forms work exactly as the WikiBook describes; the notable gaps are the `$idx` loop-index system variable (always `nil`), a completely missing `letn`, and a missing `doargs`.

**Coverage: 23 ✅ / 3 ⚠️ / 3 ❌**

> Correction: `dotree` was mis-tested with a two-variable spec `(k v Ctx)`. newLISP's real syntax is `(dotree (sym sym-context [bool]) body)` — a **single** loop variable. With the correct form it works: `(dotree (s Foo) (println s))` iterates `Foo`'s symbols in sorted order. Re-classified ✅.

| Feature | Status | Notes |
|---|---|---|
| `if` (else clause) | ✅ | |
| `if` (multiple test/action pairs) | ✅ | |
| `when` | ✅ | multiple body expressions all execute |
| `cond` | ✅ | |
| `case` | ✅ | matches literal, unevaluated values; `true` catch-all works |
| `dolist` | ⚠️ | iteration and values correct, but `$idx` is always `nil` |
| `dolist` break-test (3rd arg) | ✅ | stops before the element that satisfies the test |
| `dostring` | ✅ | yields character codes, matching the book |
| `dotimes` | ✅ | |
| `for` (with step) | ✅ | |
| `while` | ✅ | |
| `do-while` | ✅ | executes body before testing (test-last) |
| `until` | ✅ | |
| `do-until` | ✅ | executes body before testing (test-last) |
| `catch` / `throw` | ✅ | |
| `begin` | ✅ | |
| `and` (short-circuit) | ✅ | stops at first `nil`, no evaluation of later forms |
| `or` (short-circuit) | ✅ | stops at first true value |
| `let` | ✅ | parallel binding confirmed (later binding can't see earlier locals) |
| `letn` | ❌ | not implemented at all — every invocation errors |
| `local` | ✅ | uninitialized locals are `nil`; `set` works inside |
| comma-separated local params, `(x y , a b c)` | ✅ | |
| `define` (basic + args) | ✅ | |
| default parameter values `(a 1)` | ✅ | |
| `fn` | ✅ | |
| `lambda` (synonym for `fn`) | ✅ | |
| `args` | ✅ | returns unconsumed trailing arguments |
| `doargs` | ❌ | not implemented — errors as an unbound symbol called as a function |
| `amb` | ✅ | returns a random element across repeated calls |
| `map` with `fn` body using `$idx` | ⚠️ | element values correct, but `$idx` is always `nil` (same root cause as dolist) |
| dynamic scoping | ✅ | function sees caller's `x` from `for`-loop rebind, then sees restored outer value afterward |
| `unless` | ✅ | inverse of `when`, works |
| `dotree` | ✅ | works with the correct single-var syntax `(dotree (sym context) body)`; the earlier failure was a wrong two-var test spec |

## Divergences & gaps

### `$idx` never populated (dolist, map)

```
$ niilisp -e "(dolist (i (list 10 20 30)) (println \"Element \" \$idx \": \" i))"
Element nil: 10
Element nil: 20
Element nil: 30
```
```
$ niilisp -e "(map (fn (i) (println \"Element \" \$idx \": \" i)) '(10 20 30))"
Element nil: 10
Element nil: 20
Element nil: 30
```
Element values are correct but the per-iteration index that the book relies on (`$idx`, starting at 0) is never bound — it is a bare unbound symbol in niiLISP's implementation (confirmed absent from `src/*.rs`), so it always reads as `nil`.

### `letn` — missing entirely

```
$ niilisp -e '(letn (x 2) (println x))'
niilisp: not a function: nil
$ niilisp -e '(letn (x 2 y (pow x 3) z (pow x 4)) (println x " " y " " z))'
niilisp: not a function: nil
```
`letn` does not appear anywhere in the source (`grep -n letn src/*.rs` returns nothing). Even the simplest single-binding form fails immediately — sequential-binding `let` is not available under any name.

### `doargs` — missing entirely

```
$ niilisp -e '(define (flexible) (doargs (a) (println "argument " $idx " is " a))) (flexible 10 20 30)'
niilisp: not a function: nil
```
Not registered as a special form or builtin anywhere in the codebase (confirmed via source search); `args` itself works fine, but the iterating counterpart `doargs` does not exist.

### `dotree` — present but unusable

```
$ niilisp -e "(context 'Foo) (set 'Foo:a 1) (context MAIN) (dotree (k v Foo) (println k))"
niilisp: dotree: expected a context, got nil
$ niilisp -e "(context 'Foo) (set 'Foo:a 1) (context MAIN) (dotree (k v 'Foo) (println k))"
niilisp: dotree: expected a context, got nil
```
The special form exists (`src/eval.rs:1521`, `sf_dotree`) and explicitly expects its second spec element to evaluate to `Value::Context` or `Value::Symbol`, but a plain context reference (quoted or unquoted) evaluates to `nil` instead, so every invocation fails at the type check. Qualified symbol access like `Foo:a` works fine — only the bare context-as-value form is broken.
