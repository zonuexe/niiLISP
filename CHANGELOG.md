# Changelog

All notable changes to niiLISP are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-07-06

### Added

- **Lambdas as list data** (ADR-0027): a lambda now presents newLISP's list interface — `(lambda …)`/`(fn …)`/`(lambda-macro …)` build and print as `(lambda (params…) body…)`, an empty `(lambda)` is the list `(lambda)`, `append` builds a lambda as data, and a list headed by `lambda`/`fn`/`lambda-macro` is callable. Adds `expand` (`(expand expr sym…)` substitutes named symbols' values; `(expand expr)` auto-substitutes upper-case symbols bound to code-like values) and `args` (the current function's arguments not bound to a parameter). A special form can also be aliased as a value (`(define DEFINE define)`). This runs newLISP's code-as-data lambda idiom, e.g. the [lambda-calculus gist](https://gist.github.com/kosh04/262332).
- **Contexts as switchable namespaces** (ADR-0026): `(context 'X)` makes `X` the current context so unqualified symbols read after it are created in it (`X:sym`) — a read-time effect, so a symbol's context is fixed when read. `(context MAIN)` switches back. `dotree` — `(dotree (var ctx [only-top]) body)` — iterates a context's symbols in name order; `term` returns a symbol's unqualified term (`(term 'L:a)` → `a`). Names that are MAIN primitives (builtins/special forms) stay unqualified inside a context. Unlocks the `qa-utf8` display test.
- **UTF-8 character operations** (ADR-0025): `utf8len` (character count, vs `length`'s byte count), and `nth` / `(str i)` indexing / `first` / `rest` / `last` / `explode` now work on **character** boundaries for strings, so multi-byte characters stay whole. String storage remains binary-safe bytes; `slice`, the implicit slice `(i str)`, `length`, and substring search stay byte-based (for binary content). Decoding is lenient — invalid/truncated bytes count as one character each — and never assumes valid UTF-8. For ASCII, character and byte boundaries coincide, so existing behaviour is unchanged.
- **arrays** — newLISP's fixed-length, list-like value (ADR-0023). `(array size [init])` builds one (cycle-filling from `init`, else nil-filling); it indexes, `setf`-assigns elements, reports `length`, and prints like a list, but `array?` is true / `list?` is nil and it cannot be resized (`push`/`pop`/`extend` error). `array-list` converts it to a list. `true?` predicate added. (1-D only; the multi-dimensional constructor and wide list-op acceptance are deferred.)
- More standard builtins: `min`/`max` (numeric, type-preserving), `even?`/`odd?` (integers and bigints), `flat` (flatten a nested list), `join` (concatenate a list of strings with an optional separator), `member` (list tail / substring from the first match), `unique` (drop duplicate list elements), and the `swap` special form (exchange two places).
- Helper functions that, with bigint, make the vendored `qa-bigint` and `qa-longnum` suites pass: a seedable RNG — `seed`, `rand` (`(rand max [count])`), `random` (`(random)` in `[0,1)`, `(random offset scale [count])`) — and `amb` (evaluate one argument at random); the `until` loop special form (inverse of `while`); `extend` (destructively append to a string or list place); `explode` (split a string/list into `n`-wide pieces) and `chop` (drop the last `n` bytes/elements); and `main-args` (the process command line).
- **bigint** — arbitrary-precision integers (ADR-0022). A decimal literal too large for `i64`, or any `L`-suffixed literal (`12L`), reads as a bigint; `+ - * / %` yield a bigint when an operand is one (float args are truncated to integer, as with `i64`), and a fitting result stays a bigint (no auto-demote). `bigint` converts a number/string, `gcd` is added, and `int`/`float`/comparisons/`zero?`/`abs`/`length` (digit count) understand bigints. A bigint prints as plain decimal (no `L`). Behind a default-on `bigint` Cargo feature over `num-bigint`; `--no-default-features` drops the dependency and the literals become an error again. Arithmetic overflow of two `i64`s still wraps — only literals promote.
- String builtins: `upper-case`/`lower-case` (ASCII case, byte by byte — bytes ≥ 0x80 unchanged), `trim` (`(trim s)` / `(trim s ch)` / `(trim s l r)`), `slice` (`(slice seq start [len])` — a copied sub-range of a string or list, negative `start`/`len` counted from the end, clamped bounds), and `find` (`(find key seq)` — substring byte offset or list-element index, else `nil`).
- `dostring` special form: `(dostring (var str [break]) body)` iterates `var` over each byte of `str` as an integer (0–255), mirroring `dolist`.
- `case` and `if-not` are now full special forms usable in value position (not only as reference-returning place arguments); a `true`/`t` `case` label is the catch-all.
- `import`/FFI, first slice: call C functions from shared libraries with typed signatures — `(import "libm.so" "cos" "double" "double")` then `(cos x)`. Supports `void`, `int`, `long`, `float`, `double`, `char*`, and `void*`; `import` returns `nil` when a library or symbol cannot be resolved. Behind a default-on `ffi` Cargo feature (Unix only for now; `--no-default-features` gives a pure, safe, dependency-free build). Uses the system libffi via `libloading` + `libffi`.
- `callback`: pass a niiLISP function to C as a function pointer — `(apply_cb (callback 'f "int" "int") 21)`. Implemented with libffi closures; a `throw`/error inside a callback is reported to stderr and does not cross the C boundary.
- FFI memory API (third slice): build and read C structs and raw buffers so `import`ed functions can exchange structured data. `(struct 'name t…)` names a layout (a list of C type names); `(pack layout val…)` serialises values to a binary string laid out as that C struct (native alignment, padding, byte order) and `(unpack layout str)` reads them back. `(get-string addr [len [limit]])`, `get-int`, `get-long`, `get-float` (a C double), and `get-char` read a C value at an integer address; `(address 'sym)` returns the stable buffer address of a symbol-held string. A packed struct is handed to C by passing a string to a `void*` argument (no copy, binary-safe). A NULL (0) pointer through `unpack`/`get-string` raises an error instead of crashing.
- `pack`/`unpack` also accept a **format string** (the terse mini-language) in place of a struct: `c`/`b` (signed/unsigned 8-bit), `d`/`u` (16-bit), `ld`/`lu` (32-bit), `Ld`/`Lu` (64-bit), `f` (float), `lf` (double), `sN` (an N-byte string), `nN` (N null bytes), with `>`/`<` to switch to big-/little-endian for the following fields. Unlike a struct, a format string is packed tightly with no alignment — e.g. `(pack "c c c" 65 66 67)` -> `"ABC"`.

### Changed

- **Copy-on-write values** (ADR-0024): lists, arrays, and strings are now shared via `Rc` and copied only on mutation (`Rc::make_mut`). Storing or passing a large container is O(1) instead of a deep copy, while the value semantics are unchanged (a write to one owner never affects another). This removes an O(n²) blow-up when a loop reads a large container each iteration — e.g. a 100,000-element sieve dropped from ~40s to ~0.1s — with no change to the recursion benchmark.

### Fixed

- Implicit indexing: a **number** in functor position is now rest/slice, matching newLISP — `(2 lst)` is the tail from offset 2, `(2 3 lst)` takes 3 elements (a negative length counts from the end). Element access is `(lst i)` (a list/array in functor position), which is unchanged.
- `setf` into an indexed place (`(setf (v i) …)`) no longer copies the whole container to type-check it, so tight in-place mutation loops over a large list or array are no longer O(n²).
- `(apply f nil)` now treats `nil` as the empty list (the identity for the operation), rather than calling `f` with a single `nil` argument.

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

[Unreleased]: https://github.com/zonuexe/niiLISP/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.2.0
[0.1.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.1.0
