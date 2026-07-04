# niiLISP

niiLISP is a re-implementation of the [newLISP](https://en.wikipedia.org/wiki/NewLISP) dialect, written in Rust. Its overriding goal is compatibility with existing newLISP assets; practicality and learning come after.

niiLISP aims to reproduce newLISP's language semantics faithfully, including its One Reference Only (ORO) memory model, dynamic scoping and contexts, FOOP objects, and `import`/FFI. Design decisions are recorded as ADRs under [`docs/adr/`](docs/adr/), and the project's vocabulary is defined in [`CONTEXT.md`](CONTEXT.md).

This project is not affiliated with newLISP or Nuevatec. "newLISP" and "Nuevatec" are trademarks of Lutz Mueller.

## Status

Early but usable for small scripts. niiLISP has a reader (three string syntaxes, numbers, quote, comments), a tree-walking evaluator with dynamic scoping and contexts, ORO-style value semantics, FOOP objects with reference `self`, `catch`/`throw`, newLISP's reference/place model for destructive operations, a growing set of builtins (integer and float arithmetic, comparisons, lists, higher-order functions, strings, `format`, bitwise), and a first slice of `import`/FFI for calling C functions (Unix). It passes the vendored `qa-exception` and `qa-foop` suites. Not yet implemented: FFI callbacks and the memory/struct API, networking, bigint, and full UTF-8 character operations. See [`docs/adr/`](docs/adr/) for the design and scope.

## Build

niiLISP is written in Rust and needs a recent stable toolchain (2021 edition).

```
cargo build --release      # binary at target/release/niilisp
cargo install --path .     # install `niilisp` into ~/.cargo/bin
cargo test                 # run unit and integration tests
```

The default build enables the `ffi` feature (`import`), which links the system
libffi on Unix — install it if missing (`brew install libffi`, or
`apt-get install libffi-dev`). For a pure, dependency-free build without `import`,
use `--no-default-features`. FFI is currently Unix-only.

## Usage

```
niilisp script.lsp              # run a script file
niilisp -e '(println (+ 1 2))'  # evaluate an expression
echo '(println 42)' | niilisp - # read a script from stdin
niilisp                         # start an interactive REPL
niilisp --help                  # print usage
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
