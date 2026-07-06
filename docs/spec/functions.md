# niiLISP Built-in Functions

A quick reference for the built-in **functions** — primitives whose arguments
are all evaluated before the call. Special forms (which control evaluation) are
in [`special-forms.md`](special-forms.md); the language overview is in
[`syntax.md`](syntax.md).

> This is a preliminary, deliberately terse list — one line per function — to be
> organised and expanded later. Signatures use `name arg…`; `[x]` is optional,
> `x…` is variadic.

## Integer arithmetic (wrapping)

| Function | Meaning |
| --- | --- |
| `(+ n…)` | sum |
| `(- n…)` | difference; `(- n)` negates |
| `(* n…)` | product |
| `(/ n…)` | integer quotient; error on divide by zero |
| `(% a b)` | integer remainder; error on zero divisor |
| `(gcd a b…)` | greatest common divisor (accepts bigints; `bigint` feature) |
| `(min n…)` `(max n…)` | smallest / largest argument (type preserved) |
| `(even? n)` `(odd? n)` | integer parity (accepts bigints) |

A bigint operand makes `+ - * / %` compute in arbitrary precision (see
[`types.md`](types.md) → bigint); a float operand is truncated to an integer.

## Float arithmetic and math

| Function | Meaning |
| --- | --- |
| `(add n…)` `(sub n…)` `(mul n…)` `(div n…)` | float `+ - * /` |
| `(mod a b)` | float modulo; NaN on zero divisor |
| `(pow x y)` | `x` to the power `y` |
| `(sqrt x)` `(exp x)` `(log x [base])` | root, exp, log (natural, or base) |
| `(sin x)` `(cos x)` `(tan x)` `(asin x)` `(acos x)` `(atan x)` | trigonometry |
| `(abs x)` | absolute value (integer or float) |
| `(NaN? x)` `(inf? x)` | NaN / infinity predicates |

## Conversion

| Function | Meaning |
| --- | --- |
| `(int x [default])` | to integer (float truncates/saturates; string parses; bigint → low 64 bits) |
| `(float x [default])` | to float |
| `(bigint x)` | to an arbitrary-precision integer (number or numeric string; `bigint` feature) |
| `(char n)` / `(char "s")` | code point to 1-char string / first code point of string |

## Comparison

| Function | Meaning |
| --- | --- |
| `(= a…)` `(!= a…)` | structural equality / inequality |
| `(< a…)` `(> a…)` `(<= a…)` `(>= a…)` | ordering of numbers or strings (NaN compares false) |

## Bitwise

| Function | Meaning |
| --- | --- |
| `(& n…)` `(\| n…)` `(^ n…)` | bitwise and / or / xor |
| `(<< x n)` `(>> x n)` | shift left / right |

## Lists and sequences

| Function | Meaning |
| --- | --- |
| `(list x…)` | make a list |
| `(cons x lst)` | prepend (no dotted pairs; else a 2-list) |
| `(first lst)` `(rest lst)` `(last lst)` | head / tail / final element (a string's first/last/rest are by **character**) |
| `(nth i lst)` | element `i` (negative from the end); a string's `i`-th **character** |
| `(length x)` | list/array length, or string **byte** length |
| `(utf8len str)` | string **character** (code point) count |
| `(append lst…)` | concatenate lists (or strings) into a copy |
| `(sequence from to [step])` | list of numbers, inclusive |
| `(flat lst)` | flatten a nested list to a single level |
| `(member key seq)` | tail of a list / substring of a string from the first match |
| `(unique lst)` | copy with duplicate elements removed (first kept) |
| `(join lst [sep])` | concatenate a list of strings with an optional separator |
| `(array size [init])` | a fixed-length array (cycle-fill / nil-fill); see [`types.md`](types.md) |
| `(array-list arr)` | a plain list copy of an array |

List construction semantics (array-backed values, no dotted pairs) are in
[`types.md`](types.md). Destructive list operators (`push`, `pop`, `swap`, `reverse`,
`sort`, `rotate`, `replace`, `set-ref`) are **special forms** — see
[`special-forms.md`](special-forms.md).

## Higher-order

| Function | Meaning |
| --- | --- |
| `(map f lst…)` | apply `f` across the lists |
| `(apply f lst)` | call `f` with the list's elements as arguments |
| `(filter pred lst)` | keep elements where `pred` is truthy |
| `(dup x n)` | repeat a string `n` times, or make a list of `n` copies |

## Association lists

| Function | Meaning |
| --- | --- |
| `(assoc key alist)` | the `(key …)` pair, or `nil` |
| `(lookup key alist [i])` | element `i` (default last) of the matching pair |

## Predicates

| Function | Meaning |
| --- | --- |
| `(nil? x)` / `(null? x)` | is `nil` |
| `(true? x)` | not `nil` and not the empty list/array |
| `(integer? x)` `(float? x)` `(number? x)` | numeric kind |
| `(string? x)` `(symbol? x)` `(list? x)` `(array? x)` | type |
| `(atom? x)` | not a list or array |
| `(zero? x)` | numerically zero |
| `(empty? x)` | empty list/string, or `nil` |
| `(not x)` | logical negation (`nil`/`()` are false) |

## Strings

| Function | Meaning |
| --- | --- |
| `(string x…)` | concatenate arguments to a string |
| `(starts-with s prefix)` `(ends-with s suffix)` | prefix / suffix test |
| `(upper-case s)` `(lower-case s)` | Unicode case folding (invalid bytes unchanged) |
| `(trim s [l [r]])` | strip a char (default space) from both ends, or `l`/`r` per side |
| `(slice seq start [len])` | copied sub-range of a string or list (see below) |
| `(find key seq)` | index of a substring / list element, else `nil` |
| `(explode seq [n])` | split into `n`-wide pieces (default 1); a string splits by **character** |
| `(chop seq [n])` | copy without the last `n` bytes/elements (default 1) |
| `(regex pat text [opt [off]])` | first match `(str byte-off byte-len …captures…)` or `nil` (`regex` feature) |
| `(regex-comp pat [opt])` | precompile a pattern (cache); returns it, errors if malformed |
| `(format fmt arg…)` | printf-style: flags, width, `.precision`; `d i u f e g x X o s c` |

`slice` and `find` index strings by **byte** (ADR-0013). In `slice`, a negative
`start` counts from the end and a negative `len` drops that many trailing
elements; out-of-range bounds clamp. `find` on a list compares elements
structurally (`=`). Character-oriented string ops (`nth` / `(str i)` /
`first` / `rest` / `last` / `explode` / `utf8len`) work on UTF-8 character
boundaries, while byte-oriented ops (`slice`, the implicit slice `(i str)`,
`length`, substring search) stay byte-based for binary content (ADR-0025).
`regex` uses the pure-Rust `regex` crate (RE2-style, not PCRE): classes,
quantifiers, groups, alternation and anchors work, but **backreferences and
lookaround do not** (ADR-0028). Matching and offsets are byte-based.

## Evaluation, objects, system

| Function | Meaning |
| --- | --- |
| `(eval expr)` | evaluate a value as code |
| `(expand expr sym…)` | substitute the named symbols' values into `expr`; `(expand expr)` auto-substitutes upper-case symbols bound to code |
| `(args [i])` | the current function's arguments not bound to a parameter, or the `i`th |
| `(new prototype 'name)` | create a context, copying `prototype`'s symbols (its default functor `Proto:Proto` maps to `name:name`) |
| `(delete 'sym)` | clear a symbol, or a whole context (all `Ctx:*`) for a bare context name |
| `(sys-info [n])` | interpreter statistics (best-effort; element 0 ≈ live cells) |
| `(randomize list)` | a shuffled copy of `list` |
| `(term sym)` | a symbol's unqualified term (`(term 'L:a)` → `a`); see `context` |
| `(print x…)` `(println x…)` | write to stdout (no quotes; `println` adds a newline) |
| `(time-of-day)` | milliseconds since the epoch |
| `(set-locale ["C"])` | locale (currently a no-op returning `"C"`) |
| `(main-args [i])` | the process command line as a list, or its `i`th element |
| `(seed n)` | reseed the RNG, returning the previous seed |
| `(rand max [count])` | random integer in `[0, max)`, or a list of `count` |
| `(random [offset scale [count]])` | uniform float in `[0,1)` or `[offset, offset+scale)` |
| `(exit [code])` | terminate the process |

The RNG is a seedable xorshift generator shared across `rand`/`random`/`amb`;
`(random offset scale)` is **uniform** (newLISP's exact distribution is not
reproduced). `amb` is a special form (see `special-forms.md`).

## Dictionaries (ADR-0030)

A context whose default functor `Ctx:Ctx` is **nil** acts as a string/number-keyed
**Dictionary** (hash). Applying it dispatches on the first argument:

| Form | Meaning |
| --- | --- |
| `(Ctx key value)` | set `key` to `value` (a nil `value` deletes the key) |
| `(Ctx key)` | get the value for `key`, or `nil` |
| `(Ctx assoc-list)` | bulk-load a list of `(key value)` pairs; returns the context |
| `(Ctx)` | all `(key value)` pairs, sorted by key |

Keys are stored as context symbols named `_` + the key (so a Dictionary is a
context of `_`-prefixed symbols, enumerable with `dotree` and — later — savable);
a number key and its string form collapse to one entry. A context with a
**non-nil** functor (a `lambda`, or the predefined `Class` marker copied by `new`)
is a FOOP class instead: applying it constructs a tagged object list. See
`special-forms.md` (`context`) and the FOOP notes.

## File I/O and filesystem (ADR-0029)

Always compiled in. A **handle** is an opaque integer from an interpreter-side
registry; `0`/`1`/`2` are stdin/stdout/stderr. Operational failures (a missing
file, EOF, a bad handle) return `nil`; only type misuse raises an error. Paths
are byte-buffer strings, binary-safe on Unix.

| Function | Meaning |
| --- | --- |
| `(open path mode)` | open `path` (`mode` = `read`/`write`/`append`/`update`); a handle, or `nil` |
| `(close handle)` | close a handle; `true`, or `nil` if not open |
| `(seek handle [pos])` | with `pos`: seek (absolute; `-1` = end); without: the current position |
| `(read-buffer handle place size [wait])` | read ≤ `size` bytes (or until `wait`) into `place` (a symbol); returns the byte count |
| `(write-buffer handle str [size])` | write `str` (≤ `size` bytes); returns the byte count |
| `(read-line [handle])` | a line (terminator stripped) from `handle` (default stdin), or `nil` at EOF |
| `(current-line)` | the most recent `read-line` result |
| `(read-file path)` | the whole file as a string, or `nil` |
| `(write-file path str)` / `(append-file path str)` | write/append the whole string; returns the byte count, or `nil` |
| `(directory [path [pattern]])` | entry names (incl. `.`/`..`); `pattern` filters under the `regex` feature |
| `(real-path [path])` | the canonical absolute path, or `nil` |
| `(make-dir path)` / `(remove-dir path)` | create / remove a directory; `true` or `nil` |
| `(change-dir path)` | change the working directory; `true` or `nil` |
| `(rename-file old new)` / `(delete-file path)` | rename / delete; `true` or `nil` |
| `(file? path)` / `(directory? path)` | whether `path` exists / is a directory |
| `(file-info path [i])` | a 10-int list `(size mode device inode links uid gid atime mtime ctime)` (0 where a platform lacks a field), or element `i` |
| `(env name [value])` | get an environment variable (string/`nil`); with `value` set it (a `nil` value unsets), returning `true` |
| `(source 'sym…)` | loadable niiLISP source for the symbols/contexts, as a string |
| `(save path 'sym…)` | write `source` of the symbols to `path`; `true` or `nil` |
| `(load path)` | read and evaluate a file, returning the last form's value, or `nil` |

`read-buffer` is a special form (its `place` is unevaluated). `save`/`source`
serialise a context deterministically (sorted members), so re-saving an
unchanged Dictionary is byte-identical — the basis of `qa-dictionary`'s
round-trip check.

## External processes (ADR-0031)

Always compiled in, cross-platform (`std::process::Command`). A pid is a plain
integer; failures return `nil`.

| Function | Meaning |
| --- | --- |
| `(process "cmd arg…")` | spawn an external command (whitespace-split argv), non-blocking; returns the child pid, or `nil` |
| `(exec "cmd" [instr])` | run `cmd` through the shell to completion; returns stdout as a list of lines; `instr` is fed to stdin |
| `(! "cmd")` | run `cmd` through the shell with inherited stdio; returns its exit code |
| `(sleep ms)` | pause for `ms` milliseconds, returning `ms` |

The fork-based **Cilk API** (`spawn`/`sync`/`abort`/`share`/`send`/`receive`,
`fork`/`pipe`/`wait-pid`, `signal`) is a separate Unix-only, `mt`-gated slice
(ADR-0032).

## Networking (Unix, `net` feature — ADR-0033)

Stream sockets. A socket is a file **handle** (a raw fd), so `net-send`/
`net-receive`/`net-close` reuse the file-I/O machinery. Sockets are blocking;
`net-select` polls for readiness. A single string address is a Unix-domain path;
a host + port is TCP.

| Function | Meaning |
| --- | --- |
| `(net-connect host port)` / `(net-connect "/path")` | connect (TCP / Unix domain); a socket handle, or `nil` |
| `(net-listen port)` / `(net-listen "/path")` | a listening socket |
| `(net-accept lsock)` | accept one connection (blocking); a connected handle |
| `(net-send sock str)` | send bytes; returns the count |
| `(net-receive sock place maxlen [wait])` | receive ≤ `maxlen` bytes into `place`; returns the count (a special form, like `read-buffer`) |
| `(net-select sock "read"/"write" ms)` | poll one socket; the socket if ready, else `nil` |
| `(net-peek sock)` | bytes available to read |
| `(net-peer sock)` / `(net-local sock)` | the remote / local address (best-effort string) |
| `(net-close sock)` | close the socket |

Deferred: UDP, `net-eval` / server mode, raw `net-packet` / `net-ping`,
`net-lookup`, and `get-url` / HTTP.

## FFI (Unix, `ffi` feature)

| Function | Meaning |
| --- | --- |
| `(import "lib" "fn" "ret" "arg"…)` | bind a C function; `nil` if unresolved |
| `(callback 'func "ret" "arg"…)` | a C function pointer that calls `func` |
| `(struct 'name "type"…)` | bind `name` to a struct layout (a list of C types) |
| `(pack layout val…)` | serialise values to a binary string |
| `(unpack layout str)` | read a packed string back into a list of values |
| `(get-string addr [len [limit]])` | read a C string at `addr` |
| `(get-int addr)` `(get-long addr)` | read a 32-/64-bit integer at `addr` |
| `(get-float addr)` `(get-char addr)` | read a C `double` / signed byte at `addr` |
| `(address 'sym)` | stable buffer address of a symbol-held string |

Types: `void int long float double char* void*`. See `syntax.md` §10.

A packed struct is passed to C by handing the string to a `void*` argument (no
copy, binary-safe, valid for the call). With a **struct** layout, `pack`/`unpack`
use the native C ABI layout — natural alignment, padding, and byte order — so a
packed string is exactly what a C function accepts as that struct.

`pack`/`unpack` alternatively take a **format string** (packed tightly, no
alignment): `c`/`b` = signed/unsigned 8-bit, `d`/`u` = 16-bit, `ld`/`lu` =
32-bit, `Ld`/`Lu` = 64-bit, `f` = float, `lf` = double, `sN` = an N-byte string,
`nN` = N null bytes; `>` / `<` switch the following fields to big-/little-endian
(default: native). Whitespace between specifiers is ignored — e.g.
`(pack "c c c" 65 66 67)` → `"ABC"`, `(unpack ">ld lf" s)`.

A NULL (0) pointer through
`unpack` (for a `char*` field) or `get-string` raises an error rather than
dereferencing; other invalid addresses are undefined behaviour (the caller's
risk, per ADR-0015). `address` is valid only while `sym` is neither reassigned
nor resized.
