# niiLISP

niiLISP is a re-implementation of the [newLISP](https://en.wikipedia.org/wiki/NewLISP) dialect, written in Rust. Its overriding goal is compatibility with existing newLISP assets; practicality and learning come after.

niiLISP aims to reproduce newLISP's language semantics faithfully, including its One Reference Only (ORO) memory model, dynamic scoping and contexts, FOOP objects, and `import`/FFI. Design decisions are recorded as ADRs under [`docs/adr/`](docs/adr/), and the project's vocabulary is defined in [`CONTEXT.md`](CONTEXT.md).

This project is not affiliated with newLISP or Nuevatec. "newLISP" and "Nuevatec" are trademarks of Lutz Mueller.

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
