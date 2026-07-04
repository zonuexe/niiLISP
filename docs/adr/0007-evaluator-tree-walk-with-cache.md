# Evaluator: tree-walk over live lists, with a mutation-invalidated dispatch cache

niiLISP evaluates the **live `Vec`-backed list structure directly** (tree-walking), and layers a **derived dispatch cache** on top for speed. It does **not** compile to a separate bytecode/IR as its primary execution model.

## Why not a binary choice

The options form a continuum:

- **(a)** pure tree-walk over live lists — most compatible, slowest;
- **(a′)** tree-walk + a *derived* cache (resolved head dispatch, arity, builtin pointers) keyed by list-node identity/version, invalidated when the node is mutated — **chosen baseline**;
- **(b1)** lazy compile-to-IR with recompile-on-mutation;
- **(b2)** full ahead-of-time compile — fastest, breaks self-modifying code.

Only **self-modifying code** (destructively rewriting a live code body at runtime) is at risk from moving right. fexprs (`lambda-macro`, unevaluated args) and runtime `eval` do **not** require live-AST interpretation — they only need code-as-data, which every option keeps.

**ORO makes the cache cheap.** The usual reason self-modification defeats caching/JIT is aliasing; under ORO (CONTEXT.md: ORO) there is no aliasing — a lambda body has exactly one owner — so a per-node version stamp is enough to invalidate the derived cache on mutation without a whole-program aliasing analysis.

## Decision

Priority #2 ("practical") means **everyday-task-sufficient speed**, not raw throughput. So:

- Baseline is **(a′)**: correctness/compat of pure tree-walk, plus a derived cache that never becomes the source of truth (so self-modifying code stays correct — mutation just drops the cache).
- The cache is *derived and discardable*: the live list is always authoritative.

## Consequences

- List nodes carry a lightweight version/dirty stamp so mutation invalidates cached dispatch for that node.
- Deep newLISP recursion is bounded by an explicit max call depth with a clear error, as newLISP does, since tree-walking uses the host (Rust) stack.
- If speed is ever promoted to an explicit goal, **(b1)** — lazy compile + recompile-on-mutation, again leaning on ORO's no-aliasing — is the documented next step, additive over (a′). (b2) stays rejected while compat is priority #1.
