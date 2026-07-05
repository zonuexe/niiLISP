# File I/O (first slice): handles, filesystem operations

The foundation subsystem from the newLISP gap analysis
([`docs/notes/20260706_newlisp-gap-analysis.md`](../notes/20260706_newlisp-gap-analysis.md)):
raw file I/O and filesystem operations. It unblocks dictionary persistence and,
much later, the `load` of vendored modules (newLISP-GS is deliberately deferred —
we build only the substrate here). Acceptance target: the `qa-lfs` oracle. Design
was grilled before writing.

## North star: order by dependency, not by the GUI

The gap analysis named the *Graphical interface* chapter as an eventual goal.
newLISP-GS is not primitives — `guiserver.lsp` launches a Java process and drives
it over two TCP sockets, evaluating inbound events with `eval-string`. That path
needs three unbuilt subsystems (file I/O, processes, networking). We **order by
dependency and breadth-of-unlock, keeping the GUI only as a long-horizon target
we do not schedule**: file I/O is the shared prerequisite (GUI, dictionary
persistence, `save`/`load`) and unlocks the most on its own. Process, networking,
and the GUI integration itself are not scheduled until this and the dictionary
layer land.

## Slice boundary

**In this slice** (raw I/O + filesystem, no eval/serialisation):

- Handles/streaming: `open`, `close`, `read-buffer`, `write-buffer`, `read-line`,
  `read-file`, `write-file`, `append-file`, `seek`.
- Filesystem: `directory`, `real-path`, `make-dir`, `remove-dir`, `change-dir`,
  `rename-file`, `delete-file`, `file-info`.
- Predicates/env: `file?`, `directory?`, `exists`, `env`.

**Deferred to a later slice:** `load` / `save` / `source` (read+eval and
value→source serialisation — they touch the reader's current-context model,
ADR-0026, and are a different kind of work), `device` output redirection,
`read-char` / `read-utf8` / `read-key` / `read-expr`, `pretty-print` / `dump`,
`copy-file`, and the `search` on a handle.

`qa-lfs` is the clean acceptance target. `qa-utf16path` needs everything here
except `save`, but is fundamentally a Windows UTF-16-path test, so it stays
Windows-only / deferred rather than a slice-1 gate. `qa-local-domain` is a
**networking** test (Unix-domain sockets, all `net-*`), not file I/O.

## Handles are opaque integers from an interpreter registry

- **Chosen: an opaque integer handle** — `open` returns a `Value::Int` index into
  an interpreter-side registry (`RefCell<Vec<Option<…>>>`, the same interior-
  mutability shape as `libs`/`regex_cache`), with **0/1/2 pre-reserved for
  stdin/stdout/stderr**. Handles carry no new value type; newLISP handles are
  integers and scripts pass/compare them as integers, so `Value::Int` preserves
  source compatibility. `close` frees the slot; slots are **reused from a
  freelist**, matching newLISP's fd-reuse behaviour (a stale handle number can
  point at a later-opened file — the same footgun newLISP has, kept for
  behavioural fidelity). `File`s are dropped (flushed/closed) by `close` and by
  `Interp` drop via RAII; no explicit exit handling.
- **Rejected: mirror the raw OS fd** as newLISP literally does. It would match
  handle *numbers* exactly, but forces raw-fd/`unsafe` handling and diverges on
  Windows (different fd model), for no compatibility gain — scripts only hardcode
  the standard streams 0/1/2, which we reserve regardless. Windows is a release
  target (`--no-default-features`), so a pure-`std::fs`, `unsafe`-free path wins.

## Error model: nil on operational failure, throw only on type misuse

- **Chosen:** operational failures (opening a missing file, `read-line` at EOF,
  I/O on a bad handle) **return `nil`**, mirroring newLISP (`nl-filesys.c`) and
  the existing `import`-returns-`nil` convention. Only **type misuse** (a
  non-integer handle, a non-string path/data) raises a niiLISP error.
- **Rejected:** throwing on I/O failure. It reads better but breaks the newLISP
  control-flow idioms `(if (open …) …)` and `(while (read-line h) …)` that rely
  on `nil`, which source compatibility forbids.

## Always compiled in — no Cargo feature

- **Chosen:** file I/O is **always on**, unlike `ffi`/`bigint`/`regex`. Those are
  gated because each carries an external dependency or mirrors a newLISP compile
  switch; file I/O is pure `std::fs` with neither, and newLISP always has it. It
  ships in the `--no-default-features` Windows build too (`std::fs` is
  cross-platform).
- **Rejected:** an `fs` feature for sandboxed/pure builds. Sandboxing is a
  security posture on a different axis, absent from newLISP; add it only if a
  concrete need appears.

## Paths are byte buffers → OS-native, binary-safe

- **Chosen:** a path is a niiLISP byte-buffer string converted OS-natively — on
  Unix via `OsStrExt::from_bytes`, so **non-UTF-8 filenames round-trip losslessly**
  (consistent with the binary-safe string model). Windows uses a lossy UTF-8
  interpretation for now; faithful UTF-16 path handling is carved out to the
  `qa-utf16path` slice.
- **Rejected:** requiring valid UTF-8 for every path. Simpler, but drops Unix's
  arbitrary-byte filenames and contradicts the string model.

## Observable semantics: faithful newLISP mirror

Grounded in `nl-filesys.c`:

- **`open`** — modes `"read"`=`r`, `"write"`=`w` (create/truncate),
  `"append"`=`a`, `"update"`=`r+` (read/write, no truncate). All four in this
  slice. A single `std::fs::File` per handle → **one OS cursor shared by
  read/write/seek**.
- **`read-line`** — unbuffered, byte-at-a-time to the terminator; `\n`, `\r\n`,
  and a lone `\r` all terminate; the terminator is **not** included; **`nil` at
  EOF** with nothing read. No `BufReader` in this slice (correctness/seek-
  consistency first, ADR-0007; optimise later). The most recent line is stashed
  so **`current-line` is a single interpreter-global value** (newLISP's shared
  read-line buffer — deliberately not per-handle).
- **`seek`** — `(seek h)` returns the current position (tell); `(seek h -1)`
  seeks to end; otherwise absolute from the start; **64-bit** offsets (`qa-lfs` is
  a large-file test); `nil` on error.
- **`read-buffer`** `(read-buffer h place size [wait-str])` — reads up to `size`
  bytes (or until `wait-str`), **assigns the string to `place` (a symbol)**, and
  **returns the byte count**. `write-buffer` `(write-buffer h str [size])` writes
  and **returns the byte count**.
- **`file-info`** — a **fixed 10-element integer list**
  `(size mode device inode links uid gid atime mtime ctime)` on every platform;
  fields a platform lacks (inode/uid/gid on Windows) are **0**, so index
  positions (size at 0, mtime) stay stable.
- **`directory`** `(directory [path [pattern]])` — entry names including `.`/`..`
  (newLISP-faithful), default path `"."`; the `pattern` filter applies only when
  the `regex` feature is compiled in.
- **`env`** — `(env var)` gets (string / `nil`), `(env var val)` sets (`true`).
- **Standard streams** — this slice wires `write`/`write-buffer` to `1`→stdout /
  `2`→stderr and `read-line`/`read-buffer` from `0`→stdin. `print`/`println` stay
  direct to stdout; `device` redirection is deferred.

## Consequences

- A new interpreter-side handle registry with a freelist and reserved 0/1/2.
- `qa-lfs` becomes wireable; `qa-utf16path` waits on Windows UTF-16 paths and
  `save`; `qa-local-domain` waits on the networking slice.
- Dictionary persistence (`save`/`load`) can build on these handles in its own
  slice.
