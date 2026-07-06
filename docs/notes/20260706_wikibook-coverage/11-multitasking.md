# Ch. 11 — Multitasking

niiLISP implements the fork-based Cilk API (spawn/sync/abort/fork/pipe/wait-pid/share/send/receive/signal/exec/!/sleep) faithfully on Unix, but `process` silently ignores its stdio-redirection arguments and two functions the book merely lists (`destroy`, `semaphore`) don't exist at all.

**Coverage: 10 ✅ / 2 ⚠️ / 2 ❌**

| Feature | Status | Notes |
|---|---|---|
| `!` (shell exec) | ✅ | Runs command with inherited stdio, returns exit code |
| `exec` | ✅ | Returns stdout as list of lines, matches book |
| `sleep` | ✅ | Blocks for ms, returns ms |
| `spawn` / `sync` (no-arg) | ✅ | Returns pid; `(sync)` lists pending pids |
| `sync` (timeout) | ✅ | Returns `nil` while pending, `true` once all finished; binds result symbol only after `sync` returns `true`, exactly as the book describes |
| `abort` | ✅ | Kills pending spawned children, returns `true`; `(sync)` empty afterward |
| `fork` | ✅ | Child evaluates expr and exits; parent gets pid; verified via file side-effect |
| `wait-pid` | ✅ | Blocks until forked child exits |
| `pipe` | ✅ | Returns `(read-handle write-handle)`; works with `read-line`/`write-line` across `fork` |
| `share` | ✅ | Allocates/reads/writes shared mmap page; value written by a forked child is visible in the parent |
| `send` / `receive` | ⚠️ | Work correctly, but the book gives no worked example to compare against, and the calling convention differs from what a newLISP user would guess (see gaps) |
| `process` | ⚠️ | Launches the program and returns its pid, but completely ignores the `in`/`out` pipe handle arguments — stdio is inherited from the parent, not redirected. The book's `bc` example (piping input/output through `process`) cannot work |
| `signal` | ✅ | `(signal n handler)` catches the OS signal and runs the handler at the next safe point; verified catching `SIGINT` |
| `destroy` | ❌ | Not implemented — calling it errors `not a function: nil` |
| `semaphore` | ❌ | Not implemented — calling it errors `not a function: nil` |

## Divergences & gaps

### `process` ignores stdio-redirection arguments (⚠️)

The book's central `process` example pipes to/from Unix `bc`:

```
(map set '(bcin myout) (pipe))
(map set '(myin bcout) (pipe))
(process "/usr/bin/bc" bcin bcout)
(write-buffer myout (string sum "\n"))
(set 'answer (read-line myin))
```

niiLISP's `b_process` (`src/process.rs:41-52`) only does `Command::new(program).args(parts).spawn()` — it never wires the child's stdin/stdout to the given pipe handles, and the doc comment admits it: *"The optional stdio-redirection arguments are a deferred Unix refinement."* Repro:

```
$ niilisp script.lsp   # script pipes to `/usr/bin/bc` per the book's example
```
Result: the script hangs (killed after the 8s bound) because `bc` reads from the real terminal stdin, not from the `myout` pipe, and no data ever reaches `myin`. Confirmed independently:

```
$ niilisp -e '(begin (map set (list (quote a) (quote b)) (pipe)) (println (process "/bin/echo hi" a b)))'
11511
```
`process` returns a valid pid with no error, but `hi` is printed to the real stdout instead of being captured — silent divergence, not a hang, when the child doesn't block on stdin.

### `send`/`receive` — undocumented calling convention (⚠️)

The book's function list mentions `send`/`receive` don't even get an example in this chapter, so there's nothing to diverge from, but niiLISP's actual form is stricter than a plain `(receive)`/`(receive msg)` guess:

```
(receive pid place)   ; reads one datagram from `pid` into `place`, returns true/nil
(receive)             ; returns list of peer pids with data ready
```

A naive `(receive msg)` call (treating the first arg as the target place) is misparsed as a pid and errors. Once called correctly:

```
$ cat script.lsp
(set 'ppid (sys-info -3))
(set 'pid (spawn 'r1 (begin
  (set 'gotmsg nil)
  (until (receive ppid gotmsg))
  (string "child-received: " gotmsg)) true))
(sleep 200)
(send pid "hello-from-parent")
(until (sync 100))
(println r1)
$ niilisp script.lsp
r1: child-received: hello-from-parent
```
This works correctly — flagged ⚠️ only because the chapter gives no example to verify fidelity against, and the message channel requires the spawn's optional third `true` argument (undocumented in this chapter) to exist at all.

### `destroy` — missing (❌)

Listed by the book as one of the OS-interaction functions ("`destroy` kills a process") but not implemented:

```
$ niilisp -e '(println (destroy 123))'
niilisp: not a function: nil
```
No occurrence of `"destroy"` anywhere in `src/`.

### `semaphore` — missing (❌)

Listed by the book ("`semaphore` creates and controls semaphores") but not implemented:

```
$ niilisp -e '(println (semaphore))'
niilisp: not a function: nil
```
No occurrence of `"semaphore"` anywhere in `src/`.
