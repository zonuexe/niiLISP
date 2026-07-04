# Core value: Vec-backed lists, not cons cells

niiLISP's list value is backed by a growable array (`Vec<Value>` in Rust), not a chain of cons cells and not a replica of newLISP's fixed-size uniform cell.

Two facts make this a clean fit rather than a compromise:

1. **ORO deep-copies on every store/pass (see CONTEXT.md: ORO).** So the classic cons-cell advantage — O(1) prepend/`rest` via shared tails — is unavailable anyway; there is no sharing to exploit. A `Vec` pays the same copy cost ORO already mandates, while adding O(1) indexed access and cache locality.
2. **newLISP has no true dotted pairs** — a 2-argument `cons` whose second argument is not a list behaves like `list`. Nothing in the target dialect requires the `(a . b)` cell, so an array-backed list is semantically sufficient.

## Considered options

- **`Vec<Value>` (chosen):** idiomatic Rust, O(1) indexing (matches newLISP's heavy use of implicit indexing), cache-friendly. Con: mid-list insert/remove is O(n) — acceptable under a copy-everything model.
- **Box-based cons cells:** faithful to newLISP's internal structure, O(1) `cons`/`rest` — but that win evaporates under ORO, and linked nodes are awkward and allocation-heavy in Rust.
- **newLISP-style uniform cell + free-list allocator:** would also reproduce newLISP's ~250–300 KB footprint and instant stack-wise reclaim. Deferred: only worth it if the tiny-binary / startup-latency profile becomes an explicit goal, and then as its own ADR.

## Consequences

- All list builtins are written against an array, so `cons`, `rest`, `push`, `pop`, and implicit indexing are array operations.
- If the small-footprint performance profile is later made a goal, revisiting the allocator (option c) is a separate, additive decision — it does not invalidate the `Vec` interface.
