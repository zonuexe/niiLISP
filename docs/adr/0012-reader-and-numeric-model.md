# Reader and numeric model (v1)

## Numeric model

v1 has two number values: **`Value::Int(i64)`** (64-bit signed) and **`Value::Float(f64)`** (IEEE 754 double), matching newLISP.

- **Overflow wraps.** Integer arithmetic uses wrapping operations (`wrapping_add`, etc.) so 64-bit overflow is modular, exactly as newLISP does. This deliberately **avoids Rust's defaults** (debug-build overflow panic; checked/saturating). Getting this wrong is a silent compatibility trap, hence it is called out here.
- **No auto-promotion.** newLISP does not promote an overflowing `i64` to bigint, so niiLISP must not either — option (c), auto-promote-on-overflow, was rejected as a compatibility break.
- **bigint is deferred past v1.** bigint literals use the **`L` suffix** (e.g. `12345L`, `123456789012345678901234580235L`). The reader **recognises the `L`-suffix syntax and raises a clear "bigint unsupported in v1" error**, reserving the syntax slot rather than mis-lexing it as a symbol. `qa-bigint`/`qa-longnum` are outside the v1 gate (ADR-0009); bigint becomes `Value::Bigint` in a later slice.
  - **Revised by [ADR-0022](0022-bigint-numeric-tower-slice.md)** (bigint implemented, behind a default-on `bigint` feature). The literal rule here was incomplete: a decimal literal too large for `i64` also becomes a bigint, not only an `L`-suffixed one. "No auto-promotion" above refers to *arithmetic overflow* (still wraps) and is unaffected.

## Reader (lexical) surface for v1

Reproduced faithfully (fixed by compatibility, ADR-0001):

- **Three string syntaxes**, all in the v1 reader because `{...}` in particular is pervasive (regex/text):
  - `"..."` — escape-processed, single logical line.
  - `{...}` — raw, nestable braces, may span lines (no escape processing).
  - `[text]...[/text]` — raw block for large/verbatim text.
- **Symbols** may contain `.` (e.g. `a.re`). `:` is the Context separator: `Ctx:sym` lexes to a qualified symbol and `:method` to a colon-dispatch form (ADR-0010).
- **Numbers vs symbols:** a token is a number if it parses wholly as int64/float (incl. hex/scientific); otherwise a symbol.
- **`nil` and `true`** are constants; `'expr` is quote; `;` and `#` start line comments.
- `qa-comma` / `qa-dot` in the reference are 0-byte placeholders — no behaviour to match there.

## Consequences

- A numeric-tower branch (bigint, and any ratio/complex ambitions) is a separate future decision; v1 code paths assume `Int`/`Float` only.
- Wrapping arithmetic must be the default in every integer builtin — reviewers should treat a plain `+`/`*` on `i64` in arithmetic builtins as a bug.
