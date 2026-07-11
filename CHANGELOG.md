# Changelog

All notable changes to niiLISP are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Higher-order list-query builtins from the WikiBook "Lists" chapter: `(clean pred list)` (`filter` with a negated predicate), `(index pred list)` (the indices where `pred` holds), `(exists pred list)` (the first matching element, else `nil`), `(for-all pred list)` (`true` iff every element matches), and `(transpose matrix)` (swap rows and columns, padding ragged rows with `nil`).
- The `letn` (sequential-binding `let`, where each initializer sees the bindings made before it) and `letex` (`let` + `expand`: substitute the local values into the body before evaluating) special forms, both accepting the flat `(letn (s1 e1 …) …)` and fully-parenthesized `(letn ((s1 e1) …) …)` syntaxes with optional initializers.
- `curry`: `(curry func exp)` returns the one-argument partial application `(lambda ($x) (func exp $x))`. Like newLISP's, it does not evaluate its arguments — they are spliced literally into the lambda and evaluated only when it is applied.
- `global` and `global?`: `(global sym…)` declares MAIN symbols globally accessible from other contexts and returns the last (enabling the `(constant (global 'name) …)` idiom), and `(global? sym)` reports whether a symbol is global (a builtin, a special form, a context, or declared with `global`).
- `series`: `(series start factor count)` builds a geometric sequence (each term × `factor`), and `(series start func count)` builds a sequence where each term is `(func previous)`; `count < 1` yields the empty list.
- `factor`: `(factor int)` returns the prime factors of an integer, ascending with multiplicity (`(factor 12)` → `(2 2 3)`), over the full 64-bit range; floats are truncated first.
- **XML and JSON parsing** ([ADR-0038](docs/adr/0038-xml-and-json.md)): `xml-parse` (a hand-written parser for the well-formed XML 1.0 subset → newLISP's tagged-list tree `("ELEMENT" name attributes children)` / `("TEXT" str)` / …, with the `1`/`2`/`4`/`8`/`16` option flags and DTD/PI skipping), `xml-type-tags` (customize or suppress the four type tags), `xml-error`, `json-parse` (objects → association lists, arrays → lists, `false`/`null` → the symbols `false`/`null`), and `json-error`. Both are pure Rust with no dependency, always compiled in, and emit the exact newLISP representation so `assoc`/`lookup`/`ref` query the result unchanged. A malformed input returns `nil` and records a `("message" position)` pair for `xml-error`/`json-error`.
- **Dates and times** ([ADR-0037](docs/adr/0037-dates-and-times.md)): `date-value` (date components → UTC seconds since 1970, or the current time), `date-list` (UTC seconds → `(year month day hour min sec day-of-year day-of-week)`), `now` (11 local-time integers ending in the timezone offset and DST flag, with optional minute-offset and index), `date` (a local date/time string via `strftime`, with optional seconds, minute-offset, and format), and `date-parse` (a formatted string → UTC seconds). The UTC core (`date-value`/`date-list`) is pure Rust and always available; `date`/`now`'s local timezone and `date-parse` use the C library behind a default-on `date` feature (Unix), falling back to UTC / `nil` in the pure `--no-default-features` build (as `date-parse` is unavailable on Windows in newLISP too). `timer` is not yet implemented.
- The **reference/query family** ([ADR-0036](docs/adr/0036-reference-and-query-model.md)): `ref` (the index path to the first match, or `()`), `ref-all` (all index paths, or the elements with a trailing `true`; sets `$count`), `match` (the `?`/`+`/`*` wildcard matcher, nestable, returning the matched expressions or `nil`), `find-all` (regex-over-text, list-pattern, or key forms, each with an optional per-hit transform binding `$it`/`$0..$N`), and `pop-assoc` (remove and return the `(key …)` pair from an association-list place). `push`/`pop` now also accept an index *vector* (`(pop L (ref key L))` and `(push v L 1 0)`), so a `ref` path round-trips straight back into them. A comparison-function argument (`(ref key list compare)`) customizes matching; `set-ref`/`set-ref-all` remain available for deep replacement.

### Changed

- `let` now also accepts newLISP's fully-parenthesized binding form (`(let ((a 1) (b 2)) …)`) alongside the flat form, and a bare symbol in a binding list defaults to `nil` (`(let (y) …)`) instead of erroring.

## [0.3.2] - 2026-07-06

An audit of niiLISP against the *Introduction to newLISP* WikiBook (`docs/notes/20260706_wikibook-coverage/`) drives this release: it adds the `$idx` loop iterator, regex capture variables, and the `context` reflection forms, and corrects several builtins that behaved differently from newLISP.

### Added

- The **`$idx` system iterator** variable: `dolist`, `dostring`, `dotree`, `map`, and the `while`/`until`/`do-while`/`do-until` loops now maintain `$idx` as the current 0-based offset (e.g. `(map (fn (x) (list $idx x)) '(a b c))` → `((0 a) (1 b) (2 c))`), matching newLISP. It is dynamically scoped — saved on entry and restored on exit — so nested loops nest correctly.
- **Regex capture variables and regex-mode `find`/`replace`**: `regex`, `find` with an option argument, and `replace` now bind the system variables `$0` (whole match) and `$1..$N` (capture groups). `(find pattern str option)` engages the regex engine and returns the match offset. String `replace` re-evaluates its replacement expression **once per match** with `$0..$N` bound (`(replace "\\w+" s (upper-case $0) 0)` uppercases each word), in both the literal 3-argument form (the key is matched literally) and the 4-argument regex form.
- **`context` reflection and symbol-creation forms**: `(context)` with no argument returns the current context (tracked at eval time across `(context X)` switches), and `(context ctx word [value])` creates the symbol `ctx:word` — a `nil`-free way to build data structures — optionally setting its value and returning the symbol.

### Fixed

- `(save file)` with no symbol arguments now dumps the whole workspace (every user-defined MAIN symbol and context, excluding built-in primitives, `$`-system symbols, and unset symbols) as loadable source, instead of writing an empty file; `(save file sym…)` still dumps just the named symbols.
- `int` returns `nil` (or its `default` argument) instead of a silent `0` when a string can't be converted, and parses `0x`/`0b`/`0o` prefixes and an explicit `base` argument (`(int "0x1F")` → `31`, `(int "FF" 0 16)` → `255`).
- `dup`'s third argument replicates into a list (`(dup "x" 3 true)` → `("x" "x" "x")`), and `<<`/`>>` accept the 1-argument shift-by-one form (`(<< 6)` → `12`) and fold multiple shift counts (`(<< 1 2 3)` → `32`).
- `round` follows newLISP's inverted digit-sign convention: a positive count rounds the integer part, a negative count rounds decimal places (`(round 123.49 2)` → `100`, `(round 123.49 -1)` → `123.5`).
- `time-of-day` returns milliseconds since midnight (`epoch % 86400000`) rather than the raw epoch.

## [0.3.1] - 2026-07-06

### Fixed

- The Windows build failed to compile (`cargo install niilisp` on Windows, and the pure `--no-default-features` build): a `#[cfg(not(unix))]` branch of `file-info` bound a generic function without type annotations (`E0283`). That branch never compiles on Unix, so it slipped past the macOS/Linux CI; the 0.3.0 crate is unaffected on Unix.

## [0.3.0] - 2026-07-06

### Added

- **File I/O** (ADR-0029), always compiled in (pure `std::fs`): file **handles** are opaque integers from an interpreter registry (`0`/`1`/`2` are stdin/stdout/stderr) — `(open path mode)` (`read`/`write`/`append`/`update`), `close`, `seek` (`-1` = end; 64-bit), `(read-buffer handle place size [wait])` (a place-taking special form) and `(write-buffer handle str [size])`, `read-line`/`current-line`. Whole-file `read-file`/`write-file`/`append-file`. Filesystem `directory`, `real-path`, `make-dir`, `remove-dir`, `change-dir`, `rename-file`, `delete-file`, `file?`, `directory?`, `file-info` (a 10-int list), and `env`. Persistence `source`/`save`/`load`. Operational failures return `nil`; paths are byte-buffer strings mapped to OS-native (binary-safe on Unix; faithful Windows UTF-16 paths deferred).
- **Dictionaries** (ADR-0030): a context whose default functor `Ctx:Ctx` is **nil** acts as a string/number-keyed hash — `(Ctx key value)` sets (a nil value deletes), `(Ctx key)` gets, `(Ctx assoc-list)` bulk-loads, `(Ctx)` returns all pairs sorted. Keys are stored as `_`-prefixed context symbols. `(new prototype 'name)` now **copies** the prototype's symbols (its functor `Proto:Proto` → `name:name`), and a predefined `Class` marker gives FOOP classes a non-nil functor — so object construction (a non-nil functor) stays distinct from Dictionary access (a nil functor), matching newLISP. Adds `delete` (a symbol, or a whole context), `sys-info` (best-effort stats), and `randomize` (shuffle).
- **Native GUI, first slice** (ADR-0034): a `gs:`-inspired GUI that needs no JVM. `lib/gui.lsp` (`gs:init`/`gs:frame`/`gs:label`/`gs:button`/`gs:text-field`/`gs:set-text`/`gs:set-background`/`gs:set-visible`/`gs:listen`/`gs:check-event`) drives a separate `niilisp-gui` fltk helper binary over a full-duplex TCP socket (reusing `net-*` + `eval-string`); commands are space-separated tokens with base64 text, events are niiLISP source lines the loop `eval-string`s. Behind a **default-off `gui` feature**; `fltk` (bundled, no system toolkit needed) links only into the helper, never the interpreter. The API keeps `gs:` names/shapes close to newLISP-GS so scripts port with few edits, but makes no behaviour guarantee (a different toolkit than Swing). The GUI is display-dependent, so CI covers only the protocol layer (`tests/gui.rs`); `examples/gui-demo.lsp` is the manual demo. Widget layout is a vertical auto-stack for now; richer layouts, `gs:canvas`/drawing, and more widgets are later slices.
- **Networking** — `net-*` stream sockets (ADR-0033), behind a default-on `net` Cargo feature, Unix-only: `(net-connect host port)` / `(net-connect "/path")` and `(net-listen port)` / `(net-listen "/path")` (TCP or Unix-domain), `(net-accept lsock)`, `(net-send sock str)`, `(net-receive sock place maxlen [wait])` (like `read-buffer`), `(net-select sock "read"/"write" ms)`, `(net-peek sock)`, `(net-peer sock)` / `(net-local sock)`, `(net-close sock)`. A socket is a file handle (a raw fd), so send/receive/close reuse the file-I/O machinery. Sockets are blocking; `net-select` polls for readiness. Passes the vendored `qa-local-domain` (the connect/listen/accept/send/receive/select surface newLISP-GS uses). Deferred: UDP, `net-eval`/server mode, raw `net-packet`/`net-ping`, `net-lookup`, and `get-url`/HTTP.
- **External processes** (ADR-0031), always compiled in and cross-platform (`std::process::Command`): `(process "cmd arg…")` spawns a command non-blocking and returns its pid; `(exec "cmd" [instr])` runs it through the shell and returns stdout as a list of lines (feeding `instr` to stdin); `(! "cmd")` runs it through the shell and returns the exit code; `(sleep ms)` pauses.
- **Cilk API / fork multitasking** (ADR-0032), behind a default-on `mt` Cargo feature, Unix-only (real `fork()` of the interpreter via `libc`): `(spawn 'sym expr [msg])` forks a child to evaluate `expr` in parallel, `(sync [timeout [inlet]])` waits and binds each child's result to its symbol (calling an optional inlet per finish), `(abort [pid])` cancels children; `(share)` / `(share adr val)` / `(share adr)` exchange a value through an `mmap`ed shared page; the message API `(send pid msg)` / `(receive)` / `(receive pid place)` over per-child datagram socketpairs; raw `(fork expr)`, `(pipe)` (returns two file handles), `(wait-pid pid)`, and `(signal n handler)` (an async-safe handler run at eval safe points). `sys-info -3`/`-4` give this/parent pid. Cross-process values transfer as their re-readable `repr`. Passes the vendored `qa-cilk`, `qa-share`, `qa-pipefork`, `qa-message`, and `qa-siguser` (`qa-msgbig` needs `base64-enc` + stream framing for its 80 KB messages). Adds `write-line` (always-on) and the `do-until`/`do-while` post-test loops.
- **Regular expressions** (ADR-0028): `(regex pattern text [option [offset]])` returns the first match over the byte string as `(match byte-offset byte-length …captures…)` or `nil`, and `(regex-comp pattern [option])` precompiles (and caches) a pattern. Uses the pure-Rust `regex` crate (RE2-style) behind a default-on `regex` Cargo feature — so the common regex vocabulary (classes, quantifiers, groups, alternation, anchors) works, but **backreferences and lookaround are not supported** (unlike newLISP's PCRE). PCRE option bits are mapped (case-insensitive, multi-line, dot-all; the UTF-8 bit is a no-op since matching is Unicode by default).
- **`niilisp --version` and `niilisp license`**: `--version` / `version` prints a compact banner (name, version, a build stamp of the git revision + date, repo, copyright, and the GPL notice); `license` / `licenses` prints niilisp's own GPL-3.0-or-later notice plus every bundled dependency's, from an embedded `THIRD-PARTY-LICENSES.md`. A `build.rs` supplies the stamp (honouring `SOURCE_DATE_EPOCH`); the notices are generated by `cargo about` and gated in CI by `cargo deny check licenses` plus a regenerate-and-diff drift guard.
- `eval-string`: `(eval-string str [error-value])` reads `str` as code and evaluates its forms in the current dynamic environment, returning the last result (a second argument is returned instead of raising on error). Completes the newLISP-GS substrate (its inbound events are dispatched by `eval-string`).
- More standard builtins (self-contained newLISP-faithful fill-ins): rounding and sign — `ceil`, `floor`, `round` (optional decimal places), `sgn` (with optional branch values); the hyperbolic/`atan2` trig — `sinh`/`cosh`/`tanh`/`asinh`/`acosh`/`atanh`/`atan2`; `bits` (an integer's binary-digit string); `base64-enc`/`base64-dec` (standard, binary-safe, no dependency); `parse` (split a string on whitespace, a literal separator, or a regex); the list operations `count`, `select`, `difference`, `intersect`; the reflection predicates `context?`, `lambda?`, `macro?`, `primitive?`, `bigint?`, `protected?`; and `title-case`.

### Changed

- Strings now print in a **re-readable, binary-safe** form (ADR-0032): the printer escapes `"`, `\`, and control bytes (`\n`/`\t`/`\r` and `\NNN` for the rest) while keeping valid UTF-8 text literal, so a string with quotes, control, or non-UTF-8 bytes round-trips through `save`/`source` and the REPL (the reader already parsed these escapes). Only the re-readable form (`source`/`save`/REPL echo) changes; `print`/`println` still emit raw bytes.
- `upper-case`/`lower-case` now perform **Unicode** case folding (ADR-0028), not just ASCII: each valid UTF-8 character is mapped with Unicode default case rules (so Cyrillic, Greek, etc. fold, and `ß` → `SS`), while invalid bytes pass through. ASCII behaviour is unchanged.
- Applying a context now dispatches on its default functor `Ctx:Ctx` (ADR-0030): a **lambda** is called (FOOP constructor), **nil** makes it a Dictionary, any other non-nil value builds a tagged object list. This replaces the previous "implicit construction" fallback (a context with no lambda functor always built a tagged list); FOOP classes now get a non-nil functor from the predefined `Class` (copied by `new`), so existing FOOP code is unaffected while nil-functor contexts become dictionaries.
- `dostring` now binds its loop variable to each **UTF-8 character's code point** rather than each byte (ADR-0025), matching newLISP's UTF-8 build — so `(dostring (c "我") (char c))` round-trips the character. For an ASCII string a code point equals its byte value, so existing behaviour is unchanged. Unlocks the vendored `qa-utf8-ext` suite.

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

[Unreleased]: https://github.com/zonuexe/niiLISP/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/zonuexe/niiLISP/releases/tag/v0.3.2
[0.3.1]: https://github.com/zonuexe/niiLISP/releases/tag/v0.3.1
[0.3.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.3.0
[0.2.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.2.0
[0.1.0]: https://github.com/zonuexe/niiLISP/releases/tag/v0.1.0
