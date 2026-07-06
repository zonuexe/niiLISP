# Ch. 14 — The Internet

niiLISP 0.3.1 implements a working Unix/TCP stream-socket core (`net-connect`/`net-listen`/`net-accept`/`net-send`/`net-receive`/`net-select`/`net-peek`/`net-peer`/`net-local`/`net-close`) plus `base64-enc`/`base64-dec`, but has no HTTP client (`get-url`/`put-url`/`post-url`/`delete-url`), no UDP support, and no `net-error`/`net-eval`/`net-lookup`/`net-service`/`net-ipv`/`net-sessions`.

**Coverage: 11 ✅ / 1 ⚠️ / 10 ❌**

| Feature | Status | Notes |
|---|---|---|
| `net-connect` (TCP) | ✅ | Connects to `127.0.0.1:PORT`, returns an fd handle. |
| `net-connect` (Unix domain) | ✅ | `(net-connect "/path")` works against a `net-listen` Unix socket. |
| `net-listen` (TCP) | ✅ | `(net-listen port)` binds `0.0.0.0:port`. |
| `net-listen` (Unix domain) | ✅ | `(net-listen "/path")` binds, clears stale socket file first. |
| `net-accept` | ✅ | Blocking accept; returns a new fd handle. |
| `net-send` | ✅ | Sends bytes, returns byte count. |
| `net-receive` (size form) | ✅ | `(net-receive sock buf size)` reads into `buf`. |
| `net-receive` (wait-string form) | ✅ | `(net-receive sock buf size "\r\n")` reads to delimiter, matches newLISP signature. |
| `net-select` | ✅ | `(net-select sock "read"/"write" timeout-ms)` returns the socket on ready, `nil` on timeout/invalid handle — matches newLISP. |
| `net-peek` | ✅ | Returns bytes available (`FIONREAD`), 0 when idle. |
| `net-peer` / `net-local` | ✅ | Return `"ip:port"` strings for TCP sockets. |
| `net-close` | ✅ | Closes the fd handle, returns `true`. |
| `base64-enc` / `base64-dec` | ✅ | `(base64-enc "hello world")` → `"aGVsbG8gd29ybGQ="`; round-trips correctly. |
| `get-url` | ❌ | Unbound; the WikiBook's JPEG-scraping example (`get-url` + `replace`) cannot run at all. |
| `put-url` | ❌ | Unbound. |
| `post-url` | ❌ | Unbound. |
| `delete-url` | ❌ | Unbound. |
| `net-error` | ❌ | Unbound — the IRC-client example's error branch (`(net-error)` in the `cond`) silently evaluates to `nil` forever instead of detecting a dropped connection. |
| `net-eval` | ❌ | Unbound. |
| `net-lookup` | ❌ | Unbound (no reverse-DNS helper). |
| `net-service` | ❌ | Unbound (no service-name→port lookup). |
| `net-ipv` | ❌ | Unbound. |
| `net-sessions` | ⚠️ | Unbound; not explicitly demonstrated in this chapter's code but listed among its networking functions — no live socket registry to enumerate. |
| `net-send-udp` / `net-receive-udp` / `net-send-to` / `net-receive-from` | ❌ | All four UDP functions unbound; no datagram support at all. |

## Divergences & gaps

**`get-url` / `put-url` / `post-url` / `delete-url` — unbound (❌)**

```
$ niilisp -e '(println (get-url "http://example.com"))'
niilisp: not a function: nil
```
Same "not a function: nil" failure for `put-url`, `post-url`, `delete-url`. The chapter's flagship example —

```lisp
(set 'the-source (get-url "http://www.apple.com"))
(replace {src="(http\S*?jpg)"} the-source (push $1 images-list -1) 0)
```

— cannot be attempted at all in niiLISP; there is no HTTP client layer.

**`net-error` — unbound (❌)**

```
$ niilisp -e '(println (net-error))'
niilisp: not a function: nil
```
The IRC-client example relies on `(net-error)` inside a `cond` to detect connection failure/reset:
```lisp
((net-error)
    (println "\n\027[0;34m" "UH-OH: " (net-error) "\027[0;0m")
    (share connected nil))
```
Since the symbol is unbound, `(net-error)` always evaluates to `nil` (per the CRITICAL PROBING RULE — unbound symbols return `nil`, not an error), so this branch of the loop can never fire; a niiLISP port of the example would hang/spin instead of reporting a dropped IRC connection.

**`net-lookup` / `net-service` / `net-ipv` / `net-eval` — unbound (❌)**

```
$ niilisp -e '(println (net-lookup "127.0.0.1"))'
niilisp: not a function: nil
$ niilisp -e '(println (net-service "http" "tcp"))'
niilisp: not a function: nil
$ niilisp -e '(println (net-ipv))'
niilisp: not a function: nil
$ niilisp -e '(println (net-eval))'
niilisp: not a function: nil
```
None of these hostname/service/IP-version/remote-eval helpers exist.

**UDP family — unbound (❌)**

```
$ niilisp -e '(println (net-send-udp "127.0.0.1" 9 "x"))'
niilisp: not a function: nil
$ niilisp -e '(println (net-receive-udp 9 100))'
niilisp: not a function: nil
$ niilisp -e '(println (net-send-to))'
niilisp: not a function: nil
$ niilisp -e '(println (net-receive-from))'
niilisp: not a function: nil
```
No datagram (UDP) support of any kind; only stream (TCP/Unix-domain) sockets are implemented (per `src/net.rs` module doc: "Networking: `net-*` stream sockets").

**`net-sessions` — inconclusive/unbound (⚠️)**

```
$ niilisp -e '(println (net-sessions))'
niilisp: not a function: nil
```
Not exercised by any code example in this specific chapter, but it's part of the networking function family the chapter's surrounding material references; flagged as unbound rather than a hard functional failure since no chapter example depends on it.

## What does work (repro)

TCP client/server round-trip (bounded local test, two scripts against `127.0.0.1:17654`):
```
listen: 3
accept: 4
peer: 127.0.0.1:56841
local: 127.0.0.1:17654
received: ping
peek-after-send: 0
server done
---
connect: 3
select-read: 3
client received: pong
client done
```

Unix-domain socket + `net-receive` wait-string delimiter form (`/tmp/niilisp-test.sock`):
```
listen: 3
received-line: hello-unix
---
connect: 3
```

`base64-enc`/`base64-dec` round-trip:
```
$ niilisp -e '(println (base64-enc "hello world"))'
aGVsbG8gd29ybGQ=
$ niilisp -e '(println (base64-dec (base64-enc "hello world")))'
hello world
```
