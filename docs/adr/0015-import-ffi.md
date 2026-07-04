# import/FFI: `libloading` + `libffi`, typed import first

`import` (CONTEXT.md: import compatibility) is the headline compatibility surface (ADR-0001) and the v2 milestone (ADR-0008). It is implemented on mature Rust crates: **`libloading`** for runtime shared-library load + symbol resolution (dlopen/dlsym), and the **`libffi`** crate for runtime-typed dynamic calls and for `callback` (CONTEXT.md: callback) via libffi closures. This is the same mature dynamic-FFI path whose absence disqualified MoonBit (ADR-0004).

## Forms (verified against `qa-libffi`)

- **Extended/typed import is the v2 core:** `(import "lib" "fn" "ret-type" "arg-type"…)` with types `"int"`, `"long"`, `"float"`, `"double"`, `"char*"`, `"void*"`, `"void"`, etc. This is what `qa-libffi` targets and the portable, correct form.
- **Simple/untyped import** is a best-effort follow-on: a thin wrapper that supplies default marshalling, for old code that omits type strings.

## Marshalling (pays off ADR-0013)

- **Strings** (`Vec<u8>`, ADR-0013) map to `char*`/`void*` **directly — binary-safe, no validation or copy**. This is precisely why strings are byte buffers, not `String`.
- `Int` ↔ `int`/`long`, `Float` ↔ `float`/`double`, **pointers are integers** (addresses), and `pack`/`unpack`/`get-*` read/write C layouts over byte-buffer strings.

## callback error boundary (design constraint)

A Lisp `throw`/error raised **inside** a callback cannot unwind through the C frames — the `Result<_, Signal>` mechanism (ADR-0011) does not cross the C boundary. So each callback trampoline **wraps the Lisp call in an implicit `catch`** and converts any escape into a safe C-level return value; exceptions never propagate through C. This is a hard rule, not an option.

## Safety

FFI is inherently `unsafe`: `import` can crash the interpreter exactly as in newLISP. That is accepted as the cost of compatibility (ADR-0001). `unsafe` is confined to the FFI module; the rest of the interpreter keeps Rust's guarantees.

## Consequences

- v2 depends on a working v1 core (evaluator, string/number/list) before any of this lands.
- The callback trampoline and `fork`-based concurrency (ADR-0014) both re-enter the single-threaded evaluator; neither may introduce background threads that would violate the fork-and-continue invariant.
- Oracles: `qa-libffi`, `qa-libc-libffi`, and `qa-win-dll` (ADR-0009's deferred FFI bucket).
