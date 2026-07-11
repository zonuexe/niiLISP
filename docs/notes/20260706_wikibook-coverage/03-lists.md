# Ch. 3 — Lists

Audit of niiLISP against the *Introduction to newLISP* WikiBook chapter ["Lists"](https://en.wikibooks.org/wiki/Introduction_to_newLISP/Lists), function by function, verified by running the real binary and diffing actual output against the book's documented output (not just absence of an error — niiLISP silently returns `nil` for unbound symbols and only errors with `not a function: nil` when that `nil` is then called).

**Coverage: 45 ✅ / 0 ⚠️ / 0 ❌**

> Correction (verified against the newLISP 10.7.5 manual): `push` is **not** a divergence — the manual states *"The list changed is returned as a reference,"* so returning the whole mutated list is correct newLISP behavior. Re-classified ✅.
>
> Update (2026-07-06): the whole chapter now passes. `clean`/`index`/`exists`/`for-all`/`transpose` landed first; then `ref`/`ref-all`/`match`/`find-all`/`pop-assoc` (plus the `push`/`pop` index-vector forms) under **[ADR-0036](../../adr/0036-reference-and-query-model.md)**.

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
| `transpose` | ✅ | implemented (2026-07-06); ragged rows padded with `nil` per newLISP |
| `explode` (with chunk size) | ✅ | matches |
| `find` | ✅ | matches |
| `member` | ✅ | matches |
| `ref` | ✅ | implemented 2026-07-06 (ADR-0036); index path to first match, `()` if none |
| `ref-all` | ✅ | implemented 2026-07-06; all index paths (or elements with trailing `true`); sets `$count` |
| `filter` | ✅ | matches |
| `clean` | ✅ | implemented (2026-07-06); `filter` with a negated predicate |
| `index` | ✅ | implemented (2026-07-06); indices where the predicate holds |
| `exists` | ✅ | implemented (2026-07-06); first matching element, else `nil` |
| `for-all` | ✅ | implemented (2026-07-06) |
| `match` | ✅ | implemented 2026-07-06; `?`/`+`/`*` wildcards, nestable, with backtracking |
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
| `pop-assoc` | ✅ | implemented 2026-07-06; removes and returns the `(key …)` pair from an assoc-list place |
| `map` | ✅ | matches |
| `dolist` | ✅ | matches |
| `find-all` | ✅ | implemented 2026-07-06; regex / list-pattern / key forms with an optional per-hit transform |
| `last` | ✅ | matches |
| `first` / `rest` | ✅ | matches |

## Divergences & gaps

All features in this chapter are implemented and match newLISP. The
reference/query family (`ref`/`ref-all`/`match`/`find-all`/`pop-assoc`) is
documented in [ADR-0036](../../adr/0036-reference-and-query-model.md); `push`/`pop`
also gained index-vector forms so `(pop L (ref key L))` works. A couple of
worked spot-checks:

```
$ niilisp -e "(println (ref 4 '((1 2) (1 2 3) (1 2 3 4))))"
(2 3)
$ niilisp -e "(println (match '(* 1 2 3 *) '(7 9 3 8 1 2 3 4 5)))"
((7 9 3 8) (4 5))
$ niilisp -e "(println (find-all '(? 9) '((Beethoven 9) (Bruckner 9))))"
((Beethoven 9) (Bruckner 9))
```
