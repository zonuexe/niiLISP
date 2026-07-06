# Cilk API and fork-based multitasking (Slice B)

The second, harder half of the process subsystem: newLISP's **Cilk API**
(`spawn`/`sync`/`abort`), shared memory (`share`), raw `fork`/`pipe`/`wait-pid`,
the message API (`send`/`receive`), and `signal`. These fork the **interpreter
itself** — the child keeps running niiLISP. Acceptance targets: `qa-cilk`,
`qa-share`, `qa-pipe`, `qa-pipefork`, `qa-message`, `qa-siguser`. Design was
grilled before writing; grounded in `newlisp.c` / `nl-filesys.c`.

## Real fork, not synchronous emulation or a re-exec

- **Chosen: real Unix `fork()`**, behind a default-on `mt` Cargo feature,
  `cfg(all(feature = "mt", unix))`, over a new optional `libc` dependency (like
  `ffi` is gated + Unix-only, ADR-0018). `--no-default-features` / Windows leave
  the primitives undefined.
  - **A re-exec (fresh `niilisp` subprocess) cannot work:** a spawned expression
    references the parent's definitions — `qa-cilk` runs `(spawn 'f1 (fibo …))`
    where `fibo` is parent-defined. Only a fork of the running interpreter
    inherits the full environment.
  - **Synchronous emulation cannot pass the pipe/signal oracles:** `qa-pipe`/
    `qa-pipefork`/`qa-siguser` call real `fork`/`pipe`/`wait-pid`/`exec`/`signal`.
    (`qa-cilk` even guards `(if (not fork) (exit))`, so defining `fork` is what
    makes its real success path run.)
- **Safety:** niiLISP is **single-threaded** (no `thread::spawn`), so the
  fork-then-continue idiom is the classic, safe Unix pattern — not the
  multi-threaded-fork UB. The child evaluates, writes its result, and calls
  `libc::_exit` (skipping Rust unwinding / double buffer flushes). The `unsafe` is
  bounded and precedented (FFI, ADR-0015/0018).

## Cross-process values: re-readable `repr`, read back as data

Every value crossing a process boundary (a `spawn` result, a `share` cell, a
`send` message) is serialised as its **re-readable `repr`** and read back with the
reader as **data** (one form, not evaluated) — the ORO deep-copy across the
boundary, reusing the printer + reader.

This required making the string printer/reader **round-trip binary** (the printer
escaped nothing, so a string with a quote / control byte / non-UTF-8 byte did not
re-read). The printer now emits `\"` `\\` `\n` `\t` `\r` and `\NNN` for other
bytes, and the reader parses them — a faithful-to-newLISP fix that also repairs
`save`/`source` of binary strings.

## Cilk core: spawn / sync / abort

- **`(spawn 'sym expr [flag])`** — `pipe()` then `fork()`. The **child** evaluates
  `expr`, writes its `repr` to the pipe, and `_exit`s. The **parent** records
  `(pid, sym, read-fd)` and returns the pid. `flag` (message-enabled) sets up a
  `socketpair` for `send`/`receive`.
- **`(sync)`** returns the pending pids. **`(sync timeout)`** `poll`s the read-fds
  up to `timeout` ms, reads each finished child's result, binds it to its `sym`
  (`read`-as-data), and reaps with `waitpid`; `true` if all finished else `nil`.
  **`(sync timeout inlet)`** calls `(inlet pid)` after collecting each child.
- **`(abort pid)`** / **`(abort)`** — `kill` (and reap) one / all pending children;
  `true`.

## Shared memory: `share`

`(share)` `mmap`s a fixed-size `MAP_SHARED | MAP_ANONYMOUS` page and returns its
**real address as an integer** — `fork` preserves virtual addresses, so parent and
child name the same page (newLISP-faithful). `(share adr val)` writes, `(share
adr)` reads; the value is stored length-prefixed as its (now binary-safe) `repr`.
A registry of live pages validates addresses on access (bounding the `unsafe`) and
frees them. Oversized values error.

## Message API: `send` / `receive`

A `socketpair` per message-enabled child. `(send pid msg)` is **non-blocking**
(a full buffer returns `nil`, matching `qa-message`'s `(until (send …))` retry),
framing the `repr` length-prefixed. `(receive)` returns pids with a message ready;
`(receive pid 'place)` reads one into `place`. `sys-info -3` / `-4` return this /
parent pid.

## Raw fork / signals

`(fork expr)` (child evaluates, no result return), `(pipe)` (a read/write fd
pair), `(wait-pid pid)`, `(signal n handler)`, and `(exec …)` (ADR-0031) cover
`qa-pipe`/`qa-pipefork`/`qa-siguser`.

## Implementation order (by oracle, dependency-first)

1. **B1** `spawn`/`sync`/`abort`/inlet (pipe transport) → `qa-cilk`.
2. **B2** `share` (mmap) → `qa-share`.
3. **B3** raw `fork`/`pipe`/`wait-pid` → `qa-pipe`/`qa-pipefork`.
4. **B4** `send`/`receive` (socketpair) → `qa-message`/`qa-msgbig`.
5. **B5** `signal` + `exec` → `qa-siguser`.

## Consequences

- New optional `libc` dependency, Unix-only, behind the default-on `mt` feature;
  bounded `unsafe` for `fork`/`mmap`/`pipe`/`waitpid`/`kill`/`socketpair`.
- The Cilk state (spawn registry, share pages) lives on `Interp` under
  `cfg(all(feature = "mt", unix))`.
- Large; implemented as the sub-slices above, each with its oracle.
