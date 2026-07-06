# Networking: stream sockets (net-connect / net-listen / …)

The last GUI-enabling subsystem from the gap analysis
([`docs/notes/20260706_newlisp-gap-analysis.md`](../notes/20260706_newlisp-gap-analysis.md)):
newLISP's `net-*` sockets. newLISP-GS drives its Java server over two localhost
TCP sockets, so this is the substrate the (still-deferred) GUI needs. Acceptance
target: the `qa-local-domain` oracle — the same connect/listen/accept/send/
receive/select surface the GUI uses, over Unix-domain sockets, self-contained
(it `fork`s a listener). Design was grilled before writing.

## Sockets are file handles (raw fds), reusing the file-I/O machinery

- **Chosen:** a socket is a **`FileTable` handle** — the same opaque-integer
  registry as files and pipes (ADR-0029/0032). Sockets are created with the safe
  `std::net` / `std::os::unix::net` bind/connect, then `into_raw_fd` and
  registered via `FileTable::insert_fd` (as `pipe` already does). So:
  - **`net-send`** reuses the handle write path (returns the byte count);
  - **`net-receive`** *is* `read-buffer` — same `(sock place maxlen [wait])`
    signature — so it reuses `sf_read_buffer` (a place-taking special form)
    verbatim;
  - **`net-close`** reuses the handle close.
  - New code is confined to socket creation and `net-accept` / `net-select` /
    `net-peek` / `net-peer` / `net-local`, which call `libc` on the raw fd.
- **Rejected:** a separate socket registry with a typed `TcpStream`/`UnixStream`
  per handle. It would duplicate the read/write/close machinery and complicate a
  uniform `net-select`/`read-line` over both sockets and files, for no gain —
  newLISP's model is "a socket is an fd" too.

## Unix-only, behind a default-on `net` feature

- **Chosen:** `cfg(all(feature = "net", unix))`, pulling `libc` — the same shape
  as `mt` (ADR-0032). Unix-domain sockets are Unix-only, and `net-accept`/
  `net-select`/`net-peek` are raw `libc` on the fd. `--no-default-features` and
  Windows leave the `net-*` primitives undefined. `qa-local-domain` also uses
  `fork`, so running it needs `mt` too.
- TCP itself is cross-platform via `std::net`; a future Windows port could add it,
  but the fd-unified model and the Unix-domain target make Unix-first right.

## Blocking, with `net-select` for readiness

- **Chosen:** sockets are **blocking** (newLISP's default); `net-select`
  polls for readiness. `net-connect` returns `nil` on a refused connection (so a
  script retries until the server is up, as `qa-local-domain` does); `net-accept`
  blocks for a connection; `net-receive` reads what is available (after
  `net-select` says the socket is readable).

## Functions

- **`(net-connect host port)`** / **`(net-connect "/path")`** — a single string
  argument is a Unix-domain path; a host + port is TCP. Returns a socket handle,
  or `nil` on failure.
- **`(net-listen port)`** / **`(net-listen "/path")`** — a listening socket
  (TCP port, or Unix-domain path). `(net-accept lsock)` accepts one connection,
  returning a connected handle.
- **`(net-send sock str)`** — send bytes; returns the count.
- **`(net-receive sock place maxlen [wait])`** — receive up to `maxlen` bytes (or
  until `wait`) into `place`; returns the count. (Reuses `read-buffer`.)
- **`(net-select sock "read"|"write" timeout-ms)`** — poll one socket; truthy if
  ready, `nil` on timeout. (A list of sockets → the ready ones, later.)
- **`(net-peek sock)`** — bytes available to read (`FIONREAD`).
- **`(net-peer sock)`** / **`(net-local sock)`** — the remote / local address as a
  best-effort string (`ip:port` for TCP, the path for Unix). Not a strict
  compatibility surface (the oracle only prints them).
- **`(net-close sock)`** — close the socket.

## Scope and deferred

- **This slice:** the stream-socket API above → `qa-local-domain` (and the GUI's
  TCP substrate).
- **Follow-on:** UDP (`net-send`/`net-receive` datagram, `net-send-to` /
  `net-receive-from`) → `qa-udp`; the multi-socket `net-select`.
- **Deferred / N-A:** `net-eval` and newLISP server mode (`qa-net` needs a
  `newlisp` server binary), raw `net-packet` / `net-ping` (root, `qa-packet` /
  `qa-lookup6`), `net-lookup` (DNS), and `get-url` / `put-url` / `post-url`
  (HTTP).

## Consequences

- A new `src/net.rs` under `cfg(all(feature = "net", unix))`; small `libc` use for
  accept/select/peek/peer/local. Send/receive/close reuse the file-I/O handles, so
  the net-specific surface stays small.
