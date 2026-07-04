# niiLISP

A re-implementation of the newLISP dialect. The project's overriding goal is compatibility with existing newLISP assets; practicality and learning are secondary. This glossary fixes the vocabulary the project uses when talking about that dialect and its compatibility surface.

## Language

**niiLISP**:
The interpreter being built in this repository — a re-implementation of the newLISP dialect.
_Avoid_: "the interpreter", "our lisp"

**newLISP**:
The reference dialect being re-implemented, as defined by the kosh04 fork vendored under `references/newlisp`. When the project says "compatible", it means compatible with this reference unless stated otherwise.
_Avoid_: "upstream", "original lisp"

**Source compatibility**:
The property that a newLISP `.lsp` script runs on niiLISP with its observable behaviour preserved. This is the value the project is ultimately buying — not binary or ABI compatibility.
_Avoid_: "script compat", "language compat"

**import compatibility**:
The specific compatibility surface the project targets first: the `import` facility (FFI to shared libraries) behaves as in newLISP, so that newLISP modules built on `import` load and run. Distinct from native C-API/ABI compatibility, which is out of scope for now.
_Avoid_: "FFI compat" (use this precise term), "C compat"

**callback**:
A C-callable function pointer, created by `(callback 'lisp-func …)`, that trampolines into a niiLISP function so C libraries can call back into Lisp (e.g. GLUT display/idle handlers). Implemented with libffi closures.
_Avoid_: "hook", "handler", "function pointer" (bare)

**Module (newLISP sense)**:
A newLISP `.lsp` file that exposes functionality, usually by calling into an external shared library via `import`. The dominant real-world form of newLISP "module". Distinct from a native C extension that links against newLISP's own cell ABI.
_Avoid_: "library", "package", "plugin"

**Context**:
A newLISP namespace: a named table of symbols that owns its symbols' values. The default context is `MAIN`. Contexts double as the substrate for prototype-based objects (FOOP) and modules. niiLISP reproduces contexts as first-class namespaces.
_Avoid_: "namespace" (use Context), "module", "package", "object"

**Dynamic scoping**:
niiLISP's scoping rule, inherited from newLISP: a called function sees the caller's current bindings for any symbol it does not itself rebind. Bindings are established by saving a symbol's current value, installing a new one, and restoring it as evaluation unwinds. There is no lexical/closure capture.
_Avoid_: "lexical scope", "closure environment"

**fexpr**:
A callable that receives its arguments **unevaluated**, as list data, and decides itself whether/when to evaluate them. In newLISP this is the `lambda-macro` / `define-macro` form (a runtime, non-hygienic fexpr — not a compile-time macro). niiLISP reproduces this evaluation-time, unevaluated-argument semantics.
_Avoid_: "macro" (misleading — these are runtime fexprs, not hygienic/expansion macros)

**FOOP**:
newLISP's object style, reproduced by niiLISP: an object is an ordinary list whose head element is its class Context's symbol (e.g. `(complex 3 4)`); methods are functions in that Context; `self` is the target object inside a method, and `(self N)` indexes it as a list. Objects are a **convention over lists**, not a distinct value type.
_Avoid_: "class instance", "object type"

**Default functor**:
The symbol inside a Context that shares the Context's name (`Ctx:Ctx`). Applying a Context as a function invokes it; in FOOP it is the constructor that builds the tagged object list.
_Avoid_: "constructor" alone (it is the general "apply a context" hook), "main function"

**Colon dispatch**:
The `(:method obj …)` form: `:method` resolves the method at runtime from the class tag at the head of `obj`, giving polymorphism. Distinct from `Ctx:sym`, which names a specific symbol in a specific Context.
_Avoid_: "method call", "message send"

**String (byte buffer)**:
A niiLISP string is a **binary-safe sequence of bytes** (may hold arbitrary/invalid-UTF-8 bytes), not a guaranteed-UTF-8 text value. It doubles as the byte buffer for I/O and FFI. `length` counts bytes; `utf8len` counts characters. Character-oriented operations decode UTF-8 on demand over this byte storage.
_Avoid_: "text", "UTF-8 string" (misleading — storage is bytes, not validated UTF-8)

**Cilk API (`spawn`/`sync`)**:
newLISP's high-level parallelism: `spawn` evaluates an expression in a child process, binding its result to a variable later; `sync` waits for spawned children and collects results. Built on `fork`. niiLISP reproduces it on real OS processes.
_Avoid_: "threads", "async tasks", "futures"

**share**:
newLISP's OS-shared-memory cell for exchanging a single value between processes: `(share)` allocates, `(share adr val)` writes, `(share adr)` reads. Reproduced with real shared memory, consistent with the process model.
_Avoid_: "shared variable", "global"

**ORO (One Reference Only)**:
newLISP's GC-free memory model, which niiLISP commits to reproducing: every value has exactly one owner, values are deep-copied when stored in a structure or passed to a function, and memory is freed stack-wise as evaluation unwinds. Cyclic references cannot arise by construction.
_Avoid_: "garbage collection", "refcounting", "ownership model"

**Native C API**:
newLISP's embedding API (`newlisp.h`) and the cell-struct ABI that native C extensions link against. Explicitly **not** a compatibility target at this stage; recorded here only to name what `import compatibility` is *not*.
_Avoid_: "C API" (ambiguous — always qualify as native)
