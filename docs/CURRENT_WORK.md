# Current Work

A living handoff + backlog for niiLISP. Records what state the project is in and
what is deliberately deferred, so work can resume without re-deriving context.

## Status

- **v0.1.0 released** (2026-07-04): on [crates.io](https://crates.io/crates/niilisp)
  (`cargo install niilisp`) and as GitHub Release binaries for 5 platforms.
- Working: reader, tree-walking evaluator, dynamic scope + contexts, ORO value
  semantics, FOOP with reference `self`, the reference/place model, `catch`/`throw`,
  and a core builtin set. Passes vendored `qa-exception` and `qa-foop`.
- Since v0.1.0 (all on `master`, unreleased): **FFI `import`** (typed C calls,
  ADR-0018/0019) and **`callback`** (ADR-0020); `case`/`if-not` promoted to full
  special forms; a language spec under [`docs/spec/`](spec/) (syntax, types,
  special-forms, functions).
- Tests: 31 unit + 3 integration (`qa-exception`, `qa-foop`, hermetic `ffi`).

## Next task — pick up here: FFI memory API slice

**Design is done ([ADR-0021](adr/0021-ffi-memory-api-slice.md)); implement it.**
This is the third FFI slice; acceptance target is the vendored `qa-nullstring`.
Build on the existing FFI module (`src/ffi.rs`, gated `cfg(all(feature = "ffi",
unix))`), which already has `CType`, the `import` marshalling, and `callback`.

Implement these builtins (all `cfg(all(feature = "ffi", unix))`):

- **`struct`** — `(struct 'name t…)` binds `name` to a list of C type names
  (reuse `CType`; a struct is just a list of type strings, no new value type).
- **`pack` / `unpack`** — `(pack layout val…)` -> a binary string; `(unpack
  layout str)` -> a list. Use the **native C ABI layout** (natural alignment +
  padding + native endianness), so packed bytes match a real C struct. Compute
  per-field offsets from each `CType`'s size/align.
- **`get-string` / `get-int` / `get-long` / `get-float` / `get-char`** — read a C
  value at an integer address. **Check address 0 -> error** ("cannot convert NULL
  to string"); other invalid addresses are UB (accepted, ADR-0015).
- **`address`** — `(address 'sym)` returns the stable buffer address of a
  **symbol-held** value. Reject `address` of an arbitrary temporary (dangles
  under ORO). Invariant: caller must not resize/reassign while C holds it.
- Extend `import`'s **`void*` argument to accept a string** and pass its buffer
  pointer directly (no copy, binary-safe), valid for the call's duration — this
  is how a packed struct is handed to C.

Then: a hermetic test (pack a struct, pass via `void*`, read back), and wire
`qa-nullstring` into `tests/qa.rs` once it passes (it also needs `struct`/`pack`/
`unpack`/`get-string` — check what else it references).

**Gotcha (cost time this session):** cargo's incremental build went stale and
silently reused an old binary, masking a compile error. If a change seems to have
no effect, run `cargo clean -p niilisp` and confirm you see `Compiling niilisp`.

## Deferred optimizations (do after the language surface is more complete)

These are performance/footprint items intentionally postponed until more of the
language is implemented, per ADR-0007 (correctness-first, measure-then-optimize).
None change observable behaviour.

- **Evaluator dispatch & call path** ([ADR-0017](adr/0017-evaluator-dispatch-and-call-path.md)) —
  the biggest throughput lever for recursion-heavy code (`tak`/`tarai`). Replace
  name-string special-form dispatch with SymId-based dispatch (A), a dense
  `Vec<Value>` symbol table (B), then the ADR-0007 dispatch cache (C). Baseline to
  beat: `(tak 24 16 8)` ~0.80s (release).
- **Value representation & copy strategy** ([ADR-0016](adr/0016-value-representation-and-copy-strategy.md)) —
  for allocation-heavy code. Copy-on-write via `Rc::make_mut` for `List`/`Str` is
  the leading candidate (ORO-preserving, safe); then safe slot-shrink and SSO.
  NaN-boxing / uniform-cell allocator only if a tiny-footprint goal is set.
- **Dispatch cache** ([ADR-0007](adr/0007-evaluator-tree-walk-with-cache.md) `a'`) —
  the sanctioned next step once (A)/(B) above are in.

## Next feature work (roadmap)

Ordered roughly by dependency. See the corrected acceptance strategy in
[ADR-0009](adr/0009-v1-acceptance-corpus.md): the `qa-specific-tests` are
integration targets unlocked as their dependencies land.

- **qa-ref tail** — remaining reference-model features: context-as-hash,
  string-byte places (`(setf (s 3) "D")`), `eval`/loop place-returns,
  `upper-case`, `dostring`.
- **`import` / FFI** (v2 headline, [ADR-0015](adr/0015-import-ffi.md)) — **done**:
  typed `import` ([ADR-0019](adr/0019-ffi-first-slice.md)) and `callback`
  ([ADR-0020](adr/0020-ffi-callback-slice.md)), Unix + system libffi
  ([ADR-0018](adr/0018-ffi-build-and-packaging.md)). **Next: the memory API slice**
  — designed in [ADR-0021](adr/0021-ffi-memory-api-slice.md), see the handoff at
  the top. Later FFI slices: the terse `pack` format-char language, simple/untyped
  `import`, and Windows FFI. `qa-libffi` additionally needs `exec`/`real-path`.
- **bigint** — `L` literals + `Value::Bigint`; unlocks `qa-bigint`, `qa-longnum`,
  and the tail of `qa-factorfibo`.
- **Contexts as namespaces/dictionaries** — beyond FOOP; unlocks `qa-dictionary`.
- **Full UTF-8 character operations** ([ADR-0013](adr/0013-string-representation-and-unicode.md)) —
  `utf8len`, char indexing/slicing; unlocks `qa-utf8*`.
- **Self-modifying code + destructive place builtins** — the deepest ORO tests
  (`qa-inplace`); would also motivate the `Rc<Lambda>` copy-on-write noted in the
  retrospective.
- **Networking** — `net-*`; unlocks `qa-lookup6`.

## Release pipeline TODO (before the next tag)

`release.yml` builds prebuilt binaries for 5 platforms with **default features**
(now including `ffi`). Since FFI uses the *system* libffi and is Unix-only
(ADR-0018), the current release matrix would fail on Windows and on cross-compiled
`aarch64-linux`. Before tagging v0.2.0, decide per target: build the prebuilt
binaries with `--no-default-features` (simplest, drops FFI from downloads but
`cargo install` still gets it), or install/cross-provide libffi per target.

## Releasing

Follow the `niilisp-release-prep` skill (`.claude/skills/`): bump version, seal
the `[Unreleased]` changelog, reconcile the README, verify, and push a `vX.Y.Z`
tag (after a human Go — publishing is irreversible).
