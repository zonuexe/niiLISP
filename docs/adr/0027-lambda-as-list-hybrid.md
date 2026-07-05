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

The gist runs to completion (its active top-level forms are `(to-number ZERO)`,
`… ONE`, `… TWO`, `(to-number (PLUS ONE TWO))`, results discarded), and a
hermetic test checks the Church numerals it defines: `ZERO`→0, `ONE`→1, `TWO`→2,
`THREE`→3, `(PLUS ONE TWO)`→3, `(MULT TWO THREE)`→6, plus `expand`/`args` and
special-form-aliasing unit checks.

`POW`/`SUCC` and the commented-out boolean/pair/`Y` sections hit the gist's own
documented **reused-variable hazard**: they bind a parameter (e.g. `X`) to a
*code-like* value and then rebuild an inner lambda under that binding, so `expand`
substitutes it into a parameter position. The gist author flags this ("if
variables duplicate, expansion goes wrong … the lambda's formal parameters get
expanded"); these are not among the gist's asserted results. Making them work
would require lexical binding, which newLISP (and niiLISP) do not have.

## Consequences

- `Value::Lambda`/`Fexpr` remain the stored/called form; the change is additive:
  a list-view helper, list-builtin routing through it, callable lambda-headed
  lists, `expand`, `args`, and lambda printing as a list.
- Predicates gain faithful answers where routed (`list?` etc. via the helper);
  where not routed, a lambda still reads as a function value — an accepted,
  documented gap until a target needs full identity.
- This is a compatibility milestone: niiLISP now runs newLISP's code-as-data
  lambda idiom, its deepest peculiarity.

## Outcome and refinements from implementation

- **Empty `(lambda)` self-quotes to the list `(lambda)`**; any lambda with a
  parameter list stays the compact `Value::Lambda`/`Fexpr`. That is the minimum
  that makes `(append (lambda) …)` build a callable lambda, and it kept the
  change small — the gist appends only to the empty lambda.
- **Special forms are first-class enough to alias.** The gist does
  `(define DEFINE define)` / `(define IF if)`, but niiLISP dispatches special
  forms by name, so a special-form symbol used as a value was `nil`. Now a
  special-form name evaluates to itself when otherwise unbound, and an operator
  that evaluates to a special-form symbol dispatches that form — so the aliases
  work.
- **`expand` treats nested lambdas as opaque** (it does not descend into a
  `(lambda …)` sublist), so a nested Church numeral's reused `F`/`X` parameters
  are not rewritten.
- **`(expand expr)` auto-expands only code-like values** (lists / functions), not
  self-evaluating atoms. Substituting a bound loop variable's *number* into a
  parameter position would break the built lambda; restricting auto-expansion to
  code is a deliberate, documented deviation from newLISP's "any value" rule that
  makes `PLUS`/`MULT` work. `(expand expr sym…)` (explicit) still substitutes any
  value.
