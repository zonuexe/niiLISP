# niiLISP

niiLISP is a re-implementation of the [newLISP](https://en.wikipedia.org/wiki/NewLISP) dialect, written in Rust. Its overriding goal is compatibility with existing newLISP assets; practicality and learning come after.

niiLISP aims to reproduce newLISP's language semantics faithfully, including its One Reference Only (ORO) memory model, dynamic scoping and contexts, FOOP objects, and `import`/FFI. Design decisions are recorded as ADRs under [`docs/adr/`](docs/adr/), and the project's vocabulary is defined in [`CONTEXT.md`](CONTEXT.md).

This project is not affiliated with newLISP or Nuevatec. "newLISP" and "Nuevatec" are trademarks of Lutz Mueller.

## Status

Usable for small scripts. niiLISP has a reader (three string syntaxes, numbers, quote, comments), a tree-walking evaluator with dynamic scoping and **contexts as switchable namespaces** (`context`/`dotree`/`term`), ORO value semantics with **copy-on-write** sharing, FOOP objects with reference `self`, `catch`/`throw`, and newLISP's reference/place model for destructive operations. Values include integers, IEEE-754 floats, arbitrary-precision **bigints**, binary-safe strings with **UTF-8 character operations**, lists, and fixed-length **arrays**. Lambdas are **list data** (build functions with `append`/`expand`/`args`), matching newLISP's code-as-data idiom. It has a large builtin set (arithmetic, comparisons, lists/arrays, higher-order, strings, `format`, bitwise, a seedable RNG) and `import`/**FFI** for calling C functions with the struct/`pack` memory API and `callback`s (Unix). It passes the vendored `qa-exception`, `qa-foop`, `qa-nullstring`, `qa-bigint`, `qa-longnum`, and `qa-utf8` suites. Not yet implemented: networking, regular expressions, Unicode case folding, and Windows FFI. The language niiLISP accepts is described in the specification under [`docs/spec/`](docs/spec/) (start at [`syntax.md`](docs/spec/syntax.md)); design decisions live under [`docs/adr/`](docs/adr/).

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

## Usage

```
niilisp script.lsp              # run a script file
niilisp -e '(println (+ 1 2))'  # evaluate an expression
echo '(println 42)' | niilisp - # read a script from stdin
niilisp                         # start an interactive REPL
niilisp --help                  # print usage
niilisp --version               # print version
```

## Examples

Runnable scripts live in [`examples/`](examples/):

```
niilisp examples/hello.lsp
niilisp examples/fib.lsp
niilisp examples/foop.lsp
```

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
