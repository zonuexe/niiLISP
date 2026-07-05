# FFI memory API slice: struct-based pack/unpack, get-*, address

The third FFI slice (after ADR-0019 import, ADR-0020 callback) adds the memory
API: building and reading C structs and raw buffers, so `import`ed functions can
exchange structured data. Acceptance target: the vendored `qa-nullstring`.

## Scope

- **In:** `struct`, `pack`/`unpack`, the `get-*` readers (`get-string`,
  `get-int`, `get-long`, `get-float`, `get-char`), and `address`.
- **Deferred:** the terse `pack` format-char mini-language (`c b d lf n s …`) and
  its endianness toggles (`>` / `<`).

## struct and pack/unpack

A **struct** (CONTEXT.md: struct (FFI layout)) is a **list of C type names** —
the same set as `import` (ADR-0019): `int long float double char* void*`.
`(struct 'name t…)` binds `name` to that list; it introduces no new value type
(a struct is just a list of type strings).

- `(pack layout val…)` serialises the values to a binary niiLISP **string**
  (bytes), and `(unpack layout str)` returns a **list** of values. `layout` is a
  struct (list of type names).
- **Layout is the native C ABI:** each field is placed at its natural alignment
  with inter-field and trailing padding, in native byte order, so a packed
  string is exactly what a C function accepts as that struct. (Endian toggles are
  deferred with the format-char language.)

## Passing raw buffers to C under ORO

ORO copies values, so exposing a stable buffer pointer is the hard part. Two
paths (both chosen):

- **Synchronous:** `import`'s `void*` argument accepts a **string** and passes
  its buffer pointer directly — no copy, binary-safe — valid for the duration of
  the call (the argument is kept alive during it). This is how a packed struct is
  handed to C.
- **Persistent / write-through:** `(address 'sym)` (CONTEXT.md: address (FFI))
  returns the **stable buffer address of a symbol-held value**. It is valid only
  while the symbol is neither reassigned nor resized. **Invariant:** do not
  resize or reassign the value while C holds the address. Taking the address of
  an arbitrary evaluated value is **rejected** — under ORO the temporary copy is
  dropped immediately, so the address would dangle at once.

## NULL and invalid pointers

`get-*` and `unpack` (for a pointer/`char*` field) **check for a null (0)
address before dereferencing and raise an error** ("cannot convert NULL to
string"), rather than crashing. Other invalid addresses cannot be validated and
are undefined behaviour — the caller's risk — consistent with newLISP and
ADR-0015 ("`import` can crash the interpreter"). This is exactly what
`qa-nullstring` exercises.

## Acceptance

- **`qa-nullstring`**: `struct` + `pack` + `unpack` + `get-string`, with a NULL
  `char*`/address producing an error.
- A hermetic test: pack a struct, pass it to a C function via `void*`, and read
  results back with `get-*` / `unpack`.

## Consequences

- Reuses `CType` and the ADR-0019 marshalling; `void*` argument handling gains a
  string case; `get-*`/`pack`/`unpack`/`address`/`struct` are builtins gated on
  `cfg(all(feature = "ffi", unix))`.
- All new `unsafe` (reading C memory, exposing addresses) stays confined to the
  FFI module.
- The "do not resize/reassign while C holds an address" invariant is
  load-bearing for `address` soundness and must be documented for users.
