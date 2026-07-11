# Embedding: a library target and a curated interpreter API

niiLISP ships only a **binary** today — `Cargo.toml` has `[[bin]]` targets but no
`[lib]`, and every module is declared with `mod` inside `src/main.rs`, so it is
private to the executable. A Rust program cannot `use niilisp::…`: `cargo add
niilisp` gives nothing importable. This ADR adds a **library target** so niiLISP
can be embedded as an in-process interpreter (linked and called as functions, not
launched as a subprocess), and defines what that public surface is.

This is a natural fit — the interpreter already keeps all its state in a single
`pub struct Interp` with a `pub fn new()` and `pub fn eval_string(&[u8]) ->
Result<Value, Signal>`. The blocker is purely packaging: there is no lib to link.

## A `bin` + `lib` crate, with `main` a thin client of the lib

- **Chosen:** add `src/lib.rs` and a `[lib] name = "niilisp"` target alongside the
  existing binaries. The library owns the module tree (`mod eval; mod value; …`)
  and `src/main.rs` becomes a thin CLI that does `use niilisp::…`. Both targets
  share one compilation of the modules; the feature flags apply to both.
- **Rejected:** duplicating logic, or exposing modules only through the binary.
  A binary cannot be a dependency; the lib is the only way to link the code.
- **Rejected:** a separate `niilisp-core` crate. A single crate with `bin`+`lib`
  is the standard Rust layout and keeps the CLI and the embedding API in lock-step.

## A curated, minimal public API — not the whole module tree

- **Chosen:** the library re-exports a small, intentional surface at the crate
  root: **`Interp`** (`new`, `eval_string`, `eval`, `repr`, `intern`, and the
  existing accessors), **`Value`** (the value type), and **`Signal`** (the
  error/throw type). A minimal embedding is:
  ```rust
  let interp = niilisp::Interp::new();
  match interp.eval_string(b"(+ 1 2)") {
      Ok(v)  => println!("{}", interp.repr(&v)),
      Err(e) => eprintln!("{e:?}"),
  }
  ```
  The remaining modules that the binary still needs (`reader`, `repl`, the
  script-runner helpers) are exposed but marked unstable/`#[doc(hidden)]`; they are
  implementation details of the CLI, not the embedding contract.
- **Rejected:** making every module `pub` as the API. It would freeze internal
  details (the reader, the printer, FFI plumbing) as public commitments. The
  curated set is what an embedder actually needs and what we can keep stable.

## Pre-1.0: the embedding API is explicitly unstable

- **Chosen:** document that while niiLISP is `0.x`, the `Interp`/`Value`/`Signal`
  shapes may change between minor versions. Embedders should pin an exact version.
  This is stated in the README embedding section and the crate docs, not enforced.
- **Rejected:** promising stability now. The value model and evaluator are still
  moving (this very series of ADRs keeps extending them); a premature stability
  promise would be dishonest.

## Documented embedding caveats — the sharp edges that are not obvious

The clean module split makes niiLISP *look* embeddable already; these are the
non-obvious footguns, called out in the embedding guide rather than papered over:

- **`(exit)` terminates the host process.** `exit` calls `std::process::exit`, so
  a script's `(exit)` kills the embedding application. This ADR keeps that
  behaviour for now and **documents it**; an embedding-safe `exit` (returning a
  `Signal::Exit(code)` the host can handle, instead of exiting) is a bounded
  follow-up that also touches the CLI's exit handling.
- **Single-threaded, `!Send`/`!Sync`.** `Interp` is built on `Rc`/`RefCell`
  throughout, so one interpreter is confined to one thread; it cannot be shared
  across threads or held across an `.await` that requires `Send`. Use one `Interp`
  per thread.
- **Default features fork/network/FFI the host.** The default build enables `mt`
  (a real `libc::fork()` of the *host* process for `spawn`/`process`), `net`, and
  `ffi`. For a sandboxed embedded interpreter, depend with
  `default-features = false` and opt into only what you need (e.g. `bigint`,
  `regex`, `date`). This keeps the embedded interpreter pure and side-effect-free.

## Consequences

- Unblocks using niiLISP as a scripting/config language inside a Rust
  application, with no subprocess and no IPC.
- The CLI keeps working unchanged (it becomes a client of the same lib), and the
  release/`cargo publish` flow is unaffected (a crate can carry a lib and bins).
- Follow-ups this makes possible and worth doing next: an embedding-safe `exit`
  (`Signal::Exit`), a way to register host-provided Rust builtins from outside the
  crate, and a resource/step limit for untrusted scripts.
