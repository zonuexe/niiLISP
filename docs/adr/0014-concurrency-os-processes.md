# Concurrency: faithful OS-process model (fork-based), Unix-only as in newLISP

niiLISP reproduces newLISP's concurrency as **real OS processes**, not threads. This is a v2+ direction (the concurrency tests are outside the v1 gate, ADR-0009), recorded now to keep the primitives coherent.

## The model (verified against `qa-cilk`, `qa-share`, `qa-pipefork`)

- **`fork expr`** creates a child OS process (returns a PID); `pipe` + `read-line`/`write-line` do IPC.
- **`spawn 'var expr` / `sync timeout`** — the Cilk API (CONTEXT.md: Cilk API) — is itself **built on `fork`**: each caller keeps its own list of spawned children and collects their results at `sync`.
- **`share`** (CONTEXT.md: share) is an **OS shared-memory** cell for exchanging one value between processes.
- newLISP's own `qa-cilk`/`qa-pipefork` **skip on Windows** (`fork not available`): the process model is **Unix-only in newLISP itself**.

## Why processes, not threads

1. **Compatibility (ADR-0001).** These primitives have observable process semantics (separate address spaces, PIDs, real isolation, shared-memory `share`). Threads (option C) diverge from that and break the corpus; the abstraction-split (option B) fractures `share`'s semantics across thread/process worlds.
2. **ORO is share-nothing.** Processes communicate by copying — the same philosophy as ORO (CONTEXT.md: ORO). The concurrency model and the memory model share one principle: no shared mutable Lisp heap.
3. **Feasible in Rust because the evaluator is single-threaded.** newLISP forks a single-threaded interpreter and keeps interpreting in the child. Our tree-walker is single-threaded by design (ORO, no shared heap), so `fork`-and-continue avoids the multi-threaded-`fork` unsafety that would otherwise make this hard in Rust — no background threads or held locks at fork time.

## Consequences

- Concurrency is **Unix-only**, matching newLISP exactly — this is compatibility fidelity, not a regression. Windows behaves as newLISP does (primitives unavailable / degraded).
- The single-threaded-interpreter invariant becomes a **hard constraint to preserve**: introducing background threads/async into the core runtime would break `fork` safety. If future work wants threads, it must not compromise fork-and-continue for the process model.
- `spawn`/`sync`, `fork`, `pipe`, `share`, and `semaphore` are all implemented against OS process/IPC facilities so they interoperate as newLISP programs expect.
