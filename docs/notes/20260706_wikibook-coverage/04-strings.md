# Ch. 4 — Strings

Core string construction, case conversion, slicing, and `find`/`replace` (literal **and** regex mode, with `$0..$9` captures and per-match expression evaluation) all work as documented. Remaining gaps: `date`, `find-all`, `encrypt`, string-index `setf`/`push`/`pop`, and string-native `select`.

**Coverage: 28 ✅ / 0 ⚠️ / 7 ❌**  *(updated: regex-mode `find`/`replace`, `$0..$9`, per-match re-eval)*

| Feature | Status | Notes |
|---|---|---|
| Double-quote string syntax (`"..."`, escapes) | ✅ | |
| Brace string syntax `{...}` (no escaping) | ✅ | |
| Bracket tag syntax `[text]...[/text]` | ✅ | |
| `char` (code→char, char→code, unicode) | ✅ | |
| `string` (mixed-type concatenation) | ✅ | |
| `append` | ✅ | |
| `join` | ✅ | |
| `dup` | ✅ | |
| `date` / `date <timestamp>` | ❌ | Unbound symbol, errors as "not a function: nil" |
| `length` | ✅ | |
| `utf8len` | ✅ | correctly counts codepoints, not bytes |
| `reverse` | ✅ | |
| `upper-case` / `lower-case` | ✅ | |
| `title-case` | ✅ | |
| `setf` on string index | ❌ | `place index out of range` even for valid index 0 |
| `replace` (literal substring, destructive) | ✅ | |
| `first` / `rest` / `last` on strings | ✅ | |
| Index access `(str n)` | ✅ | |
| `slice` (positive/negative indices) | ✅ | |
| Shorthand slice `(start end str)` | ✅ | |
| `select` directly on a string | ❌ | errors `select: expected a list`; works only via `(select (explode s) ...)` |
| `chop` | ✅ | |
| `trim` (default and custom chars, 1 or 2 args) | ✅ | |
| `push` on string place | ❌ | `push: place is not a list` |
| `pop` on string place | ❌ | `pop: place is not a list` / `not a valid place` |
| `find` (literal substring) | ✅ | |
| `find` with regex flag `0` | ✅ | Engages the regex engine, returns the match offset, binds `$0..$N` (fixed 2026-07-06) |
| `find-all` | ❌ | Unbound symbol |
| `starts-with` / `ends-with` | ✅ | |
| `regex` (capture groups, positions) | ✅ | returns full match/position/group tuple correctly |
| `member` on strings | ✅ | |
| `replace` with regex flag `0` | ✅ | Regex substitution, replacement re-evaluated per match (fixed 2026-07-06) |
| `$0`..`$9` capture variables (regex/replace context) | ✅ | Bound by `regex`, `find` (regex), and `replace` (fixed 2026-07-06) |
| `replace` with dynamic expression, per-match re-evaluation | ✅ | Re-evaluated once per match with `$0..$N` bound, in both the literal and regex forms (fixed 2026-07-06) |
| String comparison (`<`,`>`,`=`, multi-arg, case-sensitivity) | ✅ | |
| `explode` (char list, and chunk-size form) | ✅ | |
| `parse` (literal delimiter and regex delimiter) | ✅ | |
| `format` (`%s`, `%d`, `%x`, width specs) | ✅ | |
| `eval-string` | ✅ | |
| `read-file` | ✅ | (initial nil result was a bad test path, not a real bug) |
| `encrypt` | ❌ | Unbound symbol |

## Divergences & gaps

### `date` is unbound
```
$ echo '(println (date))' | niilisp -
niilisp: not a function: nil
```
Same failure for `(date 1230000000)`. Any date-to-string workflow from the chapter is unusable.

### `setf` cannot mutate a character by string index
```
$ echo '(set (quote t2) "cream") (setf (t2 0) "d") (println t2)' | niilisp -
niilisp: place index out of range
```
Confirmed the index itself is valid — the equivalent `(setf (lst 0) 99)` on a list works fine. String places are simply not supported by `setf`.

### `select` does not accept a string directly
```
$ echo '(set (quote t5) "abcdefghij") (println (select t5 1 3 5 7))' | niilisp -
niilisp: select: expected a list
```
Book's example `(select t 1 3 5 7)` on a string errors. Workaround `(select (explode t5) ...)` returns the expected `("b" "d" "f" "h")`.

### `push` / `pop` reject string places
```
$ echo '(set (quote t6) "some ") (push "text " t6 -1) (println t6)' | niilisp -
niilisp: push: place is not a list

$ echo '(set (quote t7) "hello world") (println (pop t7 -1 2))' | niilisp -
niilisp: pop: place is not a list
```
Both push and pop assume list-only places; the string-place overload from the book is missing.

### ~~Regex mode (`0` flag) on `find`/`replace`~~ — FIXED 2026-07-06
```
$ niilisp -e '(println (find "h.l" "hello world" 0))'
0
$ niilisp -e '(println (replace "h.l" "hello world" "X" 0))'
Xlo world
```
Both now engage the regex engine and bind `$0..$N`. (Note the `.lsp`-string
escaping: a regex `\w` is written `"\\w"` in niiLISP source.)

### `find-all` is unbound
```
$ echo '(println (find-all "[aeiou]{2,}" "beautiful ocean maintain" $0))' | niilisp -
niilisp: not a function: nil
```

### ~~`$0`..`$9` capture variables never populated~~ — FIXED 2026-07-06
```
$ niilisp -e '(regex "(l+)" "hello")(println $0 " " $1)'
ll ll
$ niilisp -e '(println (replace "o" "hello" (upper-case $0)))'
hellO
```
`regex`, regex-mode `find`, and `replace` now bind `$0` (whole match) and
`$1..$N` (groups) as ordinary globals that persist until the next regex op.

### ~~`replace` evaluates its expression once, not per match~~ — FIXED 2026-07-06
```
$ niilisp -e '(set (quote counter) 0)(set (quote text) "xaxbxcx")(replace "x" text (begin (inc counter) (string counter)))(println text " " counter)'
1a2b3c4 4
```
`text` becomes `1a2b3c4` with `counter` at 4 — one evaluation per match — in
both the literal 3-arg and the regex 4-arg forms.

### `encrypt` is unbound
```
$ echo '(println (encrypt "hello" "key"))' | niilisp -
niilisp: not a function: nil
```
