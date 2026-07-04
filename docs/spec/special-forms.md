# niiLISP Special Forms

A per-form reference for niiLISP's special forms, companion to
[`syntax.md`](syntax.md) (which gives the overview in §6). A **special form**
looks like a call `(name arg…)` but controls whether and how its arguments are
evaluated — that is what distinguishes it from an ordinary function (whose
arguments are all evaluated first) and from a `fexpr` (whose arguments are all
left unevaluated). Each entry below states this explicitly under _"evaluates"_.

Notation follows `syntax.md`: `body` is a sequence of expressions whose value is
that of the last one (`nil` if empty); `place` is a location as defined in
`syntax.md` §8.

---

## Quoting

### `quote` — suppress evaluation

```
(quote x)      'x
```

_Evaluates:_ nothing. Returns `x` unevaluated.

```
'(a b c)       ; -> (a b c), the literal list
(quote sym)    ; -> the symbol sym, not its value
```

---

## Definition and functions

### `define` — bind a name

```
(define name value)              ; value form
(define (name param…) body)      ; function form
```

_Evaluates:_ the value (value form) or nothing but the body-on-call (function
form). `name` is a literal symbol; the function form is sugar for binding `name`
to `(lambda (param…) body)`. A context-qualified name (`Ctx:member`) also makes
the bare `Ctx` symbol a context (§ FOOP in `syntax.md`). A `param` is a symbol
or `(symbol default)` — the default is used when the argument is missing.

```
(define pi 3.14159)
(define (sq x) (* x x))          ; (sq 9) -> 81
(define (greet (who "world"))    ; default parameter
  (string "hi, " who))
```

### `lambda` / `fn` — anonymous function

```
(lambda (param…) body)     (fn (param…) body)
```

_Evaluates:_ nothing now; the body is evaluated per call with arguments bound to
the parameters (dynamically). `fn` is an alias.

### `lambda-macro` — anonymous fexpr

```
(lambda-macro (param…) body)
```

_Evaluates:_ nothing; when called, the **arguments are passed unevaluated** and
bound to the parameters. It is a runtime fexpr, not a hygienic macro — it
decides what to evaluate.

### `define-macro` — named fexpr

```
(define-macro (name param…) body)
```

Binds `name` to a fexpr. Equivalent to `define` for the function form but the
resulting callable receives its arguments unevaluated.

```
(define-macro (my-if c t e) (if (eval c) (eval t) (eval e)))
(my-if (< 1 2) "yes" "no")       ; -> "yes"
```

### `constant` — bind a read-only name

```
(constant 'sym value …)
```

_Evaluates:_ the symbol targets and the values (like `set`). Sets each symbol
and marks it **protected**: a later `set`/`setf`/destructive op on it raises an
error. Used, among other things, to protect FOOP objects from mutation through
`self`.

---

## Binding

### `let` — parallel local bindings

```
(let (sym expr sym expr …) body)
```

_Evaluates:_ each `expr` **in the outer scope** (the binding list is flat and
the initialisers are parallel — they do not see each other), then binds each
`sym` for the duration of `body`.

```
(let (a 3 b 4) (add (mul a a) (mul b b)))   ; -> 25
```

### `local` — locals initialised to nil

```
(local (sym …) body)
```

Binds each `sym` to `nil` for `body`. Useful for scratch variables.

---

## Assignment and places

See `syntax.md` §8 for the place / reference model these share.

### `set` — assign via an evaluated symbol

```
(set 'a 1 'b 2 …)
```

_Evaluates:_ every argument. Each odd argument must evaluate to a **symbol**;
the following value is assigned to it. Returns the last value. Errors on a
protected symbol.

### `setq` / `setf` — assign to a place

```
(setq place value …)     (setf place value …)
```

_Evaluates:_ the values (and any index sub-expressions of the places), not the
place heads. Assigns each value to its place, which may be a bare symbol or a
nested location. While evaluating a value, `$it` is bound to that place's
**current** value. `setq` and `setf` are aliases.

```
(set 'L '(1 2 3))
(setf (L 1) 99)          ; L -> (1 99 3)
(setf x 1) (setf x (+ $it 1))   ; -> 2
```

---

## Conditionals

### `if` — multi-way conditional

```
(if c1 e1 [c2 e2 …] [else])
```

_Evaluates:_ conditions left to right, and only the first branch whose condition
is truthy. A trailing odd operand is the `else`. Returns the chosen branch's
value, or `nil`.

```
(if (< x 0) "neg" (> x 0) "pos" "zero")
```

### `cond` — clause list

```
(cond (test body) …)
```

_Evaluates:_ each clause's `test` until one is truthy, then that clause's `body`.
Returns the body's value, or `nil` if no clause matches. Each clause is a list.

### `when` / `unless` — one-armed guards

```
(when cond body)      (unless cond body)
```

`when` evaluates `body` when `cond` is truthy; `unless` when it is falsy.
Otherwise `nil`.

### `and` / `or` — short-circuit

```
(and expr …)      (or expr …)
```

`and` evaluates left to right; returns `nil` at the first falsy value, else the
**last** value (empty `and` is `true`). `or` returns the **first truthy** value,
else `nil`.

---

## Iteration

Each loop returns the value of its last body evaluation (or `nil`).

### `while`

```
(while cond body)
```

Repeats `body` while `cond` is truthy.

### `for` — numeric loop

```
(for (var from to [step]) body)
```

_Evaluates:_ `from`, `to`, `step`. Iterates `var` from `from` to `to`
**inclusive**. The step magnitude defaults to 1; the **direction is automatic**
(descending when `from > to`). If `from`, `to`, and `step` are all integers the
loop is integer-valued, otherwise float-valued.

```
(for (i 1 5) (print i))          ; 12345
(for (y 1 -1 0.5) (print y " ")) ; 1 0.5 0 -0.5 -1
```

### `dolist` — iterate a list

```
(dolist (var list [break]) body)
```

Binds `var` to each element of `list`. If a `break` expression is given and
evaluates truthy for the current element, the loop stops before running `body`.

### `dotimes` — count

```
(dotimes (var count) body)
```

Binds `var` to `0 … count-1`.

---

## Sequencing

### `begin`

```
(begin body)
```

Evaluates each expression in order and returns the last. Useful where one
expression is expected but several steps are needed.

---

## In-place mutation

These are special forms because they take a `place` (an unevaluated location),
not a value. They mutate the stored structure and, in place position, return a
reference so they compose (`syntax.md` §8).

### `++` / `inc`, `--` / `dec` — numeric change

```
(++ place [amount])   (inc place [amount])
(-- place [amount])   (dec place [amount])
```

Changes the number at `place` by `±amount` (default 1) and returns the new
value. An unset/`nil` place counts as 0.

### `push` / `pop`

```
(push value place [index])       (pop place [index])
```

`push` inserts `value` into the list at `place` (default index `0` = front, `-1`
= end) and returns the modified list; a `nil` place becomes a new list. `pop`
removes and returns an element (default the front); an empty list yields `nil`.

### `reverse` / `sort` / `rotate`

```
(reverse x)   (sort x)   (rotate x [n])
```

If `x` is a place, these mutate it **in place** and return a reference; if `x`
is a computed value (e.g. the copy from `append`), they operate on that copy.
`rotate` moves the tail to the front by `n` (default 1). `sort` orders numbers
numerically, strings and symbols by name, lists lexicographically.

### `replace`

```
(replace target place new)
```

_Evaluates:_ `target` and `new`. In the list at `place`, replaces every element
equal to `target` with `new`; in a string place, replaces occurrences of the
substring. Returns the modified value.

### `set-ref` / `set-ref-all`

```
(set-ref key place new)          (set-ref-all key place new)
```

Deep-replaces the **first** (`set-ref`) or **every** (`set-ref-all`) occurrence
of `key`, anywhere within the nested list at `place`, with `new`.

---

## Objects

### `self` — the current object

```
(self [index …])
```

Valid inside a FOOP method. `(self)` is the whole target object; `(self i …)`
indexes into it. `self` is a **place**, so `(inc (self 1) …)` and
`(setf (self 2) …)` write back into the stored object (`syntax.md` §7).

---

## Control

### `catch` — catch throws and errors

```
(catch expr)          (catch expr 'result-sym)
```

_Evaluates:_ `expr` (and, in the two-argument form, the result symbol). Both a
`throw` and a runtime error unwind to the nearest `catch`.

- One argument: returns `expr`'s value, or the caught thrown value / error
  string.
- Two arguments: binds `result-sym` to the result (normal) or the thrown value /
  error (on exit), and returns `true` on normal completion, `nil` on a caught
  exit.

```
(catch (throw 'boom) 'r)         ; -> nil, r is 'boom
(catch (/ 1 0) 'e)               ; -> nil, e is the error string
```

### `throw` — non-local exit

```
(throw value)
```

_Evaluates:_ `value`, then unwinds to the nearest enclosing `catch`, which
yields `value`.

---

## Timing

### `time`

```
(time expr)
```

Evaluates `expr` and returns the wall-clock milliseconds it took (the value of
`expr` is discarded).

---

## Notes

- **Place-position-only forms.** In addition to the forms above, `case`,
  `if-not`, `assoc`, and `lookup` are recognised when they appear as a `place`
  argument to a destructive form (so a reference flows out of them —
  `(pop (case 1 (1 L)))`). They are not yet general evaluatable special forms in
  value position; this asymmetry is an implementation gap, not a design choice.
- **Reference-returning.** Most conditionals and sequencing forms (`if`, `when`,
  `unless`, `begin`, `and`, `or`, `cond`) pass a reference out when used in place
  position, which is what lets destructive forms reach through them
  (`syntax.md` §8).
- **Unwinding.** `throw`/error unwinding, dynamic-scope restoration, and value
  reclamation share one path, so bindings from `let`/`local`/parameters are
  correctly restored even when a `catch` is triggered mid-body.
