# Ch. 3 — Lists

Audit of niiLISP against the *Introduction to newLISP* WikiBook chapter ["Lists"](https://en.wikibooks.org/wiki/Introduction_to_newLISP/Lists), function by function, verified by running the real binary and diffing actual output against the book's documented output (not just absence of an error — niiLISP silently returns `nil` for unbound symbols and only errors with `not a function: nil` when that `nil` is then called).

**Coverage: 32 ✅ / 1 ⚠️ / 10 ❌**

> Correction (verified against the newLISP 10.7.5 manual): `push` is **not** a divergence — the manual states *"The list changed is returned as a reference,"* so returning the whole mutated list is correct newLISP behavior. Re-classified ✅.

| Feature | Status | Notes |
|---|---|---|
| `list` | ✅ | matches |
| `cons` | ✅ | matches |
| `append` | ✅ | matches |
| `push` | ✅ | mutates/inserts correctly; returns the changed list as a reference (per newLISP manual) |
| `dup` (2-arg) | ✅ | string concatenation form matches |
| `dup` (3-arg `true` flag) | ✅ | `(dup "x" 6 true)` → list of strings (fixed 2026-07-06) |
| `reverse` | ✅ | matches |
| `sort` | ✅ | matches |
| `unique` | ✅ | matches |
| `flat` | ✅ | matches |
| `transpose` | ❌ | not implemented (unbound) |
| `explode` (with chunk size) | ✅ | matches |
| `find` | ✅ | matches |
| `member` | ✅ | matches |
| `ref` | ❌ | not implemented (unbound) |
| `ref-all` | ❌ | not implemented (unbound) |
| `filter` | ✅ | matches |
| `clean` | ❌ | not implemented (unbound) |
| `index` | ❌ | not implemented (unbound) |
| `exists` | ❌ | not implemented (unbound) |
| `for-all` | ❌ | not implemented (unbound) |
| `match` | ❌ | not implemented (unbound) |
| `count` | ✅ | matches (verified against correct book sentence) |
| `nth` | ✅ | matches |
| `select` | ✅ | matches, including negative indices |
| `slice` | ✅ | matches |
| implicit list addressing `(r 1)` | ✅ | matches |
| implicit range addressing `(start len lst)` | ✅ | matches |
| `pop` | ✅ | matches, mutates in place |
| `chop` | ✅ | matches, 1-arg and 2-arg forms |
| `setf` on nth-element | ✅ | matches |
| `replace` | ✅ | matches |
| `set-ref` | ✅ | matches |
| `set-ref-all` | ✅ | matches |
| `swap` | ✅ | matches |
| `difference` | ✅ | matches |
| `intersect` | ✅ | matches |
| `assoc` | ✅ | matches |
| `lookup` | ✅ | matches |
| `pop-assoc` | ❌ | not implemented (unbound) |
| `map` | ✅ | matches |
| `dolist` | ✅ | matches |
| `find-all` | ❌ | not implemented (unbound) |
| `last` | ✅ | matches |
| `first` / `rest` | ✅ | matches |

## Divergences & gaps

### ~~`dup`'s third `true` argument (list-of-strings mode) is ignored~~ — FIXED 2026-07-06

```
$ niilisp -e '(println (dup "x" 6 true))'
("x" "x" "x" "x" "x" "x")
```
The plain 2-arg form `(dup "x" 6)` → `"xxxxxx"` still concatenates.

### ❌ `transpose` — unbound

```
$ echo '(println (transpose (quote (("a" 1) ("b" 2) ("c" 3)))))' | niilisp -
niilisp: not a function: nil
```
Expected: `(("a" "b" "c") (1 2 3))`

### ❌ `ref` — unbound

```
$ echo '(println (ref 4 (quote ((1 2) (1 2 3) (1 2 3 4)))))' | niilisp -
niilisp: not a function: nil
```
Expected: `(2 3)`

### ❌ `ref-all` — unbound

```
$ echo '(println (ref-all "of" (list "this" "is" "of" "strings" "of" "integers")))' | niilisp -
niilisp: not a function: nil
```
Expected: `((2) (4))`

### ❌ `clean` — unbound

```
$ echo '(println (clean integer? (list 0 1 2 3 4.01 5 6 7 8 9.1 10)))' | niilisp -
niilisp: not a function: nil
```
Expected: `(4.01 9.1)`

### ❌ `index` — unbound

```
$ echo '(println (index (fn (x) (> (length x) 3)) (list "hi" "world" "test")))' | niilisp -
niilisp: not a function: nil
```
Expected: `(1)`

### ❌ `exists` — unbound

```
$ echo '(println (exists string? (list 1 2 3 4 5 "hello" 7)))' | niilisp -
niilisp: not a function: nil
```
Expected: `"hello"`

### ❌ `for-all` — unbound

```
$ echo '(println (for-all number? (list 1 2 3 4 5 6 7)))' | niilisp -
niilisp: not a function: nil
```
Expected: `true`

### ❌ `match` — unbound

```
$ echo '(println (match (quote (* 1 2 3 *)) (quote (7 9 3 8 1 2 3 4 5))))' | niilisp -
niilisp: not a function: nil
```
Expected: `((7 9 3 8) (4 5))`

### ❌ `pop-assoc` — unbound

```
$ echo '(set (quote planets2) (list (list "Mercury" 0.382) (list "Pluto" 0.1))) (println (pop-assoc "Pluto" planets2))' | niilisp -
niilisp: not a function: nil
```
Expected: `("Pluto" 0.1)`, with `planets2` mutated to drop that entry.

### ❌ `find-all` — unbound

```
$ echo '(println (find-all (quote (? 9)) (quote ((Beethoven 9) (Bruckner 9)))))' | niilisp -
niilisp: not a function: nil
```
Expected: `((Beethoven 9) (Bruckner 9))`
