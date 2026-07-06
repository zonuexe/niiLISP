# REPL line editing: rustyline behind a default `readline` feature

The interactive REPL ([`src/repl.rs`](../../src/repl.rs)) read one line at a time
from stdin with no editing: arrow keys and backspace emitted control codes, there
was no history across (or within) a session, and any form spanning more than one
line was a "read error". This ADR adds real line editing without compromising the
project's zero-dependency build story.

## A pure-Rust line editor, not the system libreadline

- **Chosen:** `rustyline`, a pure-Rust readline (linenoise lineage). No system
  library, so it cross-compiles like the rest of the interpreter (unlike the
  system-libffi FFI slice) and works on Windows, where the FFI/`mt`/`net` slices
  compile out. MIT-licensed — no friction with our GPL-3.0-or-later.
- **Rejected:** binding GNU `libreadline`. Licensing is a non-issue for a GPL
  project, but it reintroduces a system-library dependency and a Windows gap —
  the exact packaging pain ADR-0018/0033 keep confining to individual features.

## Behind a default-on `readline` feature, with a stdin fallback

- **Chosen:** gate rustyline behind a `readline` feature in the default set,
  mirroring `ffi`/`bigint`/`regex`/`mt`/`net`. `--no-default-features` (the pure,
  dependency-free build) keeps the original line-at-a-time stdin REPL. The two
  front-ends share the crate's `read_forms` / `signal_message` read/eval/print
  helpers, so only the input layer differs. When rustyline cannot initialise a
  terminal (e.g. stdin is a pipe), the editing front-end falls back to the same
  stdin path at runtime rather than erroring.
- **Rejected:** an unconditional dependency. It is simpler, but it forfeits the
  zero-dependency build that every other native/heavier capability preserves.

## Multi-line continuation reuses the reader, not a second parser

- **Chosen:** the validator submits a line unless the *reader itself* stopped
  mid-token. The reader already distinguishes "stopped mid-token" (`unexpected
  end of input`, and the `unterminated …` list/string/brace-string/tag-string
  messages) from a hard syntax error (`unexpected ')'`). The REPL classifies on
  those exact strings ([`input_is_incomplete`](../../src/repl.rs)): a truncated
  form keeps editing on the next line; a valid form *or a hard syntax error* is
  submitted (the eval loop reports real errors, so a stray `)` can never trap the
  user in an unclosable continuation). One parser, one source of truth for what
  the three string syntaxes and lists consider complete.
- **Rejected:** a standalone bracket/quote scanner for the validator (rustyline
  ships `MatchingBracketValidator`). It would have to re-derive the reader's brace
  strings `{…}`, tag strings `[text]…[/text]`, comments, and escapes, and drift
  from it. Bracket *highlighting* has no such correctness stake, so there we do
  reuse rustyline's `MatchingBracketHighlighter`.

## Consequences

- History persists to `~/.niilisp_history` (`USERPROFILE` on Windows); no home
  directory means an in-session-only history, never an error.
- Tab completion offers interned session symbols plus primitive names. It is a
  prefix match over already-known names — it does not evaluate or introspect
  scope, so it will not complete a symbol the session has never mentioned.
- `Interner::all_names` was added purely to feed the completer; it is
  `allow(dead_code)` when `readline` is off.
