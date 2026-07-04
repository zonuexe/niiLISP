# v1 acceptance corpus: the pure-language slice of `qa-specific-tests`

Resolves the open item in ADR-0008. The v1 acceptance corpus is the **pure-language (no I/O, no module, no FFI) slice of newLISP's own `qa-specific-tests`**, vendored under `references/newlisp/qa-specific-tests`. It is bundled, GPLv3 (ADR-0002), and assertion-based, so pass/fail is objective and runnable from day one. No personal, manual-example, or community-script corpus is included in v1.

## In v1 (the pass/fail gate)

- Language behaviour: `qa-exception` (catch/throw), `qa-float`, `qa-factorfibo` (recursion), `qa-nullstring`, `qa-dictionary`, `qa-lookup6`, `qa-dot`, `qa-comma`, and `primes.lsp` as a sample program.
- **ORO/memory verification (high value):** `qa-inplace`, `qa-ref`, `qa-cellleak`, `qa-cpymem`, `qa-share`, `qa-blockmemory` — these directly pin down the ORO behaviour niiLISP commits to (CONTEXT.md: ORO). Treat them as the primary evidence that the ORO reproduction is faithful, not just that programs "run".
- `qa-foop` is pure-language but exercises the Context/FOOP object model, an as-yet-undesigned branch; it lands in v1 only once that model is settled, so it may trail the rest of v1.

## Deferred (not part of the v1 gate)

- **v2 / FFI:** `qa-libffi`, `qa-libc-libffi`, `qa-win-dll`.
- **Networking:** `qa-net`, `qa-net6`, `qa-udp`, `qa-packet`, `qa-broadcast`, `qa-message`, `qa-msgbig`.
- **Parsers/batteries:** `qa-xml`, `qa-json`, `qa-bayes`, `qa-statdist`.
- **Concurrency/processes:** `qa-pipe`, `qa-pipefork`, `qa-cilk`.
- **Filesystem/signals/tty:** `qa-lfs`, `qa-utf16path`, `qa-setsig`, `qa-siguser`, `qa-read-key`.
- **Unicode slice:** `qa-utf8`, `qa-utf8-char-regex`, `qa-utf8-compile`, `qa-utf8-ext`, `qa-utf8-special`. Deferred deliberately: although Rust strings are UTF-8 native, matching newLISP's exact byte-vs-char indexing/build semantics is its own slice, not a freebie.
- **Numeric tower:** `qa-bigint`, `qa-longnum` (and the `*-bench` files) wait on the numeric-model branch.

## Consequences

- v1 "done" = the In-v1 tests pass. Coverage is legible from that pass rate; deferred areas are tracked, not counted as failures (no silent scope-capping).
- Because the corpus is newLISP's own suite, the demand-driven builtin set (ADR-0008) is whatever these tests exercise — that list is now concretely derivable by reading the In-v1 files.
- If a personal `.lsp` corpus appears later, it is added as an additional gate, not a replacement (a new decision).

## Correction (2026-07-04): the qa slice is not cleanly "pure-language"

The In-v1 list above was drawn from test **names**. Reading the actual files shows the `qa-specific-tests` are broad *integration* tests that lean on newLISP's "batteries" even when nominally about one language area, so they do **not** form a clean pure-language gate:

- `qa-factorfibo` — needs **bigint** (`0L`, `bigint`) and **arrays**; belongs with the numeric-tower/array slices, not v1.
- `qa-nullstring` — is actually an **FFI** edge test (`struct`, `pack`, `unpack`, `get-string` on NULL); v2.
- `qa-lookup6` — is a **networking** test (`net-lookup`, `net-ipv`); v2.
- `qa-dictionary` — needs **contexts/namespaces** (`context`, `Ctx` as a dictionary); gated on the Context slice.
- `qa-float` — mostly pure FP, but its bit-pattern check calls `pack`/`unpack`/`bits`, so it also pulls in FFI-memory primitives before it will run to completion.
- `qa-inplace`, `qa-ref` — the deepest: they exercise **self-modifying code** (destructively editing a function's own body) plus a large set of destructive place-builtins. They validate the ORO/reference model itself and are late-stage, not near-term.

### Revised acceptance strategy

- **Primary v1 gate = project-authored tests**, not the qa suite: the unit tests in `src/eval.rs` plus small smoke scripts exercise the language core directly and pass/fail cleanly today. These are the real "v1 done" signal.
- **`qa-specific-tests` become staged integration targets**, each unlocked when its dependencies land (contexts → `qa-dictionary`, `qa-foop`; bigint+arrays → `qa-factorfibo`; FFI → `qa-nullstring`, the tail of `qa-float`; self-modifying + destructive places → `qa-inplace`, `qa-ref`; networking → `qa-lookup6`). `tests/qa.rs` runs each as a subprocess once it is expected to pass; `qa-exception` is the first and currently passes.
- This supersedes the "In v1 / Deferred" split above, which was name-based and inaccurate.
