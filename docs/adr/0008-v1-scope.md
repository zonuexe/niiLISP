# v1 scope: language core first, builtins demand-driven, `import` in v2

The first milestone (v1) is the **language core**, not newLISP's "batteries". Which builtins get implemented is **demand-driven** by the acceptance corpus rather than by exhaustively walking the manual.

## v1 contents

- Reader and printer
- ORO value model (CONTEXT.md: ORO) and Vec-backed lists (ADR-0005)
- Dynamic scoping + contexts (ADR-0006; CONTEXT.md: Dynamic scoping, Context)
- Tree-walk evaluator with dispatch cache (ADR-0007)
- Core special forms: `define`, `lambda`, `lambda-macro` (fexpr; CONTEXT.md: fexpr), `let`, `if`, `cond`, `and`, `or`, `quote`
- Core list / string / number builtins, `eval`, `apply`

**Explicitly out of v1:** `import`/FFI and the batteries (SQLite, networking, regex, XML, …).

## Acceptance gate

Compatibility is measured (per ADR-0003) against the **pure-language slice of newLISP's `qa-specific-tests`** plus a small corpus of real `.lsp` scripts. A builtin is implemented when the corpus or the tests exercise it — not because it appears in the manual. The manual is the spec for *how* a feature behaves, not a checklist of *what* to build.

## Roadmap boundary

- **v2 = `import`/FFI.** It is the headline compatibility surface (ADR-0001) and therefore the very next milestone, but it depends on a working evaluator plus string/number/list core, so it cannot precede v1.
- Batteries come after, each as its own demand-driven slice.

## Consequences

- No attempt at manual-completeness in v1; coverage is legible from the corpus/test pass rate, and gaps are expected and tracked, not treated as failures.
- The acceptance corpus needs to be chosen/curated; if the user has a target set of their own `.lsp` scripts, those seed the demand-driven builtin set. **Resolved in ADR-0009** — the corpus is the pure-language slice of `qa-specific-tests`; no personal corpus in v1.
