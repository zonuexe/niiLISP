# Dynamic scope via per-symbol value slots + a save/restore rebinding stack

niiLISP reproduces newLISP's dynamic scoping (see CONTEXT.md: Dynamic scoping) with its actual mechanism, not an environment-chain interpreter:

- Each symbol owns **one "current value" slot**, held in its Context's symbol table.
- Establishing a binding (lambda parameters, `let`, `local`) **saves `(symbol, old value)` onto a rebinding stack, installs the new value, evaluates, then restores** the old value as evaluation unwinds.
- Symbol reference is therefore an **O(1)** slot read, and a callee transparently sees the caller's bindings for anything it doesn't itself rebind — dynamic scope falls out for free.

In Rust the restore step is a **`Drop`-based scope guard**: the guard owns the saved bindings and restores them in its destructor, so the correct value is reinstated even on early return or panic. This mirrors ORO's stack-wise reclaim, so scope restoration and memory reclaim share one unwinding discipline.

## Considered options

- **Slots + rebinding stack (chosen):** faithful to newLISP, O(1) lookup, unwind-safe via `Drop`. Requires interior mutability (`RefCell`/arena) on the symbol table.
- **Environment-frame stack searched on lookup:** avoids global mutable slots but makes lookup O(scope depth) and diverges from newLISP's mechanism, which would risk subtle incompatibilities in fexpr/`args`-style code that inspects bindings.

## Consequences

- The Context symbol table needs interior mutability; this is the arena/`RefCell` design flagged in ADR-0004.
- A `SymId` interner backs the slots so lookups compare integers, not strings.
- Contexts are values too, so context-qualified access (`Ctx:sym`) resolves to a slot in the named context's table.
