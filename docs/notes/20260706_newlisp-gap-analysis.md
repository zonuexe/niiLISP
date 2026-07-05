# niiLISP ↔ newLISP gap analysis (2026-07-06)

A stock-take of what niiLISP is still missing relative to stock newLISP 10.7.x,
framed by the *Introduction to newLISP* WikiBook chapters, with the **Graphical
interface** chapter used as a concrete end-goal to work back from.

## Method

- **niiLISP surface**: builtins registered via `reg(...)` + FFI dispatch
  (`src/ffi.rs`) + name-dispatched special forms in `src/eval.rs`. ~150 names.
- **newLISP surface**: the 378-primitive reference index from the 10.7.5 manual.
- Diffed the two sets, then hand-verified false positives in the built binary
  (an unbound symbol call returns `nil`, so each candidate was probed
  functionally). FFI (`import`/`callback`/`struct`/`pack`/`unpack`/`address`/
  `get-*`), `lambda`, `setq`, `unique` were false positives (implemented) and are
  excluded below.

**Result: ~144 of 378 primitives implemented; ~221 missing (57%).** The missing
set is not scattered — it clusters into a handful of **whole subsystems that
niiLISP has not started**: file I/O, processes, networking, dates, XML/JSON,
plus math/stats breadth. The core language (eval, lists, strings, numbers,
contexts, FOOP, FFI, bigint, arrays, regex, UTF-8) is largely complete.

## The Graphical interface chapter is the sharp end

newLISP-GS is **not built-in primitives**. `guiserver.lsp` is a newLISP module
that launches a Java Swing process (`guiserver.jar`) and drives it over **two
localhost TCP sockets** (default 64001 out / 64002 in), exchanging newline-
terminated, base64-encoded commands; inbound events arrive as newLISP source
strings and are `eval-string`'d. The `gs:*` functions all live in that `.lsp`.

So the GUI is unreachable today not for lack of `gs:*` but because its **runtime
substrate is three unbuilt subsystems**. The exact dependency chain (from the
real `guiserver.lsp` `init`/`listen`/`check-event`):

1. **Module loading + env** — `(load (append (env "NEWLISPDIR") "/guiserver.lsp"))`
   → needs `load`, `env`, `ostype`.
2. **Async child process** — `process` to launch `java -jar guiserver.jar <port>`
   (must not block, so `exec`/`fork` don't substitute for the launch).
3. **TCP client + server + I/O** — `net-connect` (out), `net-listen` + `net-accept`
   + `net-close` (in), then `net-send` / `net-receive` (with a `"\n"` terminator)
   / `net-select` (µs-timeout poll).
4. **Text codec + dynamic eval** — `base64-enc`/`base64-dec`, and `eval-string`
   to run each incoming event callback.

Minimal viable GUI ≈ `load` + `env` + `process` + the six `net-*` above +
`base64-enc/dec` + `eval-string`. Everything visual is then the stock
`guiserver.lsp` we can vendor and `load`. External prereqs: a JVM on PATH and the
`guiserver.jar`.

## Missing subsystems, by chapter (the big rocks)

### Working with files — **entirely absent** (unlocks `qa-lfs`, `qa-local-domain`, `qa-utf16path`)
Handles & streaming: `open` `close` `read` `read-char` `read-line` `read-buffer`
`read-file` `read-key` `read-utf8` `read-expr` `write` `write-char` `write-line`
`write-buffer` `write-file` `append-file` `seek` `peek` `current-line` `device`.
Modules & images: `load` `save` `source`.
Filesystem: `directory` `directory?` `file?` `file-info` `real-path` `change-dir`
`make-dir` `remove-dir` `copy-file` `rename-file` `delete-file` `exists` `search`.
Env & misc: `env` `dump` `pretty-print` `display-html`.

### Multitasking — **entirely absent** (unlocks `qa-share`, `qa-pipe`, `qa-pipefork`, `qa-cilk`, `qa-siguser`, `qa-setsig`, `qa-message`, `qa-msgbig`, `qa-cellleak`, `qa-blockmemory`, `qa-cpymem`)
Cilk API: `spawn` `sync` `abort` `send` `receive`.
OS processes: `fork` `process` `exec` `shell` `!` `pipe` `wait-pid` `destroy`.
Shared state / signals: `share` `semaphore` `signal` `sleep` `reset` `cpymem`.

### The Internet — **entirely absent** (unlocks `qa-net`, `qa-net6`, `qa-udp`, `qa-packet`, `qa-broadcast`, `qa-lookup6`)
TCP/UDP: `net-connect` `net-listen` `net-accept` `net-close` `net-send`
`net-send-to` `net-send-udp` `net-receive` `net-receive-from` `net-receive-udp`
`net-select` `net-peek`.
Info/mgmt: `net-peer` `net-local` `net-lookup` `net-service` `net-sessions`
`net-interface` `net-ipv` `net-ping` `net-packet` `net-error`.
Distributed/HTTP: `net-eval` `get-url` `put-url` `post-url` `delete-url`.
Codec: `base64-enc` `base64-dec`.

### Working with XML / JSON — **absent** (unlocks `qa-xml`, `qa-json`)
`xml-parse` `xml-type-tags` `xml-error` · `json-parse` `json-error`.

### Working with dates and times — **absent**
`date` `date-list` `date-value` `date-parse` `now` `timer` `sleep`
(`time`/`time-of-day` already exist). `date-parse` is Unix-only in newLISP too.

## Missing within otherwise-present subsystems (smaller, higher-leverage)

### Contexts / dictionaries — the biggest *near-term* win (unlocks `qa-dictionary`)
`context?` `sym` `symbols` `name` `prefix` `def-new` `delete` `bind`
`pop-assoc`, plus the default-functor **Dict/Tree** pattern (`(Ctx key val)` /
`(Ctx key)`). Already-planned slice in `CURRENT_WORK.md`; note it also wants
`save`/`load` (file I/O) for persistence.

### Lists & sequences — reference/query gaps (leaned on by the XML chapter)
`ref` `ref-all` `match` `unify` `count` `find-all` `index` `clean` `collect`
`select` `series` `difference` `intersect` `union`. Matrix ops: `mat` `det`
`invert` `transpose` `multiply`. Plus `copy`.

### Strings — `parse` is the notable one
`parse` (string → tokens/list, used everywhere in the book), `search`,
`read-expr`, `title-case`, `unicode`/`utf8` (name↔codepoint helpers), `encrypt`.

### Numbers & math
Rounding/sign: `ceil` `floor` `round` `sgn`. Integer/format: `factor` `bits`
`flt` `++` `--`. Trig/special: `atan2` `sinh` `cosh` `tanh` `asinh` `acosh`
`atanh` `erf` `beta` `betai` `binomial` `gammai` `gammaln`. Signal: `fft` `ifft`
`normal` `randomize`. Stats (`qa-statdist`/`qa-bayes`): `stats` `corr` `ssq`
`prob-*` `crit-*` `t-test` `bayes-train/query` `kmeans-train/query`. Finance:
`pmt` `pv` `fv` `npv` `nper` `irr`. Checksum: `crc32`.

### Eval / control / reflection
Eval: `eval-string` (GUI-critical) `eval-string-js`. Binding: `letn` `letex`
`bind` `default` `def-new` `curry`. Loops: `do-until` `do-while` `doargs`
`for-all`. Macros: `macro`. Errors/debug: `throw-error` `last-error` `sys-error`
`error-event` `reset` `abort` `signal` `silent` `debug` `trace` `trace-highlight`
`history`. Predicates: `context?` `primitive?` `protected?` `legal?` `quote?`
`lambda?` `macro?` `bigint?` `global?` `directory?` `file?`. Event hooks:
`reader-event` `prompt-event` `command-event` `xfer-event`. Misc: `ostype`
`sys-info` `uuid`.

Operators/reader sugar still absent: `!` (shell), `$` (regex-capture / sys var),
`:` (FOOP method colon), `~` (bit-not), `++`/`--`.

## Suggested sequencing (dependency-ordered)

1. **File I/O core** (`open`/`close`/`read-*`/`write-*`/`read-file`/`write-file`/
   `append-file`/`directory`/`real-path`/`file-info`/`env`/`load`) — foundational;
   unblocks Dict persistence, `save`/`load`, and later the GUI module load. Needs
   a grilled ADR (binary-safe handles, the ORO/`device` interaction).
2. **Dictionary API + persistence** — already the top `CURRENT_WORK` candidate;
   pairs naturally with (1)'s `save`/`load`. Unlocks `qa-dictionary`.
3. **`parse` + rounding/`sgn` + `eval-string`** — small, broadly useful, and
   `eval-string` is a hard GUI prerequisite.
4. **Processes** (`process`/`exec`/`pipe`/`fork`/`spawn`/`sync`) — a grilled ADR
   (portability, the ORO value-passing story for `share`/`spawn`).
5. **Networking** (`net-connect`/`net-listen`/`net-accept`/`net-send`/
   `net-receive`/`net-select` first; UDP/HTTP/`net-eval` later).
6. **GUI**: vendor `guiserver.lsp`, add `base64-enc/dec`, wire it up. Mostly
   integration once 1/3/4/5 exist.

XML/JSON, dates, matrix ops, and the stats/finance math are independent leaves
that can slot in whenever a target needs them (`qa-xml`, `qa-json`,
`qa-statdist`, `qa-bayes`).

## Caveats

- Counts treat operator/word aliases (`add`≈`+`) as implemented when either form
  exists; a few reflection predicates may be trivially addable.
- `docs/spec/compatibility.md` is about divergence from *traditional* Lisp, not
  from newLISP, and predates bigint/UTF-8/arrays landing — it is not the gap list.
