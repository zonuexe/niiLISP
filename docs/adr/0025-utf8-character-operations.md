# UTF-8 character operations: the char-vs-byte contract

Implements the character-oriented function set that [ADR-0013](0013-string-representation-and-unicode.md)
deferred. The string **storage stays byte-based** (`Rc<Vec<u8>>`, binary-safe) —
that decision is unchanged and non-negotiable for FFI/IO. What this ADR pins is
*which operations are character-based and which stay byte-based*, and how the
character decoding is layered so it can be made fast later without a rewrite.
Design was grilled before writing.

## Access strategy: decode on demand, through one layer

Per-character access over UTF-8 bytes is done by **on-demand decoding** — no
cached index in this slice. On-demand char indexing is O(n), but the UTF-8
oracles (`qa-utf8*`) are display/round-trip tests, not char-index hot loops, so
a boundary index would be premature here.

The load-bearing choice is that **every character operation goes through a single
decode layer** (Rust helpers over `&[u8]`: character count, byte range of the
n-th character, a character iterator). A lazily-built, `Rc`-attached
byte-offset-per-character index (O(1) char↔byte, invalidated for free by
copy-on-write on the next write, ADR-0024) can later be added *inside that layer*
with no change to callers or semantics. So the fast data structure is deferred,
but the *place to put it* is fixed now.

Decoding is **lenient**, matching newLISP's byte scanner and preserving binary
safety: character length comes from the lead byte (`0xxxxxxx`→1, `110xxxxx`→2,
`1110xxxx`→3, `11110xxx`→4), clamped to the bytes available; an invalid lead or
truncated sequence is treated as a **one-byte character**. `str::chars()` (strict
UTF-8) is never used, because the storage may hold invalid/binary bytes.

## The contract: which operations are character-based

newLISP's UTF-8 build splits string operations — this asymmetry is the
compatibility trap, so it is pinned here (manual: "Implicit indexing … work on
character rather than single-byte boundaries"; "resting and slicing is always on
8-bit char borders"):

| Operation | UTF-8 boundary |
| --- | --- |
| `length` | **byte** (byte count) |
| `(str i)` implicit index, `nth` | **character** |
| `first`, `rest`, `last` | **character** |
| `explode` | **character** (single-character strings) |
| `char`, `utf8len` | **character** (code point) |
| `(i str)` implicit rest, `(i len str)` implicit slice, `slice` | **byte** |
| `find`, `starts-with`, `ends-with` | **byte** (substring search) |

The implicit slice/rest forms and `slice` stay byte-based deliberately — newLISP
documents them as the way to handle binary content. For ASCII, character and
byte boundaries coincide, so existing ASCII tests (e.g. `qa-longnum`'s `explode`
of a decimal string) are unaffected.

## Scope

- **In:** `utf8len` (a real newLISP function — the only *new* function); and
  switching `nth` / `(str i)` indexing / `first` / `rest` / `last` / `explode`
  to character boundaries on strings.
- **No niiLISP-original functions.** This slice only adds `utf8len` and aligns
  existing functions' semantics with newLISP; it introduces no non-newLISP
  function. Broader standard-library enhancement is a separate, later effort.
- **Deferred (no current demand):** Unicode case folding for
  `upper-case`/`lower-case` — they stay ASCII-only; `trim` stays byte-based.
  Both are real newLISP functions whose UTF-8 semantics can be upgraded together
  in a later library pass, avoiding locale-mismatch risk now.
- **Not this slice:** a full `qa-utf8` pass also needs `context`/`dotree`/`term`
  (contexts-as-namespaces) and the `qa-utf8-*regex*` oracles need `regex`; both
  are separate slices. This slice is gated on neither and does not deliver them.

## Deviations

- **`explode` does not stop at a NUL byte.** newLISP's C string scanner stops
  character processing at a zero byte; niiLISP's strings are binary-safe, so
  character `explode` processes all bytes. The oracle strings contain no NUL, so
  this is not observable there; it is recorded as a deliberate,
  binary-safety-preserving deviation.

## Acceptance

- Hermetic character-semantics tests: `utf8len` on multi-byte strings; `nth` /
  `(str i)` / `first` / `rest` / `last` / `explode` land on character boundaries
  for multi-byte input; `slice` / `(i str)` / `length` stay byte-based; ASCII
  behaviour is unchanged (regression guard).
- `qa-utf8` and the other `qa-utf8*` oracles stay **gated** on the separate
  context/regex slices; CURRENT_WORK records the co-dependencies.

## Consequences

- A small decode module (`utf8.rs` or a section of `builtins.rs`) becomes the one
  place character boundaries are computed; all char-based builtins call it.
- `nth`/`first`/`rest`/`last`/`explode` gain a character path for `Str` while
  keeping their list/array paths; `index_one`'s `Str` arm becomes character-based.
- The byte-based operations are untouched, so binary/FFI code paths are
  unaffected.
- The eventual `Rc`-attached boundary index (ADR-0013's "decode on demand" made
  O(1)) is purely additive inside the decode layer — a later optimisation, not a
  re-design.

## Outcome

Implemented in `src/utf8.rs` (the decode layer) plus the char paths in
`builtins.rs`/`eval.rs`. `utf8len`, and character-based `nth` / `(str i)` /
`first` / `rest` / `last` / `explode`, all land on character boundaries; `slice`
/ `(i str)` / `length` stay byte-based. Adding a string to the functor-position
indexing arm (`(str i)`) also fixed a pre-existing gap — strings were not
self-indexing at all before. ASCII behaviour is unchanged (existing tests green).
The `qa-utf8*` oracles remain gated on `context`/`dotree`/`term` and `regex`.
