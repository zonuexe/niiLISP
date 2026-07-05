# Changelog

All notable changes to niiLISP are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- String builtins: `upper-case`/`lower-case` (ASCII case, byte by byte — bytes ≥ 0x80 unchanged), `trim` (`(trim s)` / `(trim s ch)` / `(trim s l r)`), `slice` (`(slice seq start [len])` — a copied sub-range of a string or list, negative `start`/`len` counted from the end, clamped bounds), and `find` (`(find key seq)` — substring byte offset or list-element index, else `nil`).
- `dostring` special form: `(dostring (var str [break]) body)` iterates `var` over each byte of `str` as an integer (0–255), mirroring `dolist`.
- `case` and `if-not` are now full special forms usable in value position (not only as reference-returning place arguments); a `true`/`t` `case` label is the catch-all.

- `import`/FFI, first slice: call C functions from shared libraries with typed signatures — `(import "libm.so" "cos" "double" "double")` then `(cos x)`. Supports `void`, `int`, `long`, `float`, `double`, `char*`, and `void*`; `import` returns `nil` when a library or symbol cannot be resolved. Behind a default-on `ffi` Cargo feature (Unix only for now; `--no-default-features` gives a pure, safe, dependency-free build). Uses the system libffi via `libloading` + `libffi`.
- `callback`: pass a niiLISP function to C as a function pointer — `(apply_cb (callback 'f "int" "int") 21)`. Implemented with libffi closures; a `throw`/error inside a callback is reported to stderr and does not cross the C boundary.
- FFI memory API (third slice): build and read C structs and raw buffers so `import`ed functions can exchange structured data. `(struct 'name t…)` names a layout (a list of C type names); `(pack layout val…)` serialises values to a binary string laid out as that C struct (native alignment, padding, byte order) and `(unpack layout str)` reads them back. `(get-string addr [len [limit]])`, `get-int`, `get-long`, `get-float` (a C double), and `get-char` read a C value at an integer address; `(address 'sym)` returns the stable buffer address of a symbol-held string. A packed struct is handed to C by passing a string to a `void*` argument (no copy, binary-safe). A NULL (0) pointer through `unpack`/`get-string` raises an error instead of crashing.
- `pack`/`unpack` also accept a **format string** (the terse mini-language) in place of a struct: `c`/`b` (signed/unsigned 8-bit), `d`/`u` (16-bit), `ld`/`lu` (32-bit), `Ld`/`Lu` (64-bit), `f` (float), `lf` (double), `sN` (an N-byte string), `nN` (N null bytes), with `>`/`<` to switch to big-/little-endian for the following fields. Unlike a struct, a format string is packed tightly with no alignment — e.g. `(pack "c c c" 65 66 67)` -> `"ABC"`.

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
