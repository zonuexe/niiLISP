# WikiBook coverage: *Introduction to newLISP* vs niiLISP (2026-07-06)

Empirical coverage of the [*Introduction to newLISP* WikiBook](https://en.wikibooks.org/wiki/Introduction_to_newLISP):
does each feature the book teaches actually work in niiLISP, and does it behave as the book describes?

- **Binary probed:** `target/release/niilisp` — `niilisp 0.3.1 (2026-07-06 revision 0b83272)`, default features (`ffi`, `bigint`, `readline`). The opt-in `gui` (fltk) feature is **off**.
- **Method:** each chapter's worked examples were run against the built binary by a Sonnet sub-agent and the output compared to the book. Because niiLISP returns `nil` for a call to an unbound symbol (a missing builtin does **not** error), each function was probed *functionally*, never assumed present from the absence of an error.
- **Verification pass:** every ⚠️/❌ verdict that implied niiLISP *diverges from* (rather than *lacks*) a feature was re-checked against the vendored **newLISP 10.7.5 manual** (`references/newlisp/doc/newlisp_manual.html`). This caught several sub-agent false positives where niiLISP is actually correct — see *Corrections* below. "Unbound / missing" verdicts proved reliable and were spot-checked, not exhaustively re-run.
- **Legend:** ✅ works as described · ⚠️ present but divergent/partial · ❌ missing or broken.

## Summary

**≈218 ✅ / 15 ⚠️ / 81 ❌** across ~314 probed items (~70% work as the book describes). Counts mix granularities — most chapters count individual functions; ch. 15 counts whole example programs — and a few chapters' own tallies are approximate, so treat these as directional, not exact.

| # | Chapter | ✅ | ⚠️ | ❌ | Report |
|---|---------|----|----|----|--------|
| 1 | The basics | 17 | 1 | 0 | [01-the-basics.md](01-the-basics.md) |
| 2 | Controlling the flow | 25 | 1 | 3 | [02-controlling-the-flow.md](02-controlling-the-flow.md) |
| 3 | Lists | 32 | 1 | 10 | [03-lists.md](03-lists.md) |
| 4 | Strings | 28 | 0 | 7 | [04-strings.md](04-strings.md) |
| 5 | Apply and map | 6 | 0 | 2 | [05-apply-and-map.md](05-apply-and-map.md) |
| 6 | Contexts | 12 | 2 | 4 | [06-contexts.md](06-contexts.md) |
| 7 | Macros | 4 | 1 | 2 | [07-macros.md](07-macros.md) |
| 8 | Working with numbers | 31 | 1 | 6 | [08-working-with-numbers.md](08-working-with-numbers.md) |
| 9 | Working with dates and times | 3 | 0 | 6 | [09-dates-and-times.md](09-dates-and-times.md) |
| 10 | Working with files | 26 | 2 | 8 | [10-working-with-files.md](10-working-with-files.md) |
| 11 | Multitasking | 10 | 2 | 2 | [11-multitasking.md](11-multitasking.md) |
| 12 | Working with XML | 1 | 0 | 6 | [12-working-with-xml.md](12-working-with-xml.md) |
| 13 | The debugger | 2 | 0 | 7 | [13-the-debugger.md](13-the-debugger.md) |
| 14 | The Internet | 11 | 1 | 10 | [14-the-internet.md](14-the-internet.md) |
| 15 | More examples (programs) | 2 | 0 | 4 | [15-more-examples.md](15-more-examples.md) |
| 16 | Graphical interface | 8 | 3 | 4 | [16-graphical-interface.md](16-graphical-interface.md) |

## What the picture shows

**Solid (the core language).** Chapters 1–3 and 5–8 — basics, flow control, lists, apply/map, contexts, macros, numbers — are largely faithful. Notably, the **bigint model matches newLISP exactly** (operand-triggered promotion, `L` suffix, too-large literals), FOOP/contexts work, and the fork-based **multitasking** and **TCP networking** cores (ch. 11, 14) are real and verified against live local processes/sockets.

**Whole subsystems absent (the bulk of the ❌).** These cluster, matching the earlier [gap analysis](../20260706_newlisp-gap-analysis.md):
- **Dates & times** (ch. 9) — `date`/`now`/`date-value`/`date-parse`/`timer` all unbound; only `time`/`sleep` work.
- **XML / JSON** (ch. 12) — no `xml-parse`/`xml-type-tags`/`json-parse` at all.
- **Debugger** (ch. 13) — no `trace`/`debug`/`error-event` family; only `catch`/`throw`.
- **HTTP & UDP** (ch. 14) — `get-url`/`put-url`/`post-url` and all UDP/`net-lookup`/`net-eval` missing (stream sockets present).
- **newLISP-GS GUI** (ch. 16) — the book's JVM `guiserver.lsp` path will never run; niiLISP ships an architecturally different opt-in **fltk** helper (ADR-0034), but every non-visual prerequisite (`load`/`env`/`process`/`net-*`/`base64`/`eval-string`) is present.

**Recurring smaller gaps** that break book examples across chapters:
- ~~**`$idx`** loop-index variable~~ — **fixed 2026-07-06**: now populated in `dolist`/`dostring`/`dotree`/`map`/`while`/`until`/`do-while`/`do-until`.
- ~~**Regex mode** for `find`/`replace` and **`$0..$9`** captures~~ — **fixed 2026-07-06**: regex-mode `find`/`replace`, `$0..$N` binding, and per-match re-evaluation now work; both ch. 15 file-tree editors run.
- Missing binding/HOF helpers: `letn`, `doargs`, `curry`, `global`, `find-all`, `exists`, `match`, `ref`/`ref-all`, `clean`, `series`, `factor` (ch. 2, 3, 5, 8, 15).
- File I/O has holes: `copy-file`, `read-char`, `write-char`, `device`, `search`, `dump`, `pretty-print` unbound; `save` writes an empty file (ch. 10).

## Corrections applied during verification

The sub-agents were reliable on "unbound/missing" but produced several **false "divergence" verdicts** by recalling newLISP semantics from memory instead of the manual. Re-checked against the 10.7.5 manual and reclassified:

| Chapter | Sub-agent claim | Reality |
|---|---|---|
| 8 | `L` suffix ignored; no bigint; factorial wraps | `L` works; too-large literals auto-parse as bigint; plain-int overflow wraps **in newLISP too** — niiLISP matches |
| 8 | `(zero? 0)` returns `nil` | Returns `true` (could not reproduce) |
| 5 | `+` on floats "rounds/truncates" (bug) | Correct: newLISP `+` is integer arithmetic; `add` is the float version |
| 3 | `push` returns whole list instead of element | Correct: manual says push *"returns the list changed as a reference"* |
| 2 | `do-while`/`do-until` argument order reversed | Correct: newLISP is `(do-while exp-condition body)` — condition first, as niiLISP does |
| 2 | `dotree` unusable | Works with the real single-var syntax `(dotree (sym context) body)`; was mis-tested with two vars |
| 10 | `file?` true for directories (bug) | Correct: manual says *"will also return true for directories"* |
| 10 | `write-line` argument order reversed | Correct order (handle first); only the handle-less stdout form is missing |

## Caveats

- Coverage counts are directional; chapters count features at different granularities.
- Some interactive-only surfaces (the step debugger, live GUI event loops) were probed structurally, not driven interactively.
- `date-parse`, some UDP, and Windows-FFI behaviors are platform-gated in newLISP too; "missing" here means "not runnable as the book writes it on this macOS default build."
