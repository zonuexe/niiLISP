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

**Foreign function**:
A niiLISP value that wraps a C function resolved through `import`: the resolved code pointer plus its declared argument and return types. It is callable like any other function, and `import` binds it under the C function's name.
_Avoid_: "native function" (reserved for newLISP's cell-ABI extensions), "external function"

**struct (FFI layout)**:
A C struct layout for the FFI memory API: a named list of C type names (the same set as `import`) describing a record. `pack`/`unpack` use it to serialise values to and from the native C ABI byte layout (alignment and padding included).
_Avoid_: "record", "format" (reserved for the deferred pack format-char language)

**address (FFI)**:
The `(address 'sym)` operation: the stable memory address of a symbol-held value's buffer, for handing to C. Valid only while the symbol is neither reassigned nor resized; taking the address of an arbitrary temporary value is unsafe under ORO and disallowed.
_Avoid_: "pointer" (bare — an address is an integer)

**Module (newLISP sense)**:
A newLISP `.lsp` file that exposes functionality, usually by calling into an external shared library via `import`. The dominant real-world form of newLISP "module". Distinct from a native C extension that links against newLISP's own cell ABI.
_Avoid_: "library", "package", "plugin"

**Context**:
A newLISP namespace: a named table of symbols that owns its symbols' values. The default context is `MAIN`. Contexts double as the substrate for prototype-based objects (FOOP) and modules. niiLISP reproduces contexts as first-class namespaces.
_Avoid_: "namespace" (use Context), "module", "package", "object"

**Current context**:
The context in effect while a source is read: `(context 'X)` makes `X` current so an unqualified symbol read after it is created in `X` (`X:sym`), except names that already exist as MAIN primitives. It is a **read-time** property — a symbol's context is fixed when read, not resolved at evaluation (ADR-0026). `MAIN` is the current context until switched.
_Avoid_: "active namespace", "scope" (scope is dynamic binding, a different axis)

**Dynamic scoping**:
niiLISP's scoping rule, inherited from newLISP: a called function sees the caller's current bindings for any symbol it does not itself rebind. Bindings are established by saving a symbol's current value, installing a new one, and restoring it as evaluation unwinds. There is no lexical/closure capture.
_Avoid_: "lexical scope", "closure environment"

**fexpr**:
A callable that receives its arguments **unevaluated**, as list data, and decides itself whether/when to evaluate them. In newLISP this is the `lambda-macro` / `define-macro` form (a runtime, non-hygienic fexpr — not a compile-time macro). niiLISP reproduces this evaluation-time, unevaluated-argument semantics.

**Lambda list (open lambda)**:
newLISP's property that a lambda *is a list* — `(lambda (params) body…)` — so it can be built, indexed, and traversed with ordinary list operations, and code can construct functions as data. niiLISP keeps a compact internal function value but **presents this list interface on demand** (ADR-0027): list operations see a lambda as its list form, and a list whose head is `lambda`/`fn`/`lambda-macro` is callable.
_Avoid_: "closure" (there is no lexical capture), "function object" (obscures the list nature)
_Avoid_: "macro" (misleading — these are runtime fexprs, not hygienic/expansion macros)

**FOOP**:
newLISP's object style, reproduced by niiLISP: an object is an ordinary list whose head element is its class Context's symbol (e.g. `(complex 3 4)`); methods are functions in that Context; `self` is the target object inside a method, and `(self N)` indexes it as a list. Objects are a **convention over lists**, not a distinct value type.
_Avoid_: "class instance", "object type"

**Default functor**:
The symbol inside a Context that shares the Context's name (`Ctx:Ctx`). Applying a Context as a function dispatches on it: if it is a **lambda** it is called (in FOOP, the constructor that builds the tagged object list); if it is **nil** the Context behaves as a **Dictionary** (ADR-0030). There is no third "implicit construction" path — FOOP always rides a lambda functor (the predefined `Class`, or a user constructor).
_Avoid_: "constructor" alone (it is the general "apply a context" hook), "main function"

**Class (predefined)**:
The FOOP base Context predefined at startup as `(define (Class:Class) (cons (context) (args)))`, mirroring newLISP's built-in. `(new Class 'A)` copies its constructor so `(A …)` builds a tagged object list. Its only role is to give FOOP a lambda **Default functor**, keeping object construction distinct from **Dictionary** access.

**Dictionary (context-as-hash)**:
A Context whose **Default functor** is nil, used as a string/number-keyed hash (ADR-0030): `(Ctx key val)` sets (a nil value deletes), `(Ctx key)` gets, `(Ctx assoc-list)` bulk-loads, `(Ctx)` returns all pairs sorted. Each key is stored as a context symbol named `_` + the key (mirroring newLISP's `makeSafeSymbol`), so a Dictionary is just a Context of `_`-prefixed symbols and interoperates with `symbols`/`dotree`/`save`. A number key and its string form collapse to one entry (a newLISP quirk).
_Avoid_: "hash map"/"hash table" (say Dictionary), "association list" (that is the plain list-of-pairs `Ctx` accepts and returns)

**Colon dispatch**:
The `(:method obj …)` form: `:method` resolves the method at runtime from the class tag at the head of `obj`, giving polymorphism. Distinct from `Ctx:sym`, which names a specific symbol in a specific Context.
_Avoid_: "method call", "message send"

**String (byte buffer)**:
A niiLISP string is a **binary-safe sequence of bytes** (may hold arbitrary/invalid-UTF-8 bytes), not a guaranteed-UTF-8 text value. It doubles as the byte buffer for I/O and FFI. `length` counts bytes; `utf8len` counts characters. Character-oriented operations decode UTF-8 on demand over this byte storage.
_Avoid_: "text", "UTF-8 string" (misleading — storage is bytes, not validated UTF-8)

**File handle**:
An opaque integer returned by `open`, naming an open file in an interpreter-side registry (ADR-0029). It is a plain `Value::Int`, not a distinct value type — newLISP handles are integers and scripts pass and compare them as such. `0`/`1`/`2` are reserved for stdin/stdout/stderr; other numbers are registry slots reused from a freelist after `close` (so a stale handle can name a later-opened file, as in newLISP). Distinct from an **address (FFI)**, which is a real memory address of a value's buffer.
_Avoid_: "file descriptor" (it is not the OS fd), "stream", "pointer"

**Socket**:
A network endpoint from the `net-*` API (ADR-0033): `net-connect`/`net-listen`/`net-accept` create one, `net-send`/`net-receive` transfer bytes, `net-select` waits for readiness, `net-close` closes it. Reproduced as a **File handle** — a raw fd in the interpreter registry — so `net-send`/`net-receive`/`net-close` reuse the file-I/O machinery (`net-receive` *is* `read-buffer`). A single string address is a Unix-domain path; a host + port is TCP. Behind a default-on `net` build feature, Unix-only.
_Avoid_: "connection" (reserve for an established stream), "port" (a port is a number, not the socket)

**External process**:
An independent OS program launched by `process` (non-blocking, returns its pid) or run to completion by `exec`/`!` (ADR-0031). Uses `std::process::Command` — safe, cross-platform, always compiled in. The pid is a plain `Value::Int`, like a **File handle**. Distinct from the **Cilk API**, which forks the interpreter itself rather than exec'ing another program.
_Avoid_: "child" (reserve for a forked Cilk process), "thread"

**Cilk API (`spawn`/`sync`)**:
newLISP's high-level parallelism: `spawn` evaluates an expression in a child process, binding its result to a variable on `sync`; `sync` waits for spawned children (optionally calling an inlet per finish) and `abort` cancels them. Built on real Unix `fork()` of the interpreter, so a child inherits all the parent's definitions (ADR-0032). Behind a default-on `mt` build feature, Unix-only (like FFI). A child's result crosses back as its re-readable `repr`, read as data — the ORO deep-copy across the process boundary. niiLISP is single-threaded, so fork-then-continue is safe.
_Avoid_: "threads", "async tasks", "futures"

**share**:
newLISP's OS-shared-memory cell for exchanging a single value between processes: `(share)` allocates, `(share adr val)` writes, `(share adr)` reads. Reproduced as a `mmap`ed `MAP_SHARED` page whose real address (an integer, valid across `fork` since it preserves virtual addresses) is the handle; the value is stored as its binary-safe `repr` (ADR-0032). Part of the **Cilk API**'s `mt` feature.
_Avoid_: "shared variable", "global"

**bigint**:
An arbitrary-precision integer value, the third rung of the numeric tower beside `i64` and `f64` (ADR-0022). It arises from a decimal literal too large for `i64` or from an `L`-suffixed literal (`12L`), never from `i64` arithmetic overflow (which wraps). `+ - * / %` yield a bigint when an operand is a bigint and none is a float; a bigint prints as plain decimal digits with no `L`. Behind a default-on `bigint` build feature, mirroring newLISP's own compile-time switch.
_Avoid_: "bignum", "long" (newLISP's `L` is lexical only), "arbitrary integer"

**array**:
A fixed-length, list-like value (ADR-0023). It indexes, `setf`-assigns its elements, reports `length`, and prints exactly like a `list`; the only observable differences are the predicates (`array?` is true, `list?` is nil) and that it cannot be resized — `push`/`pop`/`extend` on an array are errors. `array-list` converts it to a plain list. Like a list it is an ORO value, deep-copied on store and pass.
_Avoid_: "vector", "tuple", "fixed list"

**regular expression**:
A pattern for `regex`/`regex-comp`. newLISP uses PCRE; niiLISP uses the pure-Rust `regex` crate (RE2-style), so classes, quantifiers, groups, alternation, and anchors work, but **backreferences and lookaround do not** (ADR-0028). Matching is over the byte string and returns byte offsets. Behind a default-on `regex` build feature.
_Avoid_: "PCRE" (niiLISP's regex is not PCRE), "pattern" (bare — ambiguous with `struct`/`pack` layouts)

**ORO (One Reference Only)**:
newLISP's GC-free memory model, which niiLISP commits to reproducing: every value has exactly one owner, values are deep-copied when stored in a structure or passed to a function, and memory is freed stack-wise as evaluation unwinds. Cyclic references cannot arise by construction.
_Avoid_: "garbage collection", "refcounting", "ownership model"

**Native C API**:
newLISP's embedding API (`newlisp.h`) and the cell-struct ABI that native C extensions link against. Explicitly **not** a compatibility target at this stage; recorded here only to name what `import compatibility` is *not*.
_Avoid_: "C API" (ambiguous — always qualify as native)
