# Ch. 5 — Apply and map

niiLISP covers the basic `apply`/`map` mechanics and `fn` closures-as-values, but silently ignores most of the list when `apply` is given a count (int-reduce) argument, and has no `curry` at all.

**Coverage: 6 ✅ / 0 ⚠️ / 2 ❌**

> Correction: `+` on floats is **not** a divergence. In newLISP `+` is integer arithmetic (it truncates float operands to ints) and `add` is the float version — they are *not* aliases. `(+ 2.1 3.2)` → `5` is correct newLISP behavior. Re-classified ✅.

| Feature | Status | Notes |
|---|---|---|
| `apply` (basic, spreads list as args) | ✅ | `(apply + (list 1 2 3 4))` → `10` |
| `map` (single list) | ✅ | `(map floor (list 1.2 3.7 -2.3))` → `(1 3 -3)` |
| `map` (multiple lists) | ✅ | `(map append '("cats " "dogs " "birds ") '("miaow" "bark" "tweet"))` → matches book |
| `map` (>2 lists, stops at shortest) | ✅ | 3-list and unequal-length cases behave as documented |
| `fn` / anonymous functions as values | ✅ | `(apply (fn (x y) (+ x y)) (list 3 4))` → `7` |
| `apply` with count argument (fold/reduce over whole list) | ❌ | Only makes a **single call** using the first N elements; rest of the list is silently discarded |
| `+` on floats | ✅ | `(+ 2.1 3.2)` → `5` — correct: newLISP `+` is integer arithmetic (truncates floats); use `add` for float sums |
| `curry` | ❌ | Unbound; `(curry f x)` errors instead of returning a partial-application function |

## Divergences & gaps

### `apply` with count argument does not fold over the whole list

Book example (`longest` repeatedly applied 2-at-a-time across a 15-item list) is documented to return `turquoise` (the longest word found by folding across all elements). niiLISP instead only ever invokes the function **once**, on the first 2 elements, and discards the remaining 13.

Repro:
```
(define (longest a b) (println "call a=" a " b=" b) (if (> (length a) (length b)) a b))
(println (apply longest (list "green" "purple" "violet" "yellow" "orange" "black" "white" "pink" "red" "turquoise" "cerise" "scarlet" "lilac" "grey" "blue") 2))
```
Actual output:
```
call a=green b=purple
purple
```
Expected (per book): `turquoise`, with the fold making a call for every pair consumed down the list. Confirmed with a plain counter function too:
```
(define (f a b) (println "call: a=" a " b=" b) (+ a b))
(println (apply f (list 1 2 3 4 5 6) 2))
```
Actual: single call `a=1 b=2`, result `3`. The count arg is accepted syntactically but its fold semantics are not implemented — it just truncates the list to the first `count` elements and calls once.

### `curry` is not implemented

Book section builds `curry` examples for partial application (`(curry dup { })`, `(map (curry pow 2) (sequence 1 10))`).

Repro:
```
(println (curry + 10))
```
Actual output:
```
niilisp: not a function: nil
```
`curry` is unbound (returns `nil` when referenced directly), so calling the "curried" result immediately fails with `not a function: nil`. No alternate name (`partial`, `curried`) is bound either. There is no built-in partial-application idiom to substitute — users must hand-roll a closure, and even that path is unrelated to this builtin's absence.
