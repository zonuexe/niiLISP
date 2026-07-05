# Lambdas as lists: a hybrid compact/list representation, plus expand & args

Makes niiLISP run newLISP programs that treat a lambda as a **modifiable list** —
the target being the lambda-calculus gist
(<https://gist.github.com/kosh04/262332>), whose core is
`(define-macro (LAMBDA) (append (lambda) (expand (args))))`: it *builds* lambdas
with `append` and expands upper-case symbols to fake static binding. Requires
three things niiLISP lacks: a list interface on lambdas, `expand`, and `args`.
Design was grilled before writing.

## newLISP's model vs ours

In newLISP a lambda **is a list** — `(lambda (x) (+ x 1))` is literally the list
with head symbol `lambda`, self-evaluating and callable, so `append`/`first`/
`length` work on it and `(list? f)` is true. niiLISP compiled lambdas to a
distinct `Value::Lambda(Rc<Lambda>)` (and fexprs to `Value::Fexpr`), which is
compact and fast to call but is *not* a list, so `(append (lambda) …)` fails.

## Decision: hybrid — compact internal form, list interface on demand

Rather than drop `Value::Lambda`/`Fexpr` and represent every function as a list
(faithful but a parse-per-call regression and a wide blast radius through
call/apply/define/FOOP), niiLISP keeps them as the **compact internal form** and
**presents a list view when a list operation is applied**:

- `(lambda …)` / `(fn …)` / `(lambda-macro …)` still produce `Value::Lambda` /
  `Fexpr` — the hot path (define + call, FOOP methods) is unchanged, so the
  `(tak …)` benchmark and existing tests are unaffected.
- A single helper reconstructs the list form `(lambda (params…) body…)` /
  `(lambda-macro (params…) body…)` from a `Lambda`/`Fexpr`. **Every list builtin
  routes through this helper**, so a lambda uniformly behaves as its list form
  for `append`, `first`, `rest`, `last`, `nth`, `cons`, `length`, `map`,
  `filter`, `explode`, … — the result of a list operation is a plain `List`.
- A **list whose head is the symbol `lambda`/`fn`/`lambda-macro` is callable**:
  in operator position it is invoked as a lambda/fexpr (params from the second
  element, body from the rest, parsed on call). So a lambda built by `append`
  runs.
- Printing shows a lambda as its list form, for consistency and re-readability.

### Trade-off recorded

The cost of the hybrid is **two representations of one value**, kept consistent
only through the central helper. The risk is coverage: a list operation that
does *not* go through the helper would not see a lambda as a list. This is
contained by funnelling all list access through the one helper (not a per-call
special case), so the coverage is structural, not ad hoc. Full list *identity*
(e.g. `(= (lambda(x)x) '(lambda(x)x))`, or `setf` into a lambda's body in place)
is **not** guaranteed by this slice — see Deferred. The alternative (pure
list-ification) was rejected: it regresses call speed and churns the core for
faithfulness this target does not need.

## `expand`

`(expand expr sym-1 …)` replaces each occurrence of the given symbols in `expr`
with their current values, recursively through nested lists. `(expand expr)`
with no symbols replaces every symbol whose **name starts with an ASCII
upper-case letter and whose value is non-`nil`** — using the current
(dynamically-bound) value, which is what lets the gist bake a bound parameter
(`N`, `M`, …) into a freshly-built lambda while leaving unbound ones (`F`, `X`)
as symbols. This upper-case auto-expansion is deliberately fragile (a bound
upper-case symbol is always substituted); that matches newLISP and the gist's
own caveat.

## `args`

`(args)` returns the list of arguments passed to the currently executing
lambda / fexpr that were **not bound to a declared parameter** (the var-args
tail); `(args i …)` indexes into it. For a fexpr the arguments are unevaluated.
The gist's `(define-macro (LAMBDA) …)` declares no parameters, so `(args)` is the
whole raw argument list. Implemented with a per-call stack in the interpreter,
pushed after parameter binding and popped as the call unwinds.

## Scope

- **In:** the list view of lambdas/fexprs via the central helper; callable
  lambda-headed lists; `expand` (explicit + upper-case auto); `args`. Target: the
  gist loads and runs, and its Church numerals evaluate correctly.
- **Deferred:** full list *identity* for lambdas — structural `=` between a
  lambda and its list form, and **in-place mutation** of a lambda's body via
  place navigation (`setf` into a lambda). The gist builds fresh lambdas with
  `append` and does not mutate them in place.

## Acceptance

- A hermetic test runs the gist's definitions and checks `to-number` of the
  Church numerals — `ZERO`→0, `ONE`→1, `TWO`→2, `(PLUS ONE TWO)`→3,
  `(MULT TWO THREE)`→6, `(POW TWO THREE)`→8 — and a small `expand`/`args` unit
  check. Deep `Y`-combinator recursion (`FACT`) is exercised only at a small
  input if it runs within the tree-walker's stack; it is commented out in the
  gist itself.

## Consequences

- `Value::Lambda`/`Fexpr` remain the stored/called form; the change is additive:
  a list-view helper, list-builtin routing through it, callable lambda-headed
  lists, `expand`, `args`, and lambda printing as a list.
- Predicates gain faithful answers where routed (`list?` etc. via the helper);
  where not routed, a lambda still reads as a function value — an accepted,
  documented gap until a target needs full identity.
- This is a compatibility milestone: niiLISP now runs newLISP's code-as-data
  lambda idiom, its deepest peculiarity.
