# FFI callback slice: libffi closures with reentry via userdata

The second FFI slice (after ADR-0019) adds `callback` (CONTEXT.md: callback): a
C-callable function pointer that trampolines into a niiLISP function, so C
libraries can call back into Lisp (GLUT handlers, `qsort` comparators, ...).

## Scope

- **In:** `(callback 'func "ret-type" "arg-type"...)` creates a libffi closure
  and returns its code-pointer **address as an `Int`** (to hand to a C API). The
  type set is the first slice's (ADR-0019): `void`/`int`/`long`/`float`/`double`/
  `char*`/`void*`.
- **Deferred:** the simple indexed form `(callback idx 'func)` (newLISP's
  fixed-pool API).

## Reentry (ADR-0019 crux)

The libffi closure's **userdata holds `*const Interp`** plus the target function
value and the argument/return `CType`s. The trampoline (`extern "C" fn`)
reconstructs `&Interp` — sound because there is a single interpreter that lives
for the whole process — then marshals the C arguments into `Value`s, evaluates
`func`, and marshals the result back to C.

- **Invariant: no `RefCell` borrow is held across a foreign call that can
  re-enter.** Otherwise the callback's `eval` would double-borrow `globals` and
  panic. The evaluator already evaluates arguments before making a foreign call,
  so no borrow is live during the call.

## Lifetime

Created closures (and their userdata) are **kept alive for the process lifetime**
— stored in the interpreter, never freed — exactly like library handles
(ADR-0019). A dropped closure would leave C holding a dangling function pointer.
Value-tying the closure is rejected: the return is an `Int` address that cannot
own the closure, and ORO would drop copies unpredictably while C still holds the
pointer. Optional dedup by `(function symbol, signature)` is a later refinement
for programs that recreate callbacks in a loop.

## Error boundary (ADR-0015)

A `throw`/error inside the callback is **caught at the trampoline, printed to
stderr, and the zero-value of the declared return type is returned to C** (`0` /
`NULL` / nothing). Exceptions never cross the C frames; failures stay visible.

## Marshalling

The first slice's `CType` marshalling, **reversed at the boundary**: C arguments
are decoded to `Value`s on entry, and the `Value` result is encoded to the C
return on exit.

## Acceptance

Extend the hermetic FFI test (`tests/ffi.rs`) with a C function that takes and
invokes a function pointer (e.g. `int apply_cb(int (*f)(int), int x)`), verifying
the niiLISP callback runs and returns the expected value.

## Consequences

- The interpreter gains a closures store (cfg `all(feature = "ffi", unix)`),
  alongside the library cache.
- The trampoline is `unsafe`, confined to the FFI module (ADR-0015).
- The "no borrow across a foreign call" invariant is now load-bearing for
  soundness and should be preserved as the evaluator evolves.
