# Compatibility and Deviations from Traditional Lisp

niiLISP implements the **newLISP dialect**, which departs from the traditional
Lisp family (Common Lisp, Scheme, Clojure, Emacs Lisp) in specific, deliberate
ways. This chapter lists those departures precisely so they are not surprises. It
is the spec-level companion to the discursive essay
[`docs/notes/20260704_newlisp-peculiarities.md`](../notes/20260704_newlisp-peculiarities.md);
the *why* behind each choice is in the [ADRs](../adr/).

## The unifying axiom: ORO

Most deviations follow from one principle — **One Reference Only** (ORO): values
are never shared. They are deep-copied when stored or passed, so there is no
aliasing and no cyclic structure. See [`types.md`](types.md) for the value
semantics. Where a departure below is a consequence of ORO, it is marked _(ORO)_.

## Deviations

### No cons cells or dotted pairs

Traditional Lisp is built on the cons cell `(car . cdr)`, with dotted pairs and
improper lists. niiLISP has **no dotted pairs**: a list is an array-backed value,
and `(cons 1 2)` yields the two-element list `(1 2)`, not `(1 . 2)` — a
non-list second argument is treated like `list`. Association lists still work
(they are lists of lists), but anything relying on `(a . b)` cells does not.
See [`types.md`](types.md) § list.

### No object identity / no `eq` _(ORO)_

Because every value is copied and nothing is shared, there is no persistent
identity to compare. Traditional Lisp offers an identity ladder (`eq`/`eql`/
`equal`/`equalp`; `eq?`/`eqv?`/`equal?`). niiLISP offers only **structural
equality**, `=`. There is no `eq`. (Symbols and contexts, being globally unique,
still have identity, but data values do not.)

### Dynamic scope only; no lexical closures

Common Lisp defaults to lexical scope (with dynamically-scoped special
variables); Scheme and Clojure are lexical only; Emacs Lisp was historically
dynamic. niiLISP is **dynamically scoped and has no lexical closures**: a callee
sees the caller's current bindings, and a function does not capture its defining
environment. Encapsulation is provided by contexts instead. See
[`syntax.md`](syntax.md) §4.

### Runtime fexprs instead of hygienic macros

Traditional Lisp uses macros — Common Lisp's expansion-time `defmacro`, Scheme's
hygienic `syntax-rules`. niiLISP's "macros" (`lambda-macro` / `define-macro`) are
**runtime fexprs**: they receive their arguments **unevaluated** and decide what
to evaluate, with no hygiene and no separate expansion phase. This is consistent
with the interpreter being a direct tree-walker (there is no compile phase to
expand into). See [`special-forms.md`](special-forms.md).

### No tail-call optimisation; no continuations

Scheme mandates proper tail calls; niiLISP has **no TCO** — recursion uses the
host stack and is bounded by a maximum call depth, so deep non-tail recursion
fails rather than looping forever. There are **no first-class continuations**
(no `call/cc`); the only non-local control is `catch`/`throw`. See
[`syntax.md`](syntax.md) §9.

### Minimal numeric tower

Common Lisp and Scheme have a full numeric tower (rationals, bignums, complex);
Clojure has ratios and bignums. niiLISP has **64-bit integers and IEEE-754
doubles only**, and integer arithmetic **wraps** on overflow rather than
promoting. A bigint literal syntax (`123L`) is reserved but _(planned)_. There
are no ratios or complex numbers. See [`syntax.md`](syntax.md) §1.2.

### Strings are byte buffers, not character sequences

Traditional Lisp strings are sequences of characters. A niiLISP string is a
**binary-safe byte buffer** — `length` counts bytes, and it may hold arbitrary
or invalid-UTF-8 data (it doubles as the I/O and FFI buffer). Character-oriented
operations (`utf8len`, char indexing) are _(planned)_. See [`types.md`](types.md)
§ string.

### No quasiquote

There is no backquote / quasiquote-unquote. Code that builds forms does so with
ordinary list construction (`list`, `cons`, `append`) and `eval`, or the newLISP
`expand`-family facilities _(planned)_.

### The nil / () / true model

In Common Lisp, `nil` *is* the empty list and false. In Scheme, `#f` is false and
`'()` is truthy and distinct from it. In Clojure, `nil` and `false` are falsy.
niiLISP: **`nil` and the empty list `()` are both false** (and distinct values),
`true` is the canonical true, and **every other value is true**. See
[`types.md`](types.md).

### Simple error model, no condition system

Common Lisp has a rich, restartable condition system. niiLISP has only
**`catch`/`throw`** plus runtime errors that unwind to the nearest `catch` — no
restarts, handlers, or condition hierarchy. See [`syntax.md`](syntax.md) §9.

### Overloaded contexts

Where traditional Lisps separate namespaces (packages / modules / namespaces)
from objects and hash tables, a niiLISP **context** is one mechanism that serves
as namespace, FOOP class, dictionary, and module at once. See
[`syntax.md`](syntax.md) §7.

## At a glance

| Feature | Common Lisp | Scheme | Clojure | niiLISP |
| --- | --- | --- | --- | --- |
| Dotted pairs / true cons | yes | yes | (seq abstraction) | **no** |
| Object identity (`eq`) | yes | yes | yes | **no** (structural `=` only) |
| Scope | lexical (default) | lexical | lexical | **dynamic only** |
| Closures | yes | yes | yes | **no** (contexts instead) |
| Macros | `defmacro` | hygienic | macros | **runtime fexprs** |
| Tail calls | impl-defined | **required** | `recur` | **none** |
| Continuations | — | `call/cc` | — | none |
| Numeric tower | full | full | ratios + bignum | **int64 + double** (bigint _(planned)_) |
| Strings | characters | characters | characters | **byte buffers** |
| Quasiquote | yes | yes | yes | **no** |
| Falsy values | `nil` | `#f` | `nil`, `false` | `nil` and `()` |
| Error model | conditions + restarts | exceptions | `ex-info` | **`catch`/`throw`** |

## Summary

niiLISP keeps Lisp's *surface* — s-expressions, `quote`, first-class lists,
symbols — while replacing several of the semantic pillars that define
traditional Lisp (cons/`eq`, lexical scope and closures, hygienic macros, the
numeric tower). It is best understood not as a Lisp variant with omissions but as
a distinct language that borrows the s-expression notation, organised around ORO.
