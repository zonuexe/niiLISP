# Ch. 10 — Working with files

niiLISP covers the core read/write/directory-navigation path solidly. The genuine gaps are unbound builtins (`copy-file`, `read-char`, `write-char`, `device`, `dump`, `pretty-print`, `search`, and no-arg `env`) and `file-info` returning a different field layout than the book.

**Coverage: 25 ✅ / 3 ⚠️ / 8 ❌**  *(updated: no-arg `save` dumps the workspace)*

> Corrections (verified against the newLISP 10.7.5 manual): `file?` returning `true` for directories is **correct** — the manual states *"This function will also return true for directories"* (pass the optional `true` flag to require a non-directory). And niiLISP's `(write-line handle str)` argument order **matches** newLISP (`(write-line [int-file [str]])`, handle first); the only real gap is the convenience form that omits the handle. Both re-classified.

| Feature | Status | Notes |
|---|---|---|
| `env` (get var) | ✅ | `(env "HOME")`, `(env "PWD")` return correct values. |
| `env` (no args, list all) | ❌ | Errors `env: expected a string` instead of returning an alist of all env vars. |
| `real-path` | ✅ | No-arg and with-path forms both return correct absolute paths. |
| `change-dir` | ✅ | Returns `true` on success, `nil` on nonexistent dir; actually changes cwd (verified via subsequent `real-path`). |
| `directory` | ✅ | No-arg, with-path, and with-regex-filter forms all work as in the book. |
| `file?` | ✅ | Returns `true` for files and directories — correct per the manual (*"will also return true for directories"*); the optional `true` flag requires a non-directory. |
| `directory?` | ✅ | Correctly `true` for directories, `nil` for regular files. |
| `file-info` | ⚠️ | Present and returns file size (index 0) and mtime correctly, but the returned list has 10 elements, not the 7 the book describes. |
| `rename-file` | ✅ | Renames and returns `true`. |
| `copy-file` | ❌ | Symbol unbound; calling it errors `not a function: nil`. |
| `delete-file` | ✅ | Deletes file, returns `true`. |
| `make-dir` | ✅ | Creates directory, returns `true`. |
| `remove-dir` | ✅ | Removes empty directory, returns `true`. |
| `open` | ✅ | `"read"`, `"write"`, `"append"`, `"update"` modes all work; returns `nil` on missing file for read. |
| `close` | ✅ | Returns `true`. |
| `read-line` | ✅ | Both `(read-line handle)` and no-arg stdin form work; loop-until-nil idiom works. |
| `current-line` | ✅ | Returns the line most recently read by `read-line`, as in the book's filter idiom. |
| `read-char` | ❌ | Symbol unbound; errors `not a function: nil`. |
| `read-buffer` | ⚠️ | Works only with a pre-bound *unquoted* symbol as the buffer arg (`(read-buffer f buf 5)`); the book's documented quoted-symbol form `(read-buffer f 'buf n)` errors `not an indexable place`. |
| `read-file` | ✅ | Reads entire file into a string, matches book output. |
| `write-line` | ⚠️ | `(write-line handle str)` works and matches newLISP's handle-first order; only the handle-less convenience form (`(write-line "text")` → current device) is unsupported (errors `expected an integer`). |
| `write-buffer` | ✅ | `(write-buffer handle str)` writes and returns the byte count. |
| `write-file` | ✅ | `(write-file path content)` writes and returns byte count. |
| `write-char` | ❌ | Symbol unbound; errors `not a function: nil`. |
| `append-file` | ✅ | Appends and returns cumulative byte count; verified across repeated calls. |
| `seek` | ✅ | `(seek f offset)` repositions, `(seek f)` with no offset returns current position; both work. |
| `device` | ❌ | Symbol unbound; `(device 0)` errors `not a function: nil` — no output-redirection support at all. |
| `load` | ✅ | Loads and evaluates a `.lsp` file, definitions take effect. |
| `save` | ✅ | `(save file sym…)` and the no-arg `(save file)` (dumps the whole workspace) both serialize loadable source (fixed 2026-07-06) |
| `source` | ✅ | `(source 'foo)` returns the function's source as a string (`(set 'foo (lambda (y) (+ y 1)))`); note the *quoted*-symbol form is required — `(source foo)` (unquoted) errors `save/source: expected symbols`. |
| `main-args` | ✅ | Full list, `((main-args) n)` indexing, and `(main-args n)` call form all return correct values. |
| Recursive directory scan (`directory` + `directory?` + `dolist`) | ✅ | The book's `search-tree` recursive-descent idiom works verbatim, including regex-filtered listing and correct recursion into subdirectories. |
| stdin filter idiom (`read-line`/`current-line`/`exit`) | ✅ | The book's `#!/usr/bin/newlisp` filter-script pattern (piping stdin through `while (read-line) ... (current-line)`) works correctly. |
| `dump` | ❌ | Symbol unbound; not checked further (not core to file I/O but listed in task scope). |
| `pretty-print` | ❌ | Symbol unbound. |
| `search` | ❌ | Symbol unbound. |

## Divergences & gaps

### `env` with no arguments doesn't list all environment variables (❌ for that form)

Book: `(env "PWD")` and similar single-var lookups are shown; newLISP's `(env)` with no args returns an alist of all environment variables.

```
$ ./target/release/niilisp -e '(println (env))'
niilisp: env: expected a string
```

### `file-info` returns 10 fields instead of the book's ~8

```
$ ./target/release/niilisp -e "(println (file-info \"testdir/a.txt\"))"
(0 33188 16777230 68091343 1 501 0 1783339225 1783339225 1783339225)
```
Indices 0 (size) and the trailing mtime-like fields are populated correctly and usable, but the shape doesn't match the book's documented 7-value layout (size, mode, device, uid, gid, atime, mtime, ctime — book text says "seven values indexed 0-7" which is itself internally inconsistent, but niiLISP's 10-element list is a distinct shape agents should not assume matches 1:1).

### `copy-file` is entirely unbound (❌)

```
$ ./target/release/niilisp -e "(println (copy-file \"testdir/a.txt\" \"testdir/a-copy.txt\"))"
niilisp: not a function: nil
```

### `read-char` is entirely unbound (❌)

```
$ ./target/release/niilisp -e "(set 'f (open \"testdir/sample.txt\" \"read\")) (println (read-char f))"
niilisp: not a function: nil
```

### `read-buffer` rejects the book's quoted-symbol form (⚠️)

Book/newLISP idiom is `(read-buffer file-handle 'buffer-symbol size)`.

```
$ ./target/release/niilisp -e "(set 'f (open \"testdir/sample.txt\" \"read\")) (set 'buf \"\") (println (read-buffer f 'buf 5))"
niilisp: not an indexable place
```
Workaround that does work in niiLISP — pass the symbol unquoted:
```
$ ./target/release/niilisp -e "(set 'f (open \"testdir/sample.txt\" \"read\")) (set 'buf \"\") (read-buffer f buf 5) (println buf)"
line1
```

### `write-line` has no handle-less (current-device) form (⚠️)

newLISP's `write-line` signature is `(write-line [int-file [str]])` — handle first, and both optional; a call with no handle writes to the current output device. niiLISP's `(write-line handle str)` matches the argument order, but requires the handle:

```
$ ./target/release/niilisp -e '(write-line "hi")'
niilisp: write-line: expected an integer
```
The explicit-handle form `(write-line f "hi")` works correctly. Only the handle-less convenience form is missing.

### `write-char` is entirely unbound (❌)

```
$ ./target/release/niilisp -e "(set 'f (open \"testdir/wc.txt\" \"write\")) (write-char f 65) (close f)"
niilisp: not a function: nil
```

### `device` is entirely unbound (❌)

Book's device-redirection example (`(device (open ...))` to redirect `println`/`print` output to a file, then `(device 0)` to restore console) has no niiLISP equivalent.

```
$ ./target/release/niilisp -e '(device 0)'
niilisp: not a function: nil
```

### ~~`save` writes an empty file with no symbol args~~ — FIXED 2026-07-06

`(save file)` with no symbols now dumps the whole workspace (all user-defined
MAIN symbols and contexts, excluding built-ins, `$`-system symbols, and unset
symbols); `(save file sym…)` dumps just the named ones. Both round-trip through
`load`.

### `dump`, `pretty-print`, `search` are unbound (❌)

```
$ ./target/release/niilisp -e '(if dump (println "bound") (println "unbound"))'
unbound
$ ./target/release/niilisp -e '(if pretty-print (println "bound") (println "unbound"))'
unbound
$ ./target/release/niilisp -e '(if search (println "bound") (println "unbound"))'
unbound
```
