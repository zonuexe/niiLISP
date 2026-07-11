# niiLISP

niiLISP is a re-implementation of the [newLISP](https://en.wikipedia.org/wiki/NewLISP) dialect, written in Rust. Its overriding goal is compatibility with existing newLISP assets; practicality and learning come after.

niiLISP aims to reproduce newLISP's language semantics faithfully, including its One Reference Only (ORO) memory model, dynamic scoping and contexts, FOOP objects, and `import`/FFI. Design decisions are recorded as ADRs under [`docs/adr/`](docs/adr/), and the project's vocabulary is defined in [`CONTEXT.md`](CONTEXT.md).

This project is not affiliated with newLISP or Nuevatec. "newLISP" and "Nuevatec" are trademarks of Lutz Mueller.

## Status

Usable for real scripts. niiLISP has a reader (three string syntaxes, numbers, quote, comments), a tree-walking evaluator with dynamic scoping and **contexts as switchable namespaces** (`context`/`dotree`/`term`) that double as **dictionaries** (contexts-as-hashes), ORO value semantics with **copy-on-write** sharing, FOOP objects with reference `self`, `catch`/`throw`, and newLISP's reference/place model for destructive operations. Values include integers, IEEE-754 floats, arbitrary-precision **bigints**, binary-safe strings with **UTF-8 character operations** and **regular expressions** (RE2-style, with `$0..$N` capture variables and regex-mode `find`/`replace`) + Unicode case folding, lists, and fixed-length **arrays**. Lambdas are **list data** (build functions with `append`/`expand`/`args`), matching newLISP's code-as-data idiom. Beyond a large builtin set (arithmetic, comparisons, lists/arrays, higher-order, strings, `format`, bitwise, a seedable RNG, `parse`, `eval-string`) it has: `import`/**FFI** for calling C functions with the struct/`pack` memory API and `callback`s (Unix); **file I/O** and **external processes** (`process`/`exec`); the fork-based **Cilk API** and message passing (`spawn`/`sync`/`share`/`send`/`receive`/`signal`, Unix); **networking** (`net-*` stream sockets, Unix); and a native, `gs:`-inspired **GUI** helper (fltk, no JVM; behind an opt-in `gui` feature). It passes the vendored `qa-exception`, `qa-foop`, `qa-nullstring`, `qa-bigint`, `qa-longnum`, `qa-utf8`, `qa-utf8-ext`, `qa-dictionary`, `qa-cilk`, `qa-share`, `qa-pipefork`, `qa-message`, `qa-siguser`, and `qa-local-domain` suites. Not yet: UDP / HTTP / `net-eval` server mode, Windows FFI, and richer GUI widgets/layouts. The language niiLISP accepts is described in the specification under [`docs/spec/`](docs/spec/) (start at [`syntax.md`](docs/spec/syntax.md)); design decisions live under [`docs/adr/`](docs/adr/).

## Build

niiLISP is written in Rust and needs a recent stable toolchain (2021 edition).

```
cargo build --release      # binary at target/release/niilisp
cargo install --path .     # install `niilisp` into ~/.cargo/bin
cargo test                 # run unit and integration tests
```

The default build enables two features. `ffi` (`import`) links the system
libffi on Unix — install it if missing (`brew install libffi`, or
`apt-get install libffi-dev`); FFI is currently Unix-only. `bigint`
(arbitrary-precision integers) pulls in the pure-Rust `num-bigint`. For a pure,
dependency-free build without either, use `--no-default-features` (or enable just
one, e.g. `--no-default-features --features bigint`).

The interactive REPL has line editing via the `readline` feature (also on by
default): command history persisted to `~/.niilisp_history`, multi-line
continuation (an unclosed form keeps reading on the next line), matching-bracket
highlighting, and Tab completion over defined symbols. It uses the pure-Rust
`rustyline` — no system libreadline — so it cross-compiles like the rest.
`--no-default-features` drops it back to a plain line-at-a-time REPL.

## Usage

```
niilisp script.lsp              # run a script file
niilisp -e '(println (+ 1 2))'  # evaluate an expression
echo '(println 42)' | niilisp - # read a script from stdin
niilisp                         # start an interactive REPL
niilisp --help                  # print usage
niilisp --version               # print version, copyright, and a license pointer
niilisp license                 # print the open-source licenses (niilisp + deps)
```

## Examples

Runnable scripts live in [`examples/`](examples/):

```
niilisp examples/hello.lsp
niilisp examples/fib.lsp
niilisp examples/foop.lsp
```

## Embedding niiLISP in a Rust program

niiLISP is also a **library**: link it and run scripts in-process, with no
subprocess and no IPC ([ADR-0039](docs/adr/0039-embedding-library-target.md)).
Add it as a dependency and drive an `Interp`:

```rust
use niilisp::Interp;

let interp = Interp::new();
interp.eval_string(b"(define (square x) (* x x))").unwrap();
let v = interp.eval_string(b"(square 9)").unwrap();  // Result<Value, Signal>
println!("{}", interp.repr(&v));                     // => 81
```

The intended surface is `niilisp::{Interp, Value, Signal}`; see
[`examples/embed.rs`](examples/embed.rs) (`cargo run --example embed`). A few
caveats when embedding:

- **`(exit)` terminates the host process** (it calls `std::process::exit`). Don't
  run untrusted scripts that might call it.
- **Single-threaded** — `Interp` is `Rc`/`RefCell`-based (`!Send`/`!Sync`); use
  one interpreter per thread.
- **Default features touch the host OS** — the default build enables `mt` (a real
  `fork()` of the host for `spawn`/`process`), `net`, and `ffi`. For a sandboxed
  interpreter, depend with `default-features = false` and opt into only what you
  need (e.g. `bigint`, `regex`, `date`).
- The `0.x` API is unstable; pin an exact version.

## Copyright

```
niiLISP -- a re-implementation of the newLISP dialect.
Copyright (C) 2026  TypedDuck, USAMI Kenta <tadsan@zonu.me>
```

Portions of niiLISP are based on or adapted from newLISP:

```
newLISP
Copyright (C) Lutz Mueller <lutz@nuevatec.com>
Licensed under the GNU General Public License, version 3.
```

niiLISP is free software licensed under the GNU General Public License, version 3, or (at your option) any later version. See [`LICENSE.md`](LICENSE.md) for details, and [`COPYING`](COPYING) for the full license text.
