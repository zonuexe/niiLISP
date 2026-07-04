# String representation: `Vec<u8>` byte buffer, single UTF-8-semantics build

niiLISP strings are **`Vec<u8>` byte buffers** (CONTEXT.md: String (byte buffer)), not Rust `String`. The full character-oriented Unicode semantics are deferred out of v1 (qa-utf8* is outside the v1 gate, ADR-0009), but the *representation* is fixed now because every v1 string depends on it and migrating later would be painful.

## Why bytes, not `String`

newLISP strings are binary-safe byte buffers: `qa-utf8` builds them from explicit byte escapes (`\195\171`), and strings routinely carry binary data for file I/O, networking, and (in v2) FFI. Rust's `String` guarantees valid UTF-8 and therefore **cannot hold** arbitrary or invalid-UTF-8 bytes — using it would be a direct compatibility break, worst of all at the I/O/FFI boundary we commit to in v2 (ADR-0001). So:

- **`length` = byte count; `utf8len` = character (code point) count.**
- Character-oriented operations **decode UTF-8 on demand** over the byte storage; they don't require the storage to be valid UTF-8.

## Single build, not newLISP's two builds

newLISP ships separate ASCII and UTF-8 builds. niiLISP ships **one** build with **UTF-8 semantics as the default** (the target of qa-utf8*), while still exposing byte-oriented operations — because the storage is bytes, both character- and byte-oriented functions coexist in one binary. Maintaining two builds would work against priority #2 (ADR-0004).

## Deferral boundary

- v1: the `Vec<u8>` representation and byte-oriented behaviour (`length`, byte indexing, construction from escapes).
- v2 slice: the precise character-oriented function set (`utf8len`, char indexing/slicing, `char`/`unicode`, regex over UTF-8), pinned to `qa-utf8`, `qa-utf8-char-regex`, `qa-utf8-compile`, `qa-utf8-ext`, `qa-utf8-special` as oracles.

## Consequences

- No `Value::Str(String)`; it is `Value::Str(Vec<u8>)` (or a newtype over it).
- Converting to `&str` for character work is a fallible, on-demand decode — never an invariant of the value.
- This representation is also what `import`/FFI (v2) marshals to/from C `char*`/byte buffers, so the FFI branch inherits it for free.
