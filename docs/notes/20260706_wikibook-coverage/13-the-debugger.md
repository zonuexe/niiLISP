# Ch. 13 — The debugger

Audit of niiLISP against the *Introduction to newLISP* WikiBook chapter "The debugger"
(https://en.wikibooks.org/wiki/Introduction_to_newLISP/The_debugger). All items were
probed functionally against `target/release/niilisp`, not just checked for absence of error.

**Coverage: 2 ✅ / 0 ⚠️ / 7 ❌**

| Feature | Status | Notes |
|---|---|---|
| `catch` | ✅ | Catches both explicit `throw` values and runtime errors (e.g. type errors); returns caught value; supports `(catch expr 'result-sym)` form. |
| `throw` | ✅ | `(catch (throw 42))` → `42`, matching book semantics. |
| `trace` | ❌ | Symbol is entirely unbound (`nil`); calling it errors `not a function: nil`. No step debugger exists. |
| `trace-highlight` | ❌ | Unbound; same failure mode. |
| `debug` | ❌ | Unbound; `(debug (load "file.lsp"))` shortcut does not exist. |
| Interactive debugger prompt (`s`/`n`/`c` keys) | ❌ | No debugger REPL exists at all — `trace`/`debug` never engage any stepping prompt since the underlying primitives are unbound. |
| `error-event` | ❌ | Unbound; can't install a custom error handler. |
| `throw-error` | ❌ | Unbound; can't raise a user-defined catchable error with a message. |
| `last-error` | ❌ | Unbound; no way to retrieve the last error info list after a `catch`. |
| `sys-error` | ❌ | Unbound; can't raise/simulate a system-style error by number. |

## Divergences & gaps

### ❌ `trace` / `trace-highlight` / `debug` — no debugger implementation at all

```
$ niilisp -e '(trace true)'
niilisp: not a function: nil

$ niilisp -e '(trace-highlight "AAA" "BBB")'
niilisp: not a function: nil

$ niilisp -e '(debug (+ 1 2))'
niilisp: not a function: nil

$ niilisp -e '(println trace)'
nil
```

Per the CRITICAL PROBING RULE, `(println trace)` confirms this is an unbound symbol
(evaluates to `nil`), not a bug in some other sense — the symbols `trace`, `trace-highlight`,
and `debug` simply do not exist in niiLISP. A source grep of `src/` turns up no
implementation for any of them (only `catch`/`throw` are registered as special forms in
`src/eval.rs`). Consequently the interactive step-debugger described in the chapter (the
`s`/`n`/`c` stepping prompt shown when `(trace true)` is active and a traced function runs)
does not exist in any form, interactive or otherwise.

### ❌ `error-event`

```
$ niilisp -e '(error-event (lambda () (println "err!")))'
niilisp: not a function: nil
$ niilisp -e '(println error-event)'
nil
```

Unbound — no mechanism to install a custom error-handling function.

### ❌ `throw-error`

```
$ niilisp -e '(println (catch (throw-error "custom error")))'
niilisp: not a function: nil
$ niilisp -e '(println throw-error)'
nil
```

Unbound — cannot raise a user-defined error with a message that `catch` would trap as an
error string (niiLISP's `catch` does trap *runtime* errors like type mismatches, but there
is no way to manually signal an equivalent user error).

### ❌ `last-error`

```
$ niilisp -e '(catch (+ 1 "a")) (println (last-error))'
niilisp: not a function: nil
$ niilisp -e '(println last-error)'
nil
```

Unbound — after `catch` traps a runtime error there is no `last-error` call to retrieve the
`(error-number error-message)` list newLISP documents.

### ❌ `sys-error`

```
$ niilisp -e '(sys-error 1 "test")'
niilisp: not a function: nil
$ niilisp -e '(println sys-error)'
nil
```

Unbound — no way to raise/simulate a system-numbered error.

### ✅ `catch` / `throw` (working, for reference)

```
$ niilisp -e '(println (catch (throw 42)))'
42
$ niilisp -e '(println (catch (+ 1 2)))'
3
$ niilisp -e "(catch (+ 1 2) 'r) (println r)"
3
$ niilisp -e '(println (catch (+ 1 "a")))'
expected a number
```

These two are correctly implemented, including catching runtime type errors and the
`(catch expr 'result-symbol)` binding form. This is the only part of the chapter's toolkit
that niiLISP actually supports — everything specific to the trace-based debugger and the
extended error-introspection API (`error-event`, `throw-error`, `last-error`, `sys-error`)
is entirely absent.
