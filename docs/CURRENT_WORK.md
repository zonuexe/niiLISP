# Current Work

A living handoff + backlog for niiLISP. Records what state the project is in and
what is deliberately deferred, so work can resume without re-deriving context.

## Status

- **v0.2.0 released** (2026-07-06): on [crates.io](https://crates.io/crates/niilisp)
  (`cargo install niilisp`) and as GitHub Release binaries for 4 targets
  (x86_64/aarch64 Linux + aarch64 macOS with FFI, x86_64 Windows pure). Ships the
  whole arc below — FFI, bigint, arrays, copy-on-write, UTF-8 char ops,
  contexts-as-namespaces, lambdas-as-list. (v0.1.0 was 2026-07-04.)
- Working: reader, tree-walking evaluator, dynamic scope + contexts, ORO value
  semantics, FOOP with reference `self`, the reference/place model, `catch`/`throw`,
  and a core builtin set. Passes vendored `qa-exception` and `qa-foop`.
- Shipped in v0.2.0 (since v0.1.0): **FFI `import`** (typed C calls,
  ADR-0018/0019), **`callback`** (ADR-0020), the **FFI memory API**
  (`struct`/`pack`/`unpack`/`get-*`/`address`, ADR-0021), **bigint**
  (arbitrary-precision integers, ADR-0022), **arrays** (fixed-length,
  ADR-0023), **copy-on-write values** (`Rc`/`make_mut`, ADR-0024),
  **UTF-8 character operations** (ADR-0025), **contexts as switchable
  namespaces** (`context`/`dotree`/`term`, ADR-0026), and **lambdas as list
  data** (`expand`/`args` + callable lambda-lists, ADR-0027); `case`/`if-not`
  promoted to full special forms; a language spec under [`docs/spec/`](spec/)
  (syntax, types, special-forms, functions).
- Since v0.2.0 (unreleased, on `master`): **regex** and **Unicode case folding**
  (ADR-0028); **character-based `dostring`** (ADR-0025); **file I/O**
  ([ADR-0029](adr/0029-file-io-slice.md)) — handles (`open`/`close`/`seek`/
  `read-buffer`/`write-buffer`/`read-line`), whole-file (`read-file`/`write-file`/
  `append-file`), filesystem (`directory`/`real-path`/`make-dir`/`remove-dir`/
  `change-dir`/`rename-file`/`delete-file`/`file-info`/`file?`/`directory?`/`env`),
  and persistence (`save`/`load`/`source`), all always-on (pure `std::fs`); and
  **dictionaries** ([ADR-0030](adr/0030-dictionary-context-as-hash.md)) — a
  nil-default-functor context as a hash (`(Ctx key [val])`/`(Ctx assoc)`/`(Ctx)`),
  with `new` copying prototypes and the predefined `Class` marker keeping FOOP
  construction distinct, plus `delete`/`sys-info`/`randomize`.
- **WikiBook coverage audit + divergence fixes** (2026-07-06,
  [`notes/20260706_wikibook-coverage/`](notes/20260706_wikibook-coverage/)): ran
  every worked example of the *Introduction to newLISP* WikiBook against the
  binary (~221/314 work). Fixed the divergences it surfaced: the **`$idx`**
  system iterator (dolist/dostring/dotree/map/while/until/do-while/do-until);
  **`int`** nil/default-on-failure + base/prefix parsing; **`dup`** list flag;
  **`<<`/`>>`** 1-arg + fold; **`round`** newLISP sign convention;
  **`time-of-day`** ms-since-midnight; no-arg **`save`** dumps the workspace;
  **regex capture vars `$0..$N`** + regex-mode `find`/`replace` with per-match
  re-evaluation; and the **`(context)`** query and **`(context ctx word [value])`**
  create/set forms.
- Tests: 65 unit + qa integration (`qa-exception`, `qa-foop`, `qa-dictionary`,
  `qa-nullstring`, `qa-bigint`, `qa-longnum`, `qa-utf8`, `qa-utf8-char-regex`,
  `qa-utf8-special`, `qa-utf8-compile`, `qa-utf8-ext`, and the Cilk/process
  oracles `qa-cilk`/`qa-share`/`qa-pipefork`/`qa-message`/`qa-siguser`;
  `qa-factorfibo` `#[ignore]`d — slow sieve) + hermetic `fileio` tests, three
  `process` tests, two hermetic `ffi` tests, and the WikiBook-audit regression
  suites (`loop_idx`, `builtin_semantics`, `regex_captures`, `contexts`); ~118
  tests total, passing under both default features and `--no-default-features`.
- Standard-library fill-ins (byte-based, no UTF-8 dependency): string builtins
  `upper-case`/`lower-case`/`trim`/`slice`/`find`/`explode`/`chop`, the RNG
  (`seed`/`rand`/`random`/`amb`), `main-args`, the list/number builtins
  `min`/`max`/`even?`/`odd?`/`flat`/`join`/`member`/`unique`/`true?`, and the
  `dostring`/`until`/`extend`/`swap` special forms.

## Next task — pick up here: choose the next slice

A gap analysis vs newLISP ([`notes/20260706_newlisp-gap-analysis.md`](notes/20260706_newlisp-gap-analysis.md))
found ~221 of 378 primitives missing, clustered into whole unbuilt subsystems
(file I/O, processes, networking, XML/JSON, dates). The
[WikiBook coverage report](notes/20260706_wikibook-coverage/) complements it as a
living, example-driven backlog: the near-term divergences (⚠️) are now fixed, so
what remains is **whole unimplemented subsystems (❌)** — best filled one planned
slice at a time. Highest-value remaining ❌, roughly:
**XML/JSON** (`xml-parse`/`json-parse`), the
**debugger** (`trace`/`debug`/`error-event`), and **HTTP/UDP**
(`get-url`/`net-*-udp`). **Dates/times** landed 2026-07-06 under
[ADR-0037](adr/0037-dates-and-times.md) (`date`/`now`/`date-value`/`date-list`/
`date-parse`; a pure UTC core plus libc local time behind a `date` feature); only
`timer` remains. The pattern/reference family
(`ref`/`ref-all`/`match`/`find-all`/`pop-assoc`) landed 2026-07-06 under
[ADR-0036](adr/0036-reference-and-query-model.md), completing the *Lists* chapter;
`unify` remains deferred. **North star (grilled): order
by dependency, not by the GUI** — the *Graphical interface* chapter stays a
long-horizon target we do **not** schedule (newLISP-GS needs process + net + a
Java `guiserver.jar`; its `eval-string`-driven socket substrate is three unbuilt
subsystems). Near term we build the shared foundation the GUI *also* needs.

**GUI — first slice done (ADR-0034), no JVM.** Rather than ship newLISP's Java
`guiserver.jar`, niiLISP has a **native, `gs:`-inspired GUI**: `lib/gui.lsp`
drives a separate `niilisp-gui` **fltk** helper binary over a socket (reusing the
`net-*` + `eval-string` substrate). Widgets so far: window/frame, label, button,
text-field (vertical auto-layout) with click events. Behind a **default-off `gui`
feature** (fltk bundled, linked only into the helper — the interpreter stays
pure). Display-dependent, so CI covers only the protocol layer (`tests/gui.rs`);
`examples/gui-demo.lsp` is the manual demo (`cargo build --features gui --bin
niilisp-gui`, then `NIILISP_GUI=…/niilisp-gui`). The stance is **gs:-shaped, not
bug-compatible** (a different toolkit than Swing). **Next GUI slices:** real
layout managers (border/grid/flow), `gs:canvas` + drawing, more widgets/events,
and reading widget state back (the demo's field→label round trip is stubbed).

Done so far: file I/O (ADR-0029, handles + filesystem + `save`/`load`/`source`),
dictionaries (ADR-0030, `qa-dictionary` passes and is wired), **external processes**
(ADR-0031: `process`/`exec`/`!`/`sleep`, cross-platform, always-on), and
**binary-safe string repr** (ADR-0032 prerequisite — `save`/`source`/REPL now
round-trip binary strings). Candidates, roughly by value:

- **Cilk / fork multitasking** (ADR-0032) — **done and wired.** Real Unix
  `fork()` of the interpreter behind a default-on `mt` feature (`libc` dep),
  bounded unsafe; cross-process values transfer as re-readable `repr`. **B1**
  spawn/sync/abort/fork → `qa-cilk` ✓; **B2** `share` (mmap) → `qa-share` ✓; **B3**
  `pipe`/`wait-pid` + `write-line` + `do-until`/`do-while` → `qa-pipefork` ✓; **B4**
  `send`/`receive` (datagram socketpairs) + `sys-info -3`/`-4` → `qa-message` ✓;
  **B5** `signal` (async-safe, polled at eval safe points) → `qa-siguser` ✓.
  **Deferred:** `qa-msgbig` (`base64-enc` now exists, but the 80 KB messages
  still need a `SOCK_STREAM` + length-framed transport — a `SOCK_DGRAM` can't
  carry them); `qa-pipe` (uses an external `./newlisp` binary); the `process`
  stdio-fd redirection args.
- **Networking** (ADR-0033) — **stream sockets done and wired.**
  `net-connect`/`net-listen`/`net-accept`/`net-send`/`net-receive`/`net-select`/
  `net-peek`/`net-peer`/`net-local`/`net-close` (TCP + Unix-domain), behind a
  default-on `net` feature, Unix-only; a socket is a FileTable handle. **`qa-local-domain`
  passes** (the GUI's connect/listen/accept/send/receive/select substrate).
  **Remaining:** UDP (`net-send`/`net-receive` datagram, `net-send-to`/
  `net-receive-from`) → `qa-udp`; the multi-socket `net-select`; `net-eval` +
  server mode (`qa-net`/`qa-net6` need a `newlisp` server binary); raw
  `net-packet`/`net-ping` (root, `qa-packet`/`qa-lookup6`); `net-lookup` (DNS);
  `get-url`/HTTP.
- **qa-ref tail** — string-byte places (`(setf (s 3) "D")`), `eval`/loop
  place-returns. Touches the place model; scope first.
- **Independent leaves** — **done:** `parse`, rounding/sign
  (`ceil`/`floor`/`round`/`sgn`), hyperbolic/`atan2` trig, `bits`,
  `base64-enc`/`base64-dec`, list ops (`count`/`select`/`difference`/`intersect`
  and the higher-order query family `clean`/`index`/`exists`/`for-all`/`transpose`
  from the WikiBook Lists chapter, 2026-07-06), binding forms `letn`/`letex`
  (+ `let` parenthesized/bare-symbol syntax, 2026-07-06), `curry`,
  `global`/`global?` (2026-07-06; global-symbol reader integration is limited by
  the batch-read model — see the contexts known-limitation note below),
  `series`/`factor` (2026-07-06), the reference/query family
  `ref`/`ref-all`/`match`/`find-all`/`pop-assoc` + `push`/`pop` index vectors
  (ADR-0036, 2026-07-06), dates/times `date`/`now`/`date-value`/`date-list`/
  `date-parse` (ADR-0037, 2026-07-06),
  reflection predicates (`context?`/`lambda?`/`macro?`/`primitive?`/`bigint?`/
  `protected?`), `title-case`. **Remaining:** XML/JSON (`xml-parse`/`json-parse`),
  the `timer` scheduler, symbol reflection
  (`sym`/`symbols`/`name`/`prefix`), `unify` (Prolog-style, deferred per
  ADR-0036), `bind`, `doargs`, `ostype`, matrix/stats math.

**Windows note:** `qa-utf16path` needs faithful UTF-16 path handling (file I/O
currently does lossy UTF-8 on Windows, binary-safe on Unix — ADR-0029). A future
Windows-paths slice would unlock it.

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

**File I/O + Dictionaries** ([ADR-0029](adr/0029-file-io-slice.md),
[ADR-0030](adr/0030-dictionary-context-as-hash.md), grilled before writing): the
first two foundation slices from the gap analysis. File I/O (`src/fileio.rs`,
always-on pure `std::fs`): opaque integer handles from an interp registry (0/1/2
reserved, freelist reuse), `open`/`close`/`seek`/`read-buffer` (a place-taking
special form)/`write-buffer`/`read-line`/`current-line`, whole-file
`read-file`/`write-file`/`append-file`, filesystem
`directory`/`real-path`/`make-dir`/`remove-dir`/`change-dir`/`rename-file`/
`delete-file`/`file?`/`directory?`/`file-info`/`env`, and persistence
`save`/`load`/`source`. Operational failure → nil; byte-buffer paths → OS-native
(binary-safe on Unix; Windows UTF-16 deferred). Dictionaries: context application
dispatches on the default functor `Ctx:Ctx` — lambda → FOOP constructor, nil →
hash (`(Ctx key [val])` get/set/delete, `(Ctx assoc)` bulk-load, `(Ctx)`
enumerate sorted), other non-nil → tagged object list; keys are `_`-prefixed
context symbols. `new` copies prototypes and the predefined `Class` marker gives
FOOP classes a non-nil functor, keeping construction distinct from hash access.
Added `delete`/`sys-info`/`randomize`. `qa-dictionary` passes (bulk-load →
verify → save → delete → load → byte-identical) and is wired. `qa-lfs` is
interactive + 5 GB so a hermetic `tests/fileio.rs` covers its logic instead.

**Character-based `dostring`** (extends [ADR-0025](adr/0025-utf8-character-operations.md),
since v0.2.0): `dostring` now binds its loop variable to each UTF-8 character's
Unicode code point instead of each byte, matching newLISP's UTF-8 build. A new
`utf8::codepoints` lenient decoder feeds it (invalid/binary bytes fall back to the
raw byte value); for ASCII a code point equals the byte, so `qa-bigint`/`qa-longnum`
(digit-string `dostring` loops) are unaffected. `qa-utf8-ext` now passes and is
wired in (gated on the `ffi` feature — it also uses `unpack`). `char`'s
int→string path already produced the multi-byte character, and the oracle's
`bits` lines are commented out, so no new builtin was needed.

**Regex + Unicode case folding** ([ADR-0028](adr/0028-regex-and-unicode-case.md),
since v0.2.0): `regex`/`regex-comp` on the pure-Rust `regex` crate (RE2-style, not
PCRE — no backreferences/lookaround), behind a default-on `regex` feature, with an
`Interp` compile cache and PCRE-option-bit mapping; `upper-case`/`lower-case` fold
Unicode via the char layer. `qa-utf8-char-regex`/`qa-utf8-special`/`qa-utf8-compile`
pass and are wired in. **`$0..$N` capture vars and regex on `find`/`replace` are
now done** (2026-07-06 WikiBook audit; `Interp::set_regex_captures`, regex-mode
`find`, per-match-re-evaluating string `replace`). Still deferred: PCRE features
(backreferences/lookaround) and `bits` (for `qa-utf8-ext`).

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

## Release pipeline (resolved)

`release.yml`'s binary matrix is **resolved** (ADR-0018 "Release matrix"): each
Unix target builds on a **native** runner (incl. `ubuntu-24.04-arm` for
`aarch64-linux` and `macos-13` for `x86_64-darwin`) with the default `ffi`
feature and a `libffi-dev` install on Linux — no cross-compilation. Windows
builds `--no-default-features` (pure, FFI deferred). So downloaded Unix binaries
carry `import`; the Windows binary does not; `cargo install` gets FFI from source
on Unix regardless. Nothing else blocks tagging.

## Releasing

Follow the `niilisp-release-prep` skill (`.claude/skills/`): bump version, seal
the `[Unreleased]` changelog, reconcile the README, verify, and push a `vX.Y.Z`
tag (after a human Go — publishing is irreversible).
