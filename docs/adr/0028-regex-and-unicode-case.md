# Regular expressions (RE2-style) and Unicode case folding

The UTF-8 follow-up to ADR-0025: `regex` / `regex-comp`, and Unicode-aware
`upper-case` / `lower-case`. Acceptance targets: the `qa-utf8-char-regex`,
`qa-utf8-special`, and `qa-utf8-compile` oracles. Design was grilled before
writing.

## Engine: the pure-Rust `regex` crate, not PCRE

newLISP uses **PCRE** (Perl-compatible: backreferences, lookaround). niiLISP uses
the **`regex` crate** instead:

- **Chosen: the `regex` crate**, behind a default-on `regex` Cargo feature over
  the optional `regex` dependency — the same shape as `bigint`/`num-bigint`
  (ADR-0022), so `--no-default-features` stays pure and dependency-free. It is
  pure Rust, fast, Unicode-aware, and its `regex::bytes` API matches over `&[u8]`
  and returns **byte** offsets — exactly niiLISP's binary-safe string model and
  the offsets the oracles expect.
- **Known limitation:** the crate is RE2-style, so **backreferences (`\1`) and
  lookaround (`(?=…)`) are not supported**. The `qa-utf8*` regex patterns are
  literals, and the common regex vocabulary (classes, quantifiers, groups,
  alternation, anchors) works; only advanced PCRE features are missing.
- **Rejected: a PCRE binding (`pcre2`).** Full PCRE compatibility, but it adds a
  native C dependency and build complexity (the libffi situation, ADR-0018) that
  the targets do not need. It can be added later as its own feature if real
  demand for backreferences/lookaround appears.

## `regex` and `regex-comp`

- **`(regex pattern text [option [offset]])`** — over `regex::bytes`, returns the
  first match as **`(matched-string byte-offset byte-length [subN offN lenN …])`**,
  or `nil` for no match; capture groups follow the whole match as `(str off len)`
  triples. `offset` starts the search at a byte position. `option` is a PCRE
  option-bit integer; the meaningful bits are mapped and the rest ignored:
  `1` (caseless) → case-insensitive; `0x2` → multi-line; `0x4` → dot-matches-
  newline; `0x800` (UTF-8) → a no-op, since Unicode is the default.
- **`(regex-comp pattern [option])`** — compiles (and caches) the pattern,
  returning the pattern string on success and raising an error on a malformed
  pattern (so `catch` can test it). The cache is an interpreter-side map from
  `(pattern, option)` to the compiled `Regex`, shared with `regex`, so a pattern
  is compiled once.

## Unicode case folding

`upper-case` / `lower-case` become Unicode-aware, decoding through the ADR-0025
character layer and mapping each character with Rust's `char::to_uppercase` /
`to_lowercase`; an invalid (non-UTF-8) byte passes through unchanged. ASCII is
unchanged, so existing behaviour and tests hold; non-ASCII (e.g. the Cyrillic
alphabet the oracle folds) now maps correctly. Rust uses Unicode *default* case
mapping, so a few locale-specific edges (Turkish dotless-i, German `ß`→`SS`)
follow that default rather than a C locale — a documented, minor deviation.

## Scope

- **In:** `regex`, `regex-comp`, and Unicode `upper-case`/`lower-case`. Wire
  `qa-utf8-char-regex`, `qa-utf8-special`, `qa-utf8-compile` into `tests/qa.rs`.
- **Deferred:** the `$0`/`$1` system match variables; the regex option on
  `replace`/`find`; PCRE backreferences/lookaround (would need a PCRE binding).
  `qa-utf8-ext` (needs `bits`, not regex) is a separate slice.

## Consequences

- A second pure-Rust optional dependency behind a default-on feature; the
  `regex`/`regex-comp` builtins and the interpreter's compiled-regex cache are
  gated on `feature = "regex"`. A minimal build drops them.
- `upper-case`/`lower-case` change from ASCII-only to Unicode; ASCII callers are
  unaffected, but scripts relying on non-ASCII bytes being left untouched will
  now see them case-folded.
- The RE2-vs-PCRE gap is the main compatibility caveat and is recorded in the
  glossary (CONTEXT.md: regular expression).
