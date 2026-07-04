# Value representation and copy strategy (deferred)

Status: proposed — decision deferred. Recorded to capture the analysis; no change is made in v1.

## Context

`Value` (src/value.rs) is currently a 32-byte tagged enum (largest variants
`Str(Vec<u8>)` / `List(Vec<Value>)` / `Builtin` at 24 bytes + tag). The question:
should niiLISP adopt NaN-boxing or tagged-pointer representations, and more
broadly, where is memory/throughput optimization actually worth spending?

Per ADR-0007, v1 is correctness-first and optimization waits until profiling
justifies it. This ADR records the design space so the eventual decision is not
re-derived from scratch.

## The ORO twist (why this differs from a normal VM)

Under ORO (CONTEXT.md: ORO) the usual value-representation calculus changes:

1. **The dominant cost is deep copy, not slot size.** Every list/string is
   deep-copied on store/pass (O(n)). Shrinking the per-slot size reduces copy
   *bandwidth* (a bounded ~4x for flat immediate-heavy data) but not the O(n)
   *asymptotics*. Slot size is the wrong lever to pull first.
2. **NaN-boxing's copy-cheapness is negated by ORO.** In a GC'd VM a NaN-boxed
   64-bit word is freely bit-copyable (sharing is fine) — a core reason it is
   fast. Under ORO a heap pointer cannot be bit-copied (that creates aliasing,
   violating ORO), so every copy must tag-check and deep-clone the pointee. Only
   *immediates* (nil/true/int/float/symbol) actually shrink and copy cheaper.
3. **ORO removes half the tagged-pointer motivation.** No sharing and no object
   identity (peculiarities note §2.3) means pointer-compare `eq` and interning
   wins do not apply.
4. **NaN-boxing costs safety.** It is inherently `unsafe` + manual memory: it
   would discard Rust's `Drop`-based ORO reclaim (ADR-0011's unified unwinding)
   and re-introduce newLISP-C-style manual cell management.

## Considered options (leading candidate first)

- **Copy-on-write via `Rc<…>` + `Rc::make_mut` for `List`/`Str` — leading
  candidate.** Shares reads, copies only on mutation; observably identical to
  ORO value semantics (each mutation stays isolated to its owner), in **safe**
  Rust. Turns the O(n) deep copy into O(1) sharing until write — the biggest
  asymptotic lever, and one newLISP's C cannot easily take. Extends the existing
  `Rc<Lambda>` choice (ADR-0004) to data, now with `make_mut` to preserve value
  semantics. Cost: place navigation (ADR-0006) and self write-back (ADR-0010)
  must go through `make_mut`, and nested `Rc` walking is more intricate.
- **Dispatch cache (ADR-0007 `a'`).** Orthogonal; targets eval speed, not memory.
- **Safe slot shrink.** Measured: boxing `Builtin` and using boxed slices takes
  the enum 32 → 24 bytes; thin-pointer boxing (`Box<Vec<…>>`) reaches ~16 bytes,
  at the price of an extra indirection. Keeps `Drop`; no `unsafe`.
- **Small-string optimization (SSO).** Inline short byte strings; orthogonal.
- **NaN-boxing / tagged pointers (8 bytes).** Bounded win (immediates only, per
  the ORO twist above), and it forces `unsafe` + manual memory. Not worth it as
  a piecemeal change.
- **Uniform fixed-size cell + free-list allocator (ADR-0005 option c).** Where a
  NaN-boxing-like representation actually belongs: a single, contained `unsafe`
  subsystem, adopted only if newLISP's ~250 KB / instant-start *footprint* is
  made an explicit goal.

## Decision

**Deferred.** No representation change in v1. If/when profiling makes throughput
or footprint a goal, the expected order is: measure → copy-on-write
(`Rc::make_mut`) → safe slot shrink → SSO. NaN-boxing / a uniform-cell allocator
are reserved for an explicit tiny-footprint goal and would be introduced together
as ADR-0005 option (c), not as a partial value-representation hack.

## Consequences

- The `Value` layout and the deep-copy-on-store model stay as-is for now; all
  current code (place navigation, self write-back, deep-copy semantics) is
  written against owned values and would need review before a CoW switch.
- Revisiting this is additive: CoW preserves the observable value semantics the
  rest of the interpreter relies on, so it can land without changing behaviour.
