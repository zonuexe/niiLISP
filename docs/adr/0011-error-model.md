# Error model: non-local exit as `Result<Value, Signal>`, unified with scope/ORO unwinding

niiLISP reproduces newLISP's exception model, where `throw`, `throw-error`, and runtime errors all unwind to the nearest enclosing `catch` (verified against `qa-exception`). The semantics are fixed by compatibility (ADR-0001); the Rust decision is the **unwinding mechanism**.

The evaluator returns **`Result<Value, Signal>`**, where `Signal` is an enum distinguishing `Throw(Value)` from `Error(ErrorInfo)`. Non-local exit is ordinary `?`-propagation; `catch` is the interception point.

## Why this mechanism

As an `Err(Signal)` propagates up through `?`, each stack frame's **scope guard `Drop`s in order** — restoring the save/restore bindings (ADR-0006) and freeing values stack-wise (CONTEXT.md: ORO). So error unwinding, dynamic-scope restoration, and ORO reclaim travel **one shared unwinding path**. This is the decisive reason to thread a `Result` rather than:

- **`panic!` + `catch_unwind`** — abuses panics for normal control flow, is slower, and drags in `UnwindSafe`/poisoning concerns.
- **setjmp/longjmp** (as newLISP's C does) — unsafe in Rust and, fatally, **skips `Drop`**, which would bypass scope restoration and leak values, breaking the ORO discipline we committed to.

## Reproduced semantics (oracle: `qa-exception`)

- `(catch expr sym)`: evaluates `expr`; on normal completion `sym` ← result and it reports success; on `throw`/error `sym` ← the thrown value / error info and it reports the exceptional exit. Both outcomes funnel their value into `sym`.
- `(catch expr)`: returns the value, or the caught thrown-value / error.
- `throw`, `throw-error`, `error-event`, `last-error` follow newLISP; runtime errors enter the same path as `Signal::Error`.
- Exact edge-case behaviour is pinned by `qa-exception`, not re-specified here.

## Consequences

- Every evaluator entry point is `Result`-typed; builtins return `Result<Value, Signal>`.
- `catch` maps `Err(Signal::Throw(v))` / `Err(Signal::Error(e))` to the caught value; other control signals (if any are later added, e.g. loop `break`) extend the same `Signal` enum.
- The unified unwinding path means no separate cleanup bookkeeping for errors — correctness rides Rust's `Drop` ordering.
