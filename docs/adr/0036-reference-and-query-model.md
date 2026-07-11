# Reference and query model: `ref`/`ref-all`/`match`/`find-all`

niiLISP already has newLISP's **place** model for destructive access — a `Place`
is a root symbol plus a flat integer `path`, walked by `place_navigate`, and
`set-ref`/`set-ref-all` deep-replace by value ([ADR-0024](0024-copy-on-write-values.md),
the reference/place slices). What is still missing is the **query** half the
WikiBook leans on: locating elements in a nested list and matching against a
pattern. This ADR adds `ref`, `ref-all`, `match`, and `find-all`, and records
what is deliberately left out.

The overriding goal (per `CONTEXT.md`) is compatibility with existing newLISP
assets, so where a decision is under-determined by taste we follow the 10.7.x
manual's observable behaviour.

## Scope: `ref`/`ref-all`/`match`/`find-all` now, `unify` deferred

- **Chosen:** implement the four query functions the *Lists*, *Strings*, and
  *XML* chapters actually use. They share one substrate — a nested-list walk that
  yields index paths, a pattern matcher, and an optional comparison function — so
  they land as one coherent slice.
- **Deferred:** `unify` (Prolog-style unification with upper-case-initial logic
  variables and an environment) is a distinct sub-language, not a list query; no
  vendored target or WikiBook example in the coverage audit uses it. It can be a
  later, self-contained slice. `$count`-consuming statistics beyond the count
  itself are out of scope.

## Index vectors are flat integer lists, reusing the place path

- **Chosen:** `ref` returns the index path to the first match as a flat list of
  integers (`(ref 'c '(a (b c)))` → `(1 1)`), and `ref-all` returns a list of
  such paths. This is exactly the `Place.path` representation
  (`Vec<i64>`), so a returned path round-trips straight back into the place
  machinery. An empty list means "not found" (newLISP returns `()`), which is
  falsy, so `(if (ref …) …)` works.
- **Rejected:** a bespoke "reference" value type. newLISP itself returns a plain
  list of integers; a new type would be less compatible and would not compose
  with `nth`/`push`/`pop`.

## `push`/`pop` gain an index-vector form, so `(pop L (ref k L))` works

The manual pairs `ref` with `push`/`pop`, "both of which can also take lists of
indices." Today niiLISP's `push`/`pop` take a **single** trailing index only.

- **Chosen:** accept a **list** in the index position as a full path
  (`(pop L '(1 0))` removes `L`'s element at path `1→0`), in addition to the
  existing one-or-more scalar indices. A single list argument is unambiguous
  (an index is always an integer), so the two forms cannot collide.
- **Rejected:** requiring the caller to `apply` the path (`(apply pop L (ref …))`).
  It works for `pop` but reads badly and diverges from the documented pairing.

## `match`: a backtracking matcher over list structure

`(match pattern list)` matches with three wildcards — `?` (exactly one element),
`+` (one or more), `*` (zero or more) — nestable, returning the list of matched
expressions (one element per `?`, a sublist per `+`/`*`), or `nil` on no match.

- **Chosen:** a recursive **backtracking** matcher. `+`/`*` are greedy but
  backtrack: they give back elements until the rest of the pattern can match, so
  `(match '(* 3 *) '(1 2 3 4))` binds the `*`s correctly around the pivot. The
  wildcards are read as ordinary symbols (`?`/`+`/`*` already tokenize as
  symbols), matched by name. A leading/trailing/adjacent-wildcard pattern is
  supported because backtracking handles the ambiguity that a single-pass greedy
  scan cannot.
- **Rejected:** a single greedy left-to-right pass. Simpler, but it fails the
  book's `(match '(* x *) …)` "find x anywhere" idiom whenever the pivot also
  appears inside a `*` region. Correctness beats the small speed win on the short
  patterns these are used with.

## `find-all`: three forms dispatched by argument type

`find-all` has three documented shapes: `(find-all regex-str text [exp [opt]])`,
`(find-all match-pattern list [exp])`, and `(find-all key list [exp [compare]])`.

- **Chosen:** dispatch on the runtime types of the first two arguments — a string
  pattern over a string is the **regex** form (reusing the ADR-0028 engine and
  the new capture-variable support); a **list** second argument selects the
  list forms, with a *list* pattern routed through `match` and any other key
  compared by value (or by `func-compare`). The optional `exp` is evaluated once
  per hit with `$it`/`$0..$N` bound, transforming each result (newLISP's "functor"
  argument), so `(find-all "..." text (upper-case $0))` works.
- **Rejected:** three separately-named builtins. newLISP exposes one name; a
  split would break verbatim scripts. The type dispatch is unambiguous for every
  documented call shape (string+string vs anything+list).

## System variables via the existing dynamic-scope guard

- **Chosen:** `ref-all` and `find-all` bind **`$count`** (the number of matches)
  and `find-all` binds **`$it`** (the current match) using the same `Scope`
  save/restore guard that already powers `$idx` and `setf`'s `$it` — bound for the
  duration of the call/`exp` evaluation and restored (to the prior value, or
  unbound) afterwards. `set-ref`/`set-ref-all` gain the same `$it`/`$count`
  binding for their replacement expression, matching the manual.
- **Rejected:** interpreter-global mutable slots. The dynamic-scope guard already
  exists, nests correctly, and is what newLISP's own scoping implies.

## Comparison function protocol

- **Chosen:** the optional `func-compare` is any callable, invoked as
  `(compare key element)` via `Interp::call`; a truthy result counts as a match.
  The default is the existing `values_equal`. This mirrors `sort`/`find`'s
  comparator convention already in the codebase.

## Consequences

- Unblocks the *Lists* chapter's remaining `ref`/`ref-all`/`match`/`find-all`
  gaps and the `match`-based idioms in the *XML* chapter's tree queries, plus
  `(filter (curry match '(a *)) …)` from *Apply and map*.
- `pop-assoc` (destructive assoc removal) is a small follow-on that reuses the
  same place machinery and `$it`; it is grouped with this family but can land
  in the same or the next slice.
- `unify` remains open; if a target needs it, it is a bounded addition that does
  not disturb this model (different value class — a binding environment).
