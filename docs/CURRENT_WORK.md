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
  (`struct`/`pack`/`unpack`/`get-*`/`address`, ADR-0021), **bigint**
  (arbitrary-precision integers, ADR-0022), **arrays** (fixed-length,
  ADR-0023), **copy-on-write values** (`Rc`/`make_mut`, ADR-0024),
  **UTF-8 character operations** (ADR-0025), **contexts as switchable
  namespaces** (`context`/`dotree`/`term`, ADR-0026), and **lambdas as list
  data** (`expand`/`args` + callable lambda-lists, ADR-0027); `case`/`if-not`
  promoted to full special forms; a language spec under [`docs/spec/`](spec/)
  (syntax, types, special-forms, functions).
- Tests: 61 unit + 9 integration (`qa-exception`, `qa-foop`, `qa-nullstring`,
  `qa-bigint`, `qa-longnum`, `qa-utf8`, `qa-factorfibo` [`#[ignore]`d — slow
  sieve], and two hermetic `ffi` tests); the suite passes under both default
  features and `--no-default-features`.
- Standard-library fill-ins (byte-based, no UTF-8 dependency): string builtins
  `upper-case`/`lower-case`/`trim`/`slice`/`find`/`explode`/`chop`, the RNG
  (`seed`/`rand`/`random`/`amb`), `main-args`, the list/number builtins
  `min`/`max`/`even?`/`odd?`/`flat`/`join`/`member`/`unique`/`true?`, and the
  `dostring`/`until`/`extend`/`swap` special forms.

## Next task — pick up here: cut the next release, then choose a slice

The lambda-calculus gist milestone is **done** (ADR-0027) — a good point to cut a
release. Use the `niilisp-release-prep` skill: bump the crate version, seal the
`[Unreleased]` changelog, reconcile the README, verify, tag. **Before tagging, do
the release-pipeline TODO below** (the `ffi` default feature vs the cross-compile
matrix). After the release, candidates for the next slice, roughly by value:

- **UTF-8 follow-ups** (the other half of the string arc): Unicode case folding
  for `upper-case`/`lower-case` (currently ASCII), char-based `trim`, and
  `regex` over UTF-8 (the `qa-utf8-*regex*` oracles). Regex is a big sub-feature
  and wants its own grilled ADR; the case/trim upgrades are a smaller library
  pass.
- **Dictionary API + persistence** — `(Dict key)` / `(Dict assoc)` / `(Dict)`
  over contexts, plus `save`/`load`/`delete`/`sys-info`/`randomize`/file I/O.
  Unlocks `qa-dictionary`. Its own grilled ADR (file I/O is the big piece).
- **qa-ref tail** — string-byte places (`(setf (s 3) "D")`), `eval`/loop
  place-returns. Touches the place model; scope first.

The evaluator dispatch optimisation ([ADR-0017](adr/0017-evaluator-dispatch-and-call-path.md))
stays intentionally **deferred** (premature optimisation).

**Known limitation of contexts (ADR-0026):** a runtime-defined MAIN symbol
referenced bare from inside a context is mis-qualified (the reader knows only the
static primitive set, not runtime MAIN definitions). No current target hits this;
perfect fidelity would need read/eval interleaving.

Note the RNG distribution for `(random offset scale)` is **uniform**, not
newLISP's; fine for `qa-bigint` (invariant-based) but revisit if a future script
depends on the exact distribution.

## Done since v0.1.0

**Lambdas as list data** ([ADR-0027](adr/0027-lambda-as-list-hybrid.md)) — the
release milestone: run the lambda-calculus gist
(<https://gist.github.com/kosh04/262332>). A lambda presents newLISP's list
interface (compact `Value::Lambda`/`Fexpr` kept as the stored/called form): empty
`(lambda)` is the list `(lambda)`, `append` builds a lambda as data, a
lambda-headed list is callable, and lambdas print as `(lambda (p…) body…)`. Added
`expand` (upper-case auto-expand restricted to code-like values) and `args`, and
made special forms aliasable as values (`(define DEFINE define)`). The gist's
Church numerals evaluate correctly (`ZERO`..`THREE`, `PLUS`, `MULT`); `POW`/`SUCC`
hit the gist's own documented reused-variable hazard (no lexical binding).
Deferred: full list identity for lambdas and in-place `setf` into a body.

**Contexts as switchable namespaces** ([ADR-0026](adr/0026-contexts-as-namespaces.md)):
the reader tracks a current context set by top-level `(context 'X)` and qualifies
bare symbols as `X:sym` (except MAIN primitives) — a read-time effect. `dotree`
iterates a context's symbols, `term` strips the prefix. Mostly reader work; the
evaluator's `lookup`/`set` were unchanged (qualified symbols are flat-`globals`
keys). **`qa-utf8` passes** and is wired in. Deferred: the dictionary API and
persistence (`qa-dictionary` stays gated).

**UTF-8 character operations** ([ADR-0025](adr/0025-utf8-character-operations.md)):
a lenient decode layer (`src/utf8.rs`) over the binary-safe byte storage; `utf8len`
and character-based `nth`/`(str i)`/`first`/`rest`/`last`/`explode` (multi-byte
characters stay whole), while `slice`/`(i str)`/`length`/search stay byte-based.
Adding a string to the functor-position indexing arm also fixed a pre-existing gap
(strings didn't self-index at all). Deferred: Unicode case folding, char `trim`,
regex. `qa-utf8*` stays gated on contexts (`dotree`) and regex.

**Copy-on-write values** ([ADR-0024](adr/0024-copy-on-write-values.md)):
`List`/`Array`/`Str` are `Rc`-wrapped and cloned only on write (`Rc::make_mut`).
Store/pass of a large container is O(1); a write to one owner never affects
another (isolation tests + the ORO suite confirm it). Removes the clone-on-read
O(n²) — a 100k sieve went ~42s → ~0.13s — with `(tak 24 16 8)` unchanged (~0.82s).
Also fixed `(apply f nil)` to treat `nil` as the empty list.

**arrays** ([ADR-0023](adr/0023-array-value-type.md)): `Value::Array` — a
fixed-length, list-like value (`array?` true / `list?` nil, prints like a list),
built with `(array size [init])` (cycle/nil fill), converted by `array-list`.
`setf`-element works, `push`/`pop`/`extend` error. Added `true?`. Corrected
implicit indexing: a number in functor position is rest/slice (`(2 lst)` → tail
from 2), matching newLISP; element access is `(lst i)`. `qa-factorfibo` now
passes (wired `#[ignore]`d — its 1M-element sieve is ~10s in debug).

**bigint** ([ADR-0022](adr/0022-bigint-numeric-tower-slice.md)): `Value::Bigint`
behind a default-on `bigint` feature over `num-bigint`; over-long / `L`-suffixed
decimal literals promote; `+ - * / %` compute in arbitrary precision when an
operand is a bigint (floats truncated, no auto-demote); cross-type compare,
`zero?`/`abs`/`length`(digit count), `bigint`/`int`/`float`, and `gcd`. The
variant is `#[cfg]`-gated. `qa-bigint` / `qa-longnum` pass (wired), with their
RNG (`seed`/`rand`/`random`/`amb`) and `until`/`extend`/`explode`/`chop`/
`main-args` helpers.

**FFI** ([ADR-0018–0021](adr/0021-ffi-memory-api-slice.md)): typed `import`,
`callback`, and the memory API (`struct`/`pack`/`unpack` incl. the format-string
mini-language, `get-*`, `address`, string-to-`void*`). `qa-nullstring` passes.
Remaining FFI slices: simple/untyped `import`, Windows FFI, and `address` of
scalars / write-through (needs a value-model decision).

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
  copy-on-write via `Rc::make_mut` for `List`/`Array`/`Str` is **done**
  ([ADR-0024](adr/0024-copy-on-write-values.md)). Still deferred from ADR-0016:
  safe slot-shrink and SSO; NaN-boxing / uniform-cell allocator only if a
  tiny-footprint goal is set.
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
