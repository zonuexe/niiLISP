# niiLISP Language Specification

This directory is the specification of the language niiLISP accepts — in
practice, the **newLISP dialect** as re-implemented by niiLISP (see the project
[`CONTEXT.md`](../../CONTEXT.md) and [`README`](../../README.md)). It is a
pragmatic reference, not a formal standard.

The spec is split into several documents. This page is the table of contents and
the proposed overall structure; chapters marked _(planned)_ are not written yet.
Existing chapters are linked. Status markers used throughout: _(partial)_ /
_(planned)_ mark features described but not yet fully implemented — see
[`../CURRENT_WORK.md`](../CURRENT_WORK.md) for the roadmap.

## How to read

Start with [Syntax](syntax.md) for the whole picture, then dip into [Types](types.md),
[Special forms](special-forms.md), and [Functions](functions.md) as reference.
The remaining chapters below deepen sections that `syntax.md` currently
summarises.

**Where a topic lives.** A built-in's *contract* (signature, return) goes in
[functions.md](functions.md); the *semantics of the data model* it operates on
(e.g. why `cons` makes no dotted pairs) goes with the type in [types.md](types.md);
the *rationale* is in the [ADRs](../adr/); and a departure from traditional Lisp
is catalogued in [compatibility.md](compatibility.md).

## Part I — The language

| Chapter | Status | Contents |
| --- | --- | --- |
| [Syntax](syntax.md) | written | Lexical structure (comments, numbers, the three string syntaxes, symbols/contexts), the evaluation model, implicit indexing, and a section-by-section overview that the other chapters expand. The spine of the spec. |
| [Types](types.md) | written | The value types and the ORO value semantics (deep copy, no identity, structural `=` only), plus equality and ordering. |
| Evaluation & scoping | _(planned)_ | A dedicated treatment of application, argument evaluation (function vs `fexpr` vs special form), the operator-position dispatch (special form / colon / function / context / implicit index), dynamic scoping, and the save/restore binding discipline. Currently in `syntax.md` §3–4. |
| [Special forms](special-forms.md) | written | Per-form reference for all special forms: which arguments each evaluates, semantics, examples. |
| Reference & place model | _(planned)_ | The place grammar, destructive operators, and how references flow out of most expressions (`(pop (if … L))`, `$it`). Currently in `syntax.md` §8. |
| Contexts & FOOP | _(planned)_ | Contexts as namespaces and as classes: default functor, colon dispatch, `self` as a place, object identity via the head tag, symbol protection. Currently in `syntax.md` §7. |
| Error handling & control flow | _(planned)_ | `catch`/`throw` semantics, the unwinding model, and the absence of TCO / continuations. Currently in `syntax.md` §9. |

## Part II — Reference

| Chapter | Status | Contents |
| --- | --- | --- |
| [Built-in functions](functions.md) | written _(preliminary)_ | A categorised, one-line-per-function list of the evaluated-argument primitives. To be expanded with per-function argument/return detail and edge cases. |
| Foreign function interface | _(planned)_ | The full FFI reference: `import` and type marshalling, `callback`, and the memory API (`struct`, `pack`/`unpack`, `get-*`, `address`). Currently sketched in `syntax.md` §10; design in [ADR-0015](../adr/0015-import-ffi.md) and ADR-0018–0021. |
| Formal grammar | _(planned)_ | A consolidated EBNF/BNF of the reader grammar in one place, gathering the fragments scattered through `syntax.md`. |

## Part III — Notes

| Chapter | Status | Contents |
| --- | --- | --- |
| [Compatibility & deviations](compatibility.md) | written | Relationship to newLISP and to traditional Lisp: no cons/dotted pairs, no `eq` identity, dynamic scope, fexprs instead of hygienic macros, no TCO, and a comparison table. The deeper *semantics* of each departure lives with its type or form; this chapter is the catalogue. |
| Numeric model | _(planned)_ | Integer wrapping, the int64 + double tower, and the `L` bigint literal _(planned feature)_. Currently in `syntax.md` §1.2 / `types.md`. |

## Related documents

- [`CONTEXT.md`](../../CONTEXT.md) — the project glossary (canonical vocabulary
  the spec uses).
- [`docs/adr/`](../adr/) — architecture decision records: the *why* behind the
  behaviour specified here.
- [`docs/notes/`](../notes/) — background essays (the newLISP archive survey, the
  Rust/ORO retrospective, the peculiarities note).
