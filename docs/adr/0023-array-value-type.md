# array: a fixed-length, list-like value type

Adds `Value::Array`, newLISP's fixed-length counterpart to the list. Acceptance
target: the array-based `primes` sieve in the vendored `qa-factorfibo`. Design
was grilled before writing.

## Why a distinct type, not an alias

newLISP arrays are observationally almost identical to lists — they index,
`setf`-assign elements, report `length`, and **print exactly like a list**. The
only differences are the predicates (`array?` true, `list?` nil) and that they
are **fixed-length** (`push`/`pop` do not apply). Aliasing arrays to
`Value::List` would pass `qa-factorfibo` for free but silently break any script
that distinguishes the two by predicate — and the project's overriding goal is
compatibility with existing newLISP assets (CONTEXT.md). So, as with `bigint`
(ADR-0022), we add a real variant rather than an alias:

- **`Value::Array(Vec<Value>)`** — a distinct variant, always present (no Cargo
  feature: unlike bigint it pulls in no dependency). It **reuses the list
  machinery**: the backing `Vec<Value>` indexes, navigates as a place, and
  clones (ORO deep-copy) exactly as a list does.

## Dimensionality

Only **1-D** arrays are built in this slice. The variant can hold nested arrays,
so N-dimensional arrays are representable later as arrays-of-arrays without a
redesign; only the multi-dimensional **constructor** is deferred (see below).

## Fixed-length semantics

Length never changes after construction:

- **`(setf (arr i) v)`** replaces element `i` in place — the core mutation, fully
  supported.
- **`push` / `pop` / `extend`** on an array are **errors** (they would resize).
- **`sort` / `reverse` / `rotate`** on an array are **not supported in this
  slice** (they error); convert with `array-list` first. They are length-
  preserving and could be added later.

## Operation breadth (minimal, with a TODO)

Arrays are accepted directly only where `qa-factorfibo` needs them:
**indexing `(arr i)`, `(setf (arr i) …)`, `length`, `nth`/`first`/`last`, and
`array-list`.** Higher-order and transforming operations (`map`, `filter`,
`slice`, `sort`, `find`, …) require an explicit `array-list` conversion first —
which is what the qa script does. Widening acceptance so every list operation
also takes an array (returning lists from transforms, as newLISP does) is a
**TODO** for when a test needs it.

Cross-cutting predicate/equality rules settled alongside:

- `(array? x)` is true only for an array; `(list? arr)` is **nil**.
- `(atom? arr)` is **nil** — an array is not an atom, like a list. (`is_atom`
  becomes "not a list *and* not an array".)
- An empty array is **falsy**, like the empty list.
- Equality: `Array == Array` compares element-wise; an `Array` never equals a
  `List` (different types), even with equal elements — convert with `array-list`
  to compare against a list.
- An array **prints like a list** (`to_repr`), and `type_name` is `"array"`.

## Constructor and conversion

- **`(array size [init])`** — `size` is a non-negative integer. With no `init`
  the array is nil-filled; with an `init` list it is **cycle-filled**
  (`result[i] = init[i mod len(init)]`, truncating a longer `init`, repeating a
  shorter one; an empty `init` nil-fills). Leading integer arguments are
  dimensions: **two or more is an error** ("multi-dimensional arrays not yet
  supported"), so `(array d1 d2 … init)` is rejected while `(array size init)`
  is accepted.
- **`(array-list arr)`** — a plain list copy of the array's elements; a
  non-array argument is an error.
- **`(array? x)`** — the type predicate.

## Consequences

- A second non-`bigint` `Value` variant, always present. Exhaustive `match`es on
  `Value` without a catch-all gain an `Array` arm (`printer::to_repr`,
  `type_name`, `is_atom`, `is_truthy`, `values_equal`, the index/place paths).
- Reusing the list's `Vec<Value>` keeps indexing, place navigation, and copying
  shared with lists; the array-specific behaviour is the fixed length, the two
  predicates, and the print/equality type distinction.
- Deferred, tracked for later: the multi-dimensional constructor; wide operation
  acceptance (arrays anywhere lists are accepted); and length-preserving
  destructive ops (`sort`/`reverse`/`rotate`) on arrays.

## Implementation notes

- Two fixes surfaced while making the `qa-factorfibo` sieve run and are folded
  into this slice: (1) implicit indexing with a **number** in functor position
  was returning the element instead of the rest/slice newLISP defines — `(2 lst)`
  is now the tail from offset 2 (element access remains `(lst i)`); (2) the
  indexed-place guard was cloning the whole container on every `setf` (O(n²) in a
  mutation loop) and now type-checks by borrow. `true?`, which the sieve's
  `filter` needs, was also added.
- `qa-factorfibo` **runs correctly** (verified on small inputs) but is **not
  wired as an automated test**: its `collect-primes` sieves to 1,000,000, and
  under the current ORO model a read of a large container variable deep-copies it
  (`lookup` clones, ADR-0005), making the sieve O(n²). Wiring it waits on the
  copy-strategy optimisation ([ADR-0016](0016-value-representation-and-copy-strategy.md));
  this is a performance gate, not a missing feature.
