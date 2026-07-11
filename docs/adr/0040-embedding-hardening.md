# Embedding hardening: safe exit, host builtins, and an eval-step limit

[ADR-0039](0039-embedding-library-target.md) made niiLISP embeddable but left
three sharp edges called out in its own "consequences": `(exit)` kills the host
process, a host cannot add its own builtins from outside the crate, and an
untrusted script can loop forever. This ADR closes all three.

## `(exit)` returns a `Signal::Exit(code)` instead of killing the process

- **Chosen:** `exit` no longer calls `std::process::exit`; it returns
  `Err(Signal::Exit(i32))`, which unwinds like any other non-local control flow
  (running Rust destructors on the way out). `catch` does **not** trap it — it
  propagates past `catch`/`catch`-with-target, so a script cannot suppress its own
  exit. The **CLI** translates a top-level `Signal::Exit(code)` into its process
  exit code (the observable CLI behaviour is unchanged); the **REPL** treats it as
  "quit"; an **embedder** matches on it (`Err(Signal::Exit(code))`) and decides
  what to do — the host is never terminated behind its back.
- **Rejected:** keeping `process::exit`. It is fine for a standalone CLI but
  unacceptable for a library — a config script calling `(exit)` would take down
  the whole host application.
- **Note:** the Cilk fork child (`spawn`) already terminates with `libc::_exit`
  after `interp.eval(...).unwrap_or(nil)`, so it is unaffected — a child body's
  `(exit)` collapses to `nil` and the child still `_exit`s as before.

## Host-provided Rust builtins via `register_builtin`

- **Chosen:** document and support `Interp::register_builtin(name, func)` (already
  used internally by the builtin modules) as the public extension point, and
  re-export `BuiltinFn` at the crate root. A host adds a primitive with a plain
  function:
  ```rust
  fn host_now(i: &niilisp::Interp, _args: &[niilisp::Value]) -> Result<niilisp::Value, niilisp::Signal> {
      Ok(niilisp::Value::Int(42))
  }
  interp.register_builtin("host-answer", host_now);
  ```
- **Rejected (deferred):** closures / `Box<dyn Fn>` builtins that capture host
  state. That requires widening `Value::Builtin` from a bare `fn` pointer to an
  `Rc<dyn Fn>`, touching the value model and its `Clone`/`Debug`. It is a real
  feature but out of scope here; a host can thread state through the interpreter's
  own globals or a `thread_local` in the meantime. Recorded as a follow-up.

## An opt-in eval-step limit for untrusted scripts

- **Chosen:** an interpreter-level counter incremented once per `eval` call, with
  an optional ceiling set by `Interp::set_eval_limit(Option<u64>)` (default: no
  limit). When the ceiling is exceeded, evaluation stops with an **uncatchable**
  `Signal::Limit` — like `Exit`, it propagates past `catch` so a script cannot
  loop-and-catch its way around it. The counter resets at the start of each
  top-level `eval_string`, so the limit bounds one script run, not the lifetime of
  the interpreter.
- **Chosen (cost):** when no limit is set the check is a single `Cell` read and a
  branch per `eval`; measurable but tiny, and off by default. This honours
  ADR-0007 (correctness-first, opt-in) — it is a safety valve for untrusted input,
  not a always-on tax.
- **Rejected:** a wall-clock timeout thread. It needs another thread and a way to
  interrupt synchronous Rust code; a step counter is deterministic, portable, and
  reproducible (the same script always stops at the same point).
- **Rejected:** counting memory/allocations. Useful but a much larger design
  (custom allocator or pervasive accounting); the step limit already stops the
  common infinite-loop / runaway-recursion case.

## `Signal` grows two propagating variants

- **Chosen:** `Signal` becomes `Throw | Error | Exit(i32) | Limit`. `Throw` and
  `Error` remain catchable; `Exit`/`Limit` are control signals that bypass
  `catch`. Every exhaustive `match` on `Signal` (the two `catch` arms, the CLI, the
  REPL, `signal_message`, the `Debug` impl) is updated — the compiler enforces
  completeness, so nothing silently mishandles the new variants.

## Consequences

- niiLISP is safe to embed for **untrusted** scripting: `(exit)` cannot kill the
  host, a runaway script is bounded by the step limit, and the host can expose
  exactly the primitives it wants.
- The CLI and REPL behave identically to before (exit codes preserved).
- Remaining embedding follow-ups (noted, not scheduled): closure/userdata
  builtins, and a way to sandbox or disable the OS-touching builtins
  (`process`/`net`/`import`/file I/O) at runtime rather than only via Cargo
  features.
