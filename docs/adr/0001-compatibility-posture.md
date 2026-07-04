# Compatibility posture: newLISP source + `import`/FFI, not native ABI

niiLISP is a re-implementation of newLISP whose priorities, in order, are: **(1) compatibility with existing newLISP assets, (2) practicality, (3) learning/experimentation**. When these conflict, compatibility wins.

Concretely, the compatibility target is **source-level compatibility for the subset of newLISP actually used, plus `import`/FFI compatibility** — newLISP modules that are `.lsp` files calling shared libraries via `import` must load and run. We are **not** targeting newLISP's native C-API/cell-struct ABI: native C extensions linking against `newlisp.h` are out of scope until a real need is demonstrated.

Why: the dominant real-world form of a newLISP "module" is a `.lsp` file built on `import`, so `import` compatibility recovers most of the value of "C-module compatibility" at a fraction of the cost. Reproducing the native ABI would force the implementation language to C and force the internal cell representation to mirror newLISP's, which would sacrifice priority (2) and (3) for a small set of true native extensions.

## Consequences

- The implementation language is not forced to be C — but it must be able to drive libffi (or an equivalent) to implement `import`. This keeps a modern systems language (Rust/Zig/C/C++) on the table; it rules out a runtime that cannot do C FFI cheaply.
- Internal value representation is free to diverge from newLISP's cell layout, since no native extension will observe it.
- If native-ABI compatibility is ever needed, it is a new, separate decision (superseding ADR) — not an incremental tweak.
