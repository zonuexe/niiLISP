# Evaluator dispatch and call-path optimization (deferred)

Status: proposed — decision deferred. Recorded to capture the analysis; no change is made now.

## Context

Recursion-heavy, allocation-free benchmarks (Takeuchi's `tak` / `tarai`) isolate
the interpreter's **call, dispatch, and scope** overhead — they allocate nothing
(integers are `Copy` immediates; the function value is `Rc`-shared), so the
value-model options of ADR-0016 (NaN-boxing, copy-on-write) do **not** apply.

Baseline (release build): `(tak 24 16 8)` = 9 in ~0.80s.

This ADR records where the time goes and the optimization ideas, and defers the
work: the special-form / builtin surface is still growing, and the largest idea
(A) is a dispatch refactor best done once, after the language surface stabilises
(ADR-0007: correctness-first, measure-then-optimize).

## Hot path (src/eval.rs)

For every symbol-headed `eval_list` — i.e. every `(if …)`, `(< y x)`, `(- x 1)`,
`(tak …)`:

1. **`sym_name(id)` allocates a `String`** (`.to_string()`) on every call, then
   `try_special_form` matches it against ~44 names — even for ordinary calls like
   `tak` / `<` / `-` that are not special forms. Millions of String allocations
   and name matches for a single `tak` run.
2. **Dynamic-scope bind and lookup go through `HashMap<SymId, Value>`** — a hash
   per parameter bind (3 per `tak` call) and per variable reference (`x`/`y`/`z`).
3. **`eval_args` allocates a `Vec`** per call.

## Considered options (ranked by expected impact)

- **A. SymId-based special-form dispatch — largest win.** Stop matching head
  symbols by name string. Represent special forms as values in the symbol table
  (e.g. a `Value::Special` variant) or a `SymId → handler` table, so `eval_list`
  resolves the head with one lookup and dispatches on the value's kind (special →
  unevaluated args; builtin/lambda → evaluated). Removes the per-node `String`
  allocation and the ~44-arm name match entirely.
- **B. Dense `Vec<Value>` symbol table indexed by `SymId`.** SymIds are small and
  dense, so an array replaces the `HashMap`: lookup and bind become array indexing
  with no hashing. Pairs naturally with A; big for `tak`'s heavy variable
  reference and parameter binding.
- **C. Dispatch cache (ADR-0007 `a'`).** Memoize the resolved head (`tak` → the
  Lambda, `<` → the Builtin) per call-site list node; ORO's no-aliasing makes
  invalidation cheap. Layered on top of A/B.
- **D. Fuse `eval_args` into parameter binding.** For lambda calls, evaluate each
  argument directly into its parameter slot, avoiding the per-call `Vec` (or use a
  `SmallVec` / stack array for fixed small arity).
- **E. Minor.** Fast paths for 2-argument integer `<` / `-` / arithmetic.

## Does not help these benchmarks

- **NaN-boxing / copy-on-write (ADR-0016):** `tak`/`tarai` allocate nothing.
- **Tail-call optimization:** `tak`/`tarai` are not tail-recursive (nested calls
  in argument position); newLISP has no TCO either, so it is out of scope for
  compatibility.

## Decision

**Deferred.** Do this after the language surface (special forms, builtins) is more
complete, so the A dispatch refactor is done once against a stable set. Expected
order when resumed: land A + B together, measure against the recorded baseline,
then add C. All of A/B/C/D are safe Rust and observably behaviour-preserving.

## Consequences

- The name-based dispatch and `HashMap` scope stay as-is in v1; they are the first
  things to revisit when throughput becomes a goal.
- Because A/B/C/D preserve observable semantics, they can be introduced later
  without changing behaviour or the test suite's expectations.
- Tracked as a future task in `docs/CURRENT_WORK.md`.
