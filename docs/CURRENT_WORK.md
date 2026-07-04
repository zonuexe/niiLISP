# Current Work

A living handoff + backlog for niiLISP. Records what state the project is in and
what is deliberately deferred, so work can resume without re-deriving context.

## Status

- **v0.1.0 released** (2026-07-04): on [crates.io](https://crates.io/crates/niilisp)
  (`cargo install niilisp`) and as GitHub Release binaries for 5 platforms.
- Working: reader, tree-walking evaluator, dynamic scope + contexts, ORO value
  semantics, FOOP with reference `self`, the reference/place model, `catch`/`throw`,
  and a core builtin set. Passes vendored `qa-exception` and `qa-foop`.
- Tests: 30 unit + 2 integration.

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
- **`import` / FFI** (v2 headline, [ADR-0015](adr/0015-import-ffi.md)) — first slice
  **done**: typed `import` of scalar/string/pointer C functions (Unix, system
  libffi; [ADR-0018](adr/0018-ffi-build-and-packaging.md)/[0019](adr/0019-ffi-first-slice.md)).
  Next FFI slices: `callback` (libffi closures), the memory API
  (`pack`/`unpack`/`get-*`/`struct`), simple/untyped `import`, and Windows FFI.
  `qa-libffi`/`qa-nullstring` also need `exec`/`real-path` and the memory API.
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
