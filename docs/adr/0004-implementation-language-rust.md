# Implementation language: Rust (Zig and Nim as runners-up)

niiLISP is implemented in **Rust**. Zig and Nim were the original front-runners and remain the fallback if Rust proves a poor fit; their trade-offs are recorded so the decision can be revisited without re-deriving it.

The only hard constraint (ADR-0001) is that the language must drive libffi for `import`. All candidates satisfy it. The deciding factors were ORO fit (ADR: ORO in CONTEXT.md) and priority #2, practicality/maintainability.

## Considered options

**Rust — chosen.**
- Pros: ORO maps almost one-to-one onto ownership (single owner), move/`clone` (deep-copy on pass), and `Drop` (stack-wise deallocation); the borrow rules make ORO's "no cyclic references" invariant a compile-time guarantee. Strongest tooling (cargo, built-in test), memory safety without GC, large ecosystem, `libffi` crate for `import`.
- Cons: globally-referenced structures (context/symbol tables, interned symbols, the Lisp object graph) fight the borrow checker; needs arena allocation / `RefCell` / targeted `unsafe`. Steeper learning curve (acceptable — learning is the lowest priority).

**Zig — runner-up (was 1st choice).**
- Pros: excellent C interop (can consume newLISP's C headers directly), comptime, manual memory suits a hand-managed ORO, smaller/simpler than Rust.
- Cons: pre-1.0 and unstable; young ecosystem; no borrow-checker-style safety net, so ORO invariants must be upheld by hand.

**Nim — runner-up (was 2nd choice).**
- Pros: compiles to C, so libffi/`dlopen` interop for `import` is trivial and low-overhead; ARC/ORC destructors + move semantics map reasonably to ORO; high-productivity syntax.
- Cons: smaller ecosystem and contributor pool than Rust; ARC/ORC and default-GC nuances to reason about; fewer guarantees than Rust's type system.

**C — not chosen.**
- Pros: mirrors newLISP directly, easiest to transcribe the reference source (ADR-0003).
- Cons: weakest safety and maintainability, which are exactly priority #2.

**MoonBit — considered and declined (2026-07).**
- Pros: modern ADT/pattern-matching language; WASM/JS/native/LLVM backends; native backend lowers to a C subset so *static* C interop is smooth and fast.
- Cons, on exactly the axes that decided this ADR: (1) **ORO fit** — MoonBit is GC'd (reference counting on native/C, runtime GC on WASM-GC/JS), so the ORO↔ownership 1:1 mapping and the borrow-checker's compile-time "no cycles" guarantee do **not** transfer; ORO would be a semantic layered over a GC host. The `Drop`-guard + `?`-propagation unification in ADR-0006/0007/0011/0013 is Rust-specific. (2) **`import` (headline goal, ADR-0001)** — newLISP `import` needs *runtime* dlopen + resolve-by-name + libffi; MoonBit's C FFI is *compile-time* `extern "C"` + build-time linking, with no documented dynamic-loading/libffi path (Rust has mature `libffi` + `libloading`). (3) **Maturity** — v0.x, small ecosystem, against priority #2.
- MoonBit's headline strength (WASM-first) is orthogonal to our #1 goal, since native shared-library `import` cannot run in a WASM sandbox at all.

## Consequences

- Expect an arena/`RefCell`-based design for the symbol table and context namespaces; this will likely warrant its own ADR once the value representation is settled.
- If Rust's friction on the Lisp object graph proves worse than expected, Zig is the documented fallback.
- **Flip condition for MoonBit:** if a WASM/browser niiLISP target (à la the reference's `newlisp-js/`) becomes an explicit goal, MoonBit deserves reconsideration for *that* target — where `import` is out of scope anyway, so its main con no longer applies. Absent that goal, Rust stands.
