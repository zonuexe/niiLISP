# Changelog

All notable changes to niiLISP are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-07-04

Initial release: a usable command-line interpreter for small newLISP scripts.

### Added

- Command-line interface: run a script file, `-e EXPR`, stdin (`-`), a REPL,
  plus `--help` and `--version`.
- Reader: s-expressions, three string syntaxes (`"..."`, `{...}`,
  `[text]...[/text]`), int64/hex/float numbers (`L`-suffix bigint rejected),
  `'` quote, `;` and `#` line comments.
- Tree-walking evaluator with dynamic scoping (value slots + save/restore),
  ORO-style deep-copy value semantics, and `catch`/`throw` error handling.
- Special forms: `define`, `lambda`/`fn`, `lambda-macro`/`define-macro`
  (fexprs), `let`, `if`/`cond`/`when`/`unless`/`and`/`or`, `while`/`for`/
  `dolist`/`dotimes`, `begin`, `set`/`setq`/`setf`, `quote`, `constant`,
  `time`, `self`.
- FOOP object model: contexts, context-qualified symbols, default functors,
  colon dispatch, default parameters, and reference `self` with write-back.
- newLISP reference/place model: destructive operations (`push`, `pop`,
  `inc`/`dec`/`++`/`--`, `setf`, `replace`, `rotate`, `sort`, `reverse`,
  `set-ref`/`set-ref-all`) act through references, and control forms
  (`if`, `case`, `cond`, ...) return references. `$it` in `setf`.
- Builtins: integer (wrapping) and float arithmetic, comparisons, bitwise ops,
  list and higher-order functions (`map`, `apply`, `filter`, `sequence`),
  predicates, string helpers, `format`, `char`, and I/O.
- Vendored `qa-exception` and `qa-foop` pass as integration tests.

### Not yet implemented

`import`/FFI, networking, bigint, arrays, full UTF-8 character operations, and
the remaining newLISP standard library.

[Unreleased]: https://github.com/zonuexe/niiLISP/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.1.0
