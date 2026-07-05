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
  ADR-0018/0019), **`callback`** (ADR-0020), the **FFI memory API**
  (`struct`/`pack`/`unpack`/`get-*`/`address`, ADR-0021), and **bigint**
  (arbitrary-precision integers, ADR-0022); `case`/`if-not` promoted to full
  special forms; a language spec under [`docs/spec/`](spec/) (syntax, types,
  special-forms, functions).
- Tests: 50 unit + 7 integration (`qa-exception`, `qa-foop`, `qa-nullstring`,
  `qa-bigint`, `qa-longnum`, and two hermetic `ffi` tests); the suite passes
  under both default features and `--no-default-features`.
- Standard-library fill-ins (byte-based, no UTF-8 dependency): string builtins
  `upper-case`/`lower-case`/`trim`/`slice`/`find`/`explode`/`chop`, the RNG
  (`seed`/`rand`/`random`/`amb`), `main-args`, the list/number builtins
  `min`/`max`/`even?`/`odd?`/`flat`/`join`/`member`/`unique`, and the
  `dostring`/`until`/`extend`/`swap` special forms.

## Next task — pick up here: choose the next slice

The FFI and bigint arcs are complete and their qa targets pass. Pick the next
piece; the roadmap below has the full list. Leading candidates:

- **`qa-factorfibo` tail** — uses the bigint machinery already in place, but is
  blocked on newLISP **arrays**: it calls `array` / `array-list` (its `primes`
  sieve is array-based). Arrays are a distinct newLISP type (own `array?`, print
  syntax, fixed size), so this **wants a grilled ADR** first — decide whether to
  add a `Value::Array` variant or alias arrays to lists. (`swap`, the other gap,
  is now implemented.)
- **qa-ref tail** — context-as-hash, string-byte places (`(setf (s 3) "D")`),
  and `eval`/loop place-returns. Touches the place model; scope it first.
- **Contexts as namespaces/dictionaries** or **UTF-8 char ops**
  ([ADR-0013](adr/0013-string-representation-and-unicode.md)) — each wants its
  own grilled ADR before implementation.

Note the RNG distribution for `(random offset scale)` is **uniform**, not
newLISP's; fine for `qa-bigint` (invariant-based) but revisit if a future script
depends on the exact distribution.

## Done since v0.1.0

**qa-bigint / qa-longnum helpers**: a seedable xorshift RNG (`seed`/`rand`/
`random`/`amb`), the `until` loop, `extend` (destructive place append),
`explode`/`chop`, and `main-args`. `qa-bigint` (scaled down via `(main-args -1)`
to keep the test fast) and `qa-longnum` are wired into `tests/qa.rs`.

**bigint** ([ADR-0022](adr/0022-bigint-numeric-tower-slice.md)): `Value::Bigint`
behind a default-on `bigint` feature over `num-bigint`; over-long / `L`-suffixed
decimal literals promote; `+ - * / %` compute in arbitrary precision when an
operand is a bigint (floats truncated, no auto-demote); cross-type compare,
`zero?`/`abs`/`length`(digit count), `bigint`/`int`/`float` conversions, and
`gcd`. The variant is `#[cfg]`-gated so `--no-default-features` compiles it out
and the literals revert to a read error.

The **FFI memory API slice is done** ([ADR-0021](adr/0021-ffi-memory-api-slice.md)):
`struct`, `pack`/`unpack` (native C ABI layout), `get-string`/`get-int`/
`get-long`/`get-float` (a C double)/`get-char`, and `address` (symbol-held
strings only), plus `import`'s `void*` argument now accepts a string (passes its

**bigint** ([ADR-0022](adr/0022-bigint-numeric-tower-slice.md)): `Value::Bigint`
behind a default-on `bigint` feature over `num-bigint`; over-long / `L`-suffixed
decimal literals promote; `+ - * / %` compute in arbitrary precision when an
operand is a bigint (floats truncated, no auto-demote); cross-type compare,
`zero?`/`abs`/`length`(digit count), `bigint`/`int`/`float` conversions, and
`gcd`. The variant is `#[cfg]`-gated so `--no-default-features` compiles it out
and the literals revert to a read error.

The **FFI memory API slice is done** ([ADR-0021](adr/0021-ffi-memory-api-slice.md)):
`struct`, `pack`/`unpack` (native C ABI layout), `get-string`/`get-int`/
`get-long`/`get-float` (a C double)/`get-char`, and `address` (symbol-held
strings only), plus `import`'s `void*` argument now accepts a string (passes its

The **FFI memory API slice is done** ([ADR-0021](adr/0021-ffi-memory-api-slice.md)):
`struct`, `pack`/`unpack` (native C ABI layout), `get-string`/`get-int`/
`get-long`/`get-float` (a C double)/`get-char`, and `address` (symbol-held
strings only), plus `import`'s `void*` argument now accepts a string (passes its
buffer pointer, no copy). NULL through `unpack`/`get-string` errors instead of
crashing. `qa-nullstring` passes and is wired into `tests/qa.rs`. The terse
**`pack`/`unpack` format-string mini-language** (`c b d u ld lu Ld Lu f lf sN nN`
with `>`/`<` endian toggles, packed tightly) is also in — the ADR-0021 deferral
is closed.

Other slices on deck (bigint is the active task, above):

- **bigint** — **done** ([ADR-0022](adr/0022-bigint-numeric-tower-slice.md)),
  and `qa-bigint` / `qa-longnum` now pass and are wired into `tests/qa.rs` (their
  RNG / string helpers landed too). `qa-factorfibo`'s bigint tail still needs a
  `primes` sieve builtin.
- **`address` of scalars / write-through** — `address` today only exposes a
  symbol-held *string* buffer. Symbol-held numbers have no separate buffer under
  the current value model; revisit if a test needs write-through to a scalar.
- **simple/untyped `import`** and **Windows FFI** (ADR-0018) — later.

**Gotcha (cost time before):** cargo's incremental build can go stale and
silently reuse an old binary, masking a compile error. If a change seems to have
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

- **qa-ref tail** — `upper-case`/`lower-case`/`trim`/`slice`/`find` and
  `dostring` are **done**. Remaining reference-model features: context-as-hash,
  string-byte places (`(setf (s 3) "D")`), and `eval`/loop place-returns.
- **`import` / FFI** (v2 headline, [ADR-0015](adr/0015-import-ffi.md)) — **done**:
  typed `import` ([ADR-0019](adr/0019-ffi-first-slice.md)), `callback`
  ([ADR-0020](adr/0020-ffi-callback-slice.md)), and the memory API
  ([ADR-0021](adr/0021-ffi-memory-api-slice.md): `struct`/`pack`/`unpack`/`get-*`/
  `address`), Unix + system libffi ([ADR-0018](adr/0018-ffi-build-and-packaging.md)).
  Remaining FFI slices: simple/untyped `import` and Windows FFI. `qa-libffi`
  additionally needs `exec`/`real-path`.
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
