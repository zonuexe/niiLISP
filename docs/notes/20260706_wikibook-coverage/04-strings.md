# Ch. 4 — Strings

Core string construction, case conversion, slicing, and simple literal `find`/`replace` all work as documented; the chapter's regex-mode features (`0` flag on `find`/`replace`, `$0..$9` capture variables, per-match expression evaluation, `find-all`), `date`, string-index `setf`/`push`/`pop`, and string-native `select` are broken or missing.

**Coverage: 24 ✅ / 3 ⚠️ / 8 ❌**

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
| `find` with regex flag `0` | ❌ | Silently falls back to literal match; returns `nil` for patterns like `h.l` or `l+` that should match |
| `find-all` | ❌ | Unbound symbol |
| `starts-with` / `ends-with` | ✅ | |
| `regex` (capture groups, positions) | ✅ | returns full match/position/group tuple correctly |
| `member` on strings | ✅ | |
| `replace` with regex flag `0` | ❌ | Pattern never matches/substitutes; string returned unchanged |
| `$0`..`$9` capture variables (regex/replace context) | ❌ | Never bound; referencing `$0` errors "expected a string" / reads as `nil` |
| `replace` with dynamic expression, per-match re-evaluation | ⚠️ | Expression is evaluated once and the single result is reused for every match, instead of being re-evaluated per match |
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

### Regex mode (`0` flag) on `find`/`replace` does not actually engage the regex engine
```
$ echo '(println (find "h.l" "hello world" 0))' | niilisp -
nil
$ echo '(println (find "l+" "hello world" 0))' | niilisp -
nil
$ echo '(println (replace "h.l" "hello world" "X" 0))' | niilisp -
hello world
```
Non-literal patterns silently fail to match even though `(regex ...)` itself works correctly with the same patterns. Only a literal substring passed with the `0` flag succeeds (falls through to plain substring semantics), so any workflow depending on regex-mode `find`/`replace` silently no-ops instead of erroring.

### `find-all` is unbound
```
$ echo '(println (find-all "[aeiou]{2,}" "beautiful ocean maintain" $0))' | niilisp -
niilisp: not a function: nil
```

### `$0`..`$9` capture variables are never populated
```
$ echo '(println (replace "o" "hello" $0))' | niilisp -
hello
$ echo '(regex "l" "hello") (println $0)' | niilisp -
nil
```
Neither `regex` nor `replace` binds `$0`/`$1`... after a match, even though `regex`'s direct return value does include the correct captured substrings and offsets. The book's canonical idiom `(replace "o" "hello" (upper-case $0) 0)` cannot work because `$0` is always `nil`, causing a downstream type error (`upper-case/lower-case: expected a string`) whenever the expression tries to use it.

### `replace` with a dynamic expression evaluates the expression once, not per match
```
$ echo '(set (quote counter) 0) (set (quote text) "xaxbxcx") (replace "x" text (begin (inc counter) (string counter))) (println text) (println counter)' | niilisp -
1a1b1c1
1
```
Expected (per newLISP semantics) `text` to become `1a2b3c4` with `counter` ending at 4 (one increment per match). Instead the replacement expression runs exactly once and its single result ("1") is spliced into every match position, and `counter` stays at 1.

### `encrypt` is unbound
```
$ echo '(println (encrypt "hello" "key"))' | niilisp -
niilisp: not a function: nil
```
