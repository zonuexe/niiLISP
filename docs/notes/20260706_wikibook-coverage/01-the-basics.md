# Ch. 1 — The basics

Audit of niiLISP against the *Introduction to newLISP* WikiBook chapter "The basics"
(https://en.wikibooks.org/wiki/Introduction_to_newLISP/The_basics). All items were
probed functionally against `target/release/niilisp`, not just checked for absence of error.

**Coverage: 17 ✅ / 1 ⚠️ / 0 ❌**

| Feature | Status | Notes |
|---|---|---|
| List literal as data, `'(1 2 3 4 5)` etc. (int/string/symbol/fn/mixed/nested lists) | ✅ | All literal shapes round-trip via `println` exactly as shown in book. |
| Rule 2: first element of a list is a function, e.g. `(+ 2 2)` | ✅ | `4` |
| `+` variadic addition, `(+ 1 2 3 4 5 6 7 8 9)` | ✅ | `45` |
| `max`, `(max 1 1.2 12.1 12.2 1.3 1.2 12.3)` | ✅ | `12.3` |
| `print` (no trailing newline, concatenates args) | ✅ | `(print 1 2 "buckle" "my" "shoe")` → `12bucklemyshoe` |
| `println` | ✅ | Adds newline as expected. |
| `directory` (arg form and no-arg form) | ✅ | `(directory "/tmp")` and `(directory)` both list current/parent dirs and contents. |
| `read-file` | ✅ | Reads file contents as string correctly. |
| Nested-list evaluation, `(* (+ 1 2) (+ 3 4))` | ✅ | `21` |
| Rule 3: quoting prevents evaluation, `'(+ 2 2)` | ✅ | Returns unevaluated list `(+ 2 2)`. |
| Quoted data lists, `'(2006 1 12)`, `'("Arthur" "J" "Chopin")` | ✅ | Both preserved as-is. |
| `set` (requires quoted symbol) | ✅ | `(set 'alphabet "abc...z")` then `(upper-case alphabet)` works. |
| `upper-case` | ✅ | Produces uppercase string. |
| `first` (composed with upper-case) | ✅ | `(first (upper-case alphabet))` → `"A"` |
| `define` with evaluated init, `(define x (+ 2 2))` | ✅ | `x` → `4` |
| `define` with quoted init, `(define y '(+ 2 2))` | ✅ | `y` → `(+ 2 2)`; `'y` → `y` |
| `setf` / `setq` (no quote needed) | ✅ | Both `(setf y (+ 2 2))` and `(setq y (+ 2 2))` → `4` |
| `exit` | ⚠️ | Terminates the script (subsequent forms not evaluated) with exit code 0, matching expected behavior — flagged only because behavior wasn't cross-checked against real newLISP's documented exit-code semantics (no reference newlisp binary available in this environment for diffing exact process exit code / message). Functionally correct. |
| Comments (`;`), shebang line (`#!/usr/bin/env ...`) | ✅ | Comment ignored to end of line; shebang script executes normally when run as a file. |

## Divergences & gaps

### ⚠️ `exit`

Command:
```
niilisp -e '(println "before") (exit) (println "after")'
```
Actual output:
```
before
```
Exit code: `0`.

This matches the book's described behavior (terminates the interpreter/script). It's marked
⚠️ only as a caveat: no reference `newlisp` binary was available in this environment to
confirm niiLISP's exit code and message parity with upstream newLISP in edge cases (e.g.
`(exit N)` with a status code argument). No functional gap was observed for the plain
`(exit)` case taught in this chapter.

## Notes

- No ❌ items: every function, form, and worked example in this chapter is present in
  niiLISP and produces output matching the book.
- The book's list-literal examples (e.g. `(1 2 3 4 5)`, `(sin cos tan atan)`) are shown in a
  descriptive/data context (implicitly quoted or as illustration), not evaluated bare at
  top level. Evaluating such a list bare, e.g. `(1 2 3 4 5)`, correctly errors in niiLISP
  (`implicit index: expected (i seq) or (i len seq)`) because `1` is not bound as a function —
  this is expected LISP-1 behavior consistent with the book's own Rule 2, not a divergence.
