# Contexts as switchable namespaces: read-time context, dotree, term

Extends contexts (CONTEXT.md: Context) from the FOOP-only substrate they are
today into switchable **namespaces**: `(context 'X)` makes `X` the current
namespace so unqualified symbols are created in it, and `dotree`/`term` iterate
and deconstruct a context's symbols. Acceptance target: the tail of `qa-utf8`
(its `(context 'L) … (context MAIN) (dotree (l L) (println (term l) ":" (eval l)))`).
Design was grilled before writing.

## What already exists

A context is a `Value::Context(SymId)` marker plus a **naming convention**:
context-qualified symbols `Ctx:member` are ordinary interned names in the single
flat `globals` table. FOOP (colon dispatch, default functor, `self` write-back)
is built on this. What is missing is the *current-context* switch — the thing
that makes a bare `Albanian` mean `L:Albanian`.

## Read-time context (symbol identity is fixed when read)

`(context 'X)` is a **read-time** (translation-time) effect, as in newLISP: a
symbol's context is fixed when it is read, not resolved at evaluation time.

- **Chosen: read-time.** The reader tracks a current context and qualifies bare
  symbols as it reads, so a symbol defined in `X` keeps its `X:` identity even
  when the body later runs under a different current context. This is the
  faithful model (the same reason ADR-0013 fixed the string representation at
  read time).
- **Rejected: eval-time** (resolve bare symbols against the current context when
  evaluated). Localised, but a function defined in `L` and called from `MAIN`
  would resolve its body's bare symbols in `MAIN` — a compatibility trap. Passing
  one data-only test is not worth an unfaithful core.

## Mechanics over read-all-then-eval

niiLISP reads all top-level forms, then evaluates. To get read-time context
without interleaving read and eval:

- The **reader recognises top-level `(context …)` forms** and switches its own
  current context (from `'X` quoted or `X` bare); it does not wait for eval.
- While the current context is `X` (≠ `MAIN`), a bare symbol is interned as
  **`X:sym` unless its name is a known MAIN primitive** (a builtin or special
  form). The reader is given that static primitive-name set (`Interp::new`'s
  registered builtins plus the special-form names). So `set`/`println`/`quote`
  stay `MAIN`, while data symbols like `Albanian` become `L:Albanian`.
- `(context …)` is **also a runtime special form**: it registers
  `Value::Context(X)` (creating the context if new) so `X` evaluates to its
  context value and `dotree`/`(X …)` work, and it returns that context.

**Known limitation (recorded):** a symbol defined in `MAIN` *at runtime* before a
context, then referenced *bare* from inside that context, is mis-qualified — the
reader only knows the static primitive set, not runtime `MAIN` definitions.
Perfect fidelity needs read/eval interleaving; `qa-utf8` does not hit this (its
contexts hold only data and reference only static builtins). Revisit if a target
needs it.

## New surface

- **`context`** — `(context 'X)` / `(context X)`: switch the current context
  (read-time, and register/return the runtime context). Top-level only.
- **`dotree`** — `(dotree (var ctx [bool]) body)`: bind `var` to each symbol of
  `ctx` (names starting `ctx:`), in name order (deterministic); an optional true
  `bool` skips `_`-prefixed terms. Needs interner enumeration by context prefix.
- **`term`** — `(term sym)`: the symbol's term (the part after the last `:`) as a
  symbol; `(term 'L:Albanian)` → `Albanian`.

The evaluator otherwise needs no change: qualified data symbols are already
ordinary keys in the flat `globals`, so existing `lookup`/`set` handle them.

## Scope

- **In:** the reader's current-context tracking and qualification; the `context`
  special form; `dotree`; `term`. Target: `qa-utf8` runs end to end (its char
  operations are already in, ADR-0025) and is wired into `tests/qa.rs`.
- **Deferred (qa-dictionary stays gated):** the dictionary API — `(Dict key)`
  value lookup, `(Dict assoc-list)` construction, `(Dict)` all-keys — and the
  persistence/introspection it also needs (`save`/`load`/`delete`/`sys-info`/
  `randomize`/file I/O). These are a separate slice.

## Consequences

- No new value representation: contexts stay a naming convention over the flat
  `globals` plus the `Value::Context` marker; the new state is the reader's
  current context and the interp's static primitive set.
- The reader gains a dependency on the interpreter's primitive-name set (computed
  once at `Interp::new`), passed in when reading a source.
- The interner needs an enumeration-by-context-prefix method for `dotree`.
- The dictionary API and persistence remain deferred; `qa-dictionary` is gated on
  them, not on this slice.
