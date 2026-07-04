# FFI first slice: typed `import` of scalar / string / pointer functions

The first shippable piece of `import`/FFI (ADR-0015, built per ADR-0018) is the
**extended typed `import`** for ordinary C functions — enough to call libc/libm
routines (`cos`, `pow`, `strlen`, `getenv`, ...) with declared types.

## Scope

- **In:** `(import "lib" "fn" "ret-type" "arg-type"...)` for the types **`void`,
  `int`, `long`, `float`, `double`, `char*`, `void*`**, and calling the resulting
  function.
- **Deferred to later slices:** `callback` (libffi closures), the memory API
  (`pack`/`unpack`/`get-*`/`struct`/`address`), simple/untyped `import`, and
  struct-by-value.

## Foreign function value (CONTEXT.md: Foreign function)

- A new value variant **`Value::Foreign(Rc<ForeignFn>)`** holds the resolved code
  pointer and the libffi CIF (argument + return types).
- **`import`** is a builtin (gated on the `ffi` feature, ADR-0018) that: `dlopen`s
  the library and **caches the handle in the interpreter, kept open for the
  process lifetime** (never `dlclose` — calling a function pointer after unload is
  UB, and newLISP also keeps libraries loaded); `dlsym`s the symbol; builds the
  CIF; binds the `Foreign` under the C function's name; and returns it. On failure
  (library or symbol not found) it returns **`nil`**, matching the newLISP
  `(if (import …) …)` idiom.

## Marshalling

- `int`/`long` <-> `Int`; `float`/`double` <-> `Float` (Int coerced as needed).
- **`char*` argument:** niiLISP strings are not NUL-terminated, so a **temporary
  NUL-terminated copy** of the `Str` bytes is passed for the duration of the call.
- **`char*` return:** copied out to a `Str` up to the NUL terminator and **not
  freed** (the callee owns it; matches newLISP); a NULL return is `nil`.
- `void*` and opaque handles are **`Int` addresses** (as in newLISP); `void`
  return is `nil`.

## Platform and acceptance

- **Platform:** compiles on Unix (`.so`/`.dylib`) and Windows (`.dll`) via
  `libloading`; library names are caller-provided (no name munging). Tested on
  Unix first.
- **Acceptance:** a hermetic integration test (`tests/ffi.rs`, `#[cfg(feature =
  "ffi")]`) compiles a tiny C shared library with `cc` at test time, then imports
  and calls it. The vendored `qa-libffi` waits on `exec`/`real-path`/file I/O
  builtins (ADR-0009's staged approach).

## Consequences

- The `Value::Foreign` variant is **always present** in the enum (so `match`
  arms are not `cfg`-gated everywhere); only the `import` builtin and the libffi
  call path are behind `#[cfg(feature = "ffi")]`. A `--no-default-features` build
  therefore never constructs a `Foreign` and stays 100% safe Rust.
- `unsafe` is confined to the FFI module (ADR-0015); the marshalling and call
  path are the only unsafe code.
- The next FFI slices (`callback`, memory API) build on this `Foreign`/CIF
  machinery.
