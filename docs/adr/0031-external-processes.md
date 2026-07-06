# External processes (Slice A): process / exec / sleep

The first, cross-platform half of the process subsystem from the gap analysis
([`docs/notes/20260706_newlisp-gap-analysis.md`](../notes/20260706_newlisp-gap-analysis.md)):
launching and running **external** programs. This is what the (deferred) GUI
launcher needs — newLISP-GS starts `java -jar guiserver.jar` with `process`.
Distinct from the fork-based **Cilk API** (ADR-0032), which forks the interpreter
itself. Design was grilled before writing.

## Always compiled in, cross-platform via `std::process::Command`

Unlike the Cilk API, external-process launch has no fork-in-Rust hazard —
`std::process::Command` does the fork+exec safely — and needs no `libc`, so it is
**always compiled in** and works on Windows too. Same rationale as file I/O
(ADR-0029).

## Functions

- **`(process "cmd arg…" [in out err])`** — split the command string into argv on
  whitespace, `Command::spawn` it, and return the child **pid** (integer);
  non-blocking. `(process "java -jar guiserver.jar 64001")` launches the GUI
  server. The `in`/`out`/`err` fd-redirection arguments and shell-style quote
  handling are a **deferred** Unix refinement (the basic launch the GUI needs
  wants neither).
- **`(exec "cmd" [instr])`** — run `cmd` to completion (blocking) and return its
  stdout as a **list of lines**; with `instr`, feed it to the child's stdin.
  `Command::output`.
- **`(! "cmd")`** — run `cmd` through the shell, returning its exit code.
- **`(sleep ms)`** — `std::thread::sleep` for `ms` milliseconds, returning `ms`.
  Always-on (the Cilk oracles use it for timing, but it is just `std`).

## Value model

A pid is a plain `Value::Int`, consistent with the file-handle model (ADR-0029) —
newLISP process ids are integers that scripts pass around as such. Operational
failures (a command that cannot be spawned) return `nil`, matching the file-I/O
error model (ADR-0029) and `import`.

## Consequences

- A new `src/process.rs` module holds this (unconditional) code and, behind
  `cfg(all(feature = "mt", unix))`, the Cilk API (ADR-0032).
- `process` unblocks the eventual GUI launcher; `exec` is a `qa-siguser`
  prerequisite (with `signal`, ADR-0032).
