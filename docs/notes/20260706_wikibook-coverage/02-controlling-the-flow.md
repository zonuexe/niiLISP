# Ch. 2 — Controlling the flow

Most conditionals, loops, and binding forms work exactly as the WikiBook describes; the remaining gap is a missing `doargs`.

**Coverage: 26 ✅ / 1 ⚠️ / 2 ❌**  *(updated 2026-07-06: `$idx` for dolist/dostring/dotree/map/while/until/do-while/do-until; `letn` implemented)*

> Correction: `dotree` was mis-tested with a two-variable spec `(k v Ctx)`. newLISP's real syntax is `(dotree (sym sym-context [bool]) body)` — a **single** loop variable. With the correct form it works: `(dotree (s Foo) (println s))` iterates `Foo`'s symbols in sorted order. Re-classified ✅.

| Feature | Status | Notes |
|---|---|---|
| `if` (else clause) | ✅ | |
| `if` (multiple test/action pairs) | ✅ | |
| `when` | ✅ | multiple body expressions all execute |
| `cond` | ✅ | |
| `case` | ✅ | matches literal, unevaluated values; `true` catch-all works |
| `dolist` | ✅ | `$idx` loop index now populated (fixed 2026-07-06) |
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
| `letn` | ✅ | implemented 2026-07-06 — sequential binding (`(letn (x 2 y (pow x 3)) …)`) |
| `local` | ✅ | uninitialized locals are `nil`; `set` works inside |
| comma-separated local params, `(x y , a b c)` | ✅ | |
| `define` (basic + args) | ✅ | |
| default parameter values `(a 1)` | ✅ | |
| `fn` | ✅ | |
| `lambda` (synonym for `fn`) | ✅ | |
| `args` | ✅ | returns unconsumed trailing arguments |
| `doargs` | ❌ | not implemented — errors as an unbound symbol called as a function |
| `amb` | ✅ | returns a random element across repeated calls |
| `map` with `fn` body using `$idx` | ✅ | `$idx` now populated (fixed 2026-07-06) |
| dynamic scoping | ✅ | function sees caller's `x` from `for`-loop rebind, then sees restored outer value afterward |
| `unless` | ✅ | inverse of `when`, works |
| `dotree` | ✅ | works with the correct single-var syntax `(dotree (sym context) body)`; the earlier failure was a wrong two-var test spec |

## Divergences & gaps

### ~~`$idx` never populated (dolist, map)~~ — FIXED 2026-07-06

`$idx` is now maintained by `dolist`, `dostring`, `dotree`, `map`, and the
`while`/`until`/`do-while`/`do-until` loops (`src/eval.rs`, `src/builtins.rs`;
regression test `tests/loop_idx.rs`). It is dynamically scoped and restored on
loop exit.

```
$ niilisp -e "(dolist (x '(a b d e f g)) (println \$idx \":\" x))"
0:a
1:b
2:d
3:e
4:f
5:g
$ niilisp -e "(println (map (fn (x) (list \$idx x)) '(a b c)))"
((0 a) (1 b) (2 c))
```

(`for` and `dotimes` intentionally do **not** set `$idx` — newLISP does not document it there, and their loop variable already is the index.)

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
