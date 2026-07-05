# Dictionary API: contexts as hashes (nil default functor)

The second slice from the gap analysis, following file I/O (ADR-0029): newLISP's
dictionary — a context used as a string/number-keyed hash. Acceptance target: the
`qa-dictionary` oracle. Design was grilled before writing, grounded in the newLISP
source (`newlisp.c`, `nl-symbol.c`).

## The FOOP / dictionary collision, and how newLISP avoids it

Applying a context is overloaded: `(Complex 3 4)` constructs a FOOP object, while
`(Lex "key")` looks up a dictionary entry. niiLISP had **implicit construction** —
applying a context with no lambda default functor built a tagged list `(Ctx args…)`
(a shortcut that let `qa-foop` pass without defining a constructor). That shortcut
collides with dictionary access, which is *also* "apply a context with no lambda
functor".

newLISP has no collision because it distinguishes on the **default functor
`Ctx:Ctx`** (`newlisp.c:1566-1577`):

- **lambda** → call it (FOOP constructor);
- **nil** → `evaluateNamespaceHash` (dictionary access).

FOOP always rides a lambda functor because newLISP **predefines**, in its built-in
init string (`newlisp.c:138`):

```lisp
(define (Class:Class) (cons (context) (args)))
```

So `(new Class 'A)` copies that constructor to `A:A`, and `(A 0 (B 0))` calls it to
build `(A 0 (B 0))`. A bare `(context 'Lex)` leaves `Lex:Lex` nil, so `Lex` is a
dictionary.

## Decision: mirror newLISP exactly

- **Chosen:** predefine `Class:Class` as newLISP does, and rewrite context
  application so a **nil** default functor dispatches to dictionary access,
  **removing the implicit-construction fallback**. FOOP construction then always
  goes through a lambda functor (the predefined `Class`, or a user constructor),
  and a nil-functor context is a hash — the two paths never overlap. The existing
  `foop_construct_dispatch_and_self_writeback` unit test keeps passing: with
  `Class` predefined, `(new Class 'A)` / `(A 0 (B 0))` builds the tagged list via
  the lambda, not the removed fallback.
  - Sub-requirements: `(context)` with no argument returns the current context
    symbol (used by the predefined `Class`); `cons`/`args` already exist.
- **Rejected:** keep implicit construction and trigger dictionaries some other way
  (an explicit dict marker, or argument-type heuristics on a per-application
  basis). It diverges from newLISP and leaves `(Ctx)` (enumerate) vs `(Ctx)`
  (empty object) fundamentally ambiguous.

## Dictionary dispatch (`evaluateNamespaceHash`)

When `Ctx:Ctx` is nil, dispatch on the **first evaluated argument's type**
(`newlisp.c:1706`):

- **string / number key**: one arg → **get** (`Ctx:_key` value, or nil); two args
  → **set** (value nil **deletes** the key and returns nil, else stores and returns
  the value).
- **list of `(key value)` pairs**: **bulk-load** each pair, return the context. (The
  `qa-dictionary` benchmark relies on `(Lex ass)` populating the whole dictionary.)
- **no args** (evaluates to nil): return the **association list of all entries**.

## Keys are `_`-prefixed context symbols (`makeSafeSymbol`)

Grounded in `nl-symbol.c:133`: a key becomes a context symbol named **`_` +
key-as-string** (a number formatted as decimal). The leading `_` (a) avoids
collision with the default functor `Ctx:Ctx`, (b) makes numeric keys valid symbol
names, and (c) marks hash entries. A number key and the equal string key collapse
to the same symbol (`1` and `"1"` → `_1`) — a newLISP quirk kept as-is.

- A dictionary is therefore just **a context of `_`-prefixed symbols**, so
  `symbols`, `dotree`, and `save` interoperate with it for free.
- **Enumeration** (`(Ctx)`) returns pairs **sorted by symbol name**, keys with the
  leading `_` stripped and returned **as strings** (numeric keys included). The
  deterministic order is what makes saving the same dictionary twice byte-identical
  (the oracle's success check).

## `delete` and helpers

- `(delete 'Ctx)` removes the context and all its symbols; `(delete 'Ctx:sym)`
  removes one; returns true.
- `sys-info` — best-effort counts. The oracle only **prints** them and never
  asserts (its success is a `read-file` byte comparison), so exact cell counts are
  not a compatibility surface; the values are documented as approximate.
- `randomize` — a Fisher–Yates shuffle over the existing RNG, returning a new list
  (ORO).

## Slice boundary

- **This slice (dictionary core):** predefined `Class`, the nil-functor→hash
  rewrite, get/set/bulk/enumerate/delete, key mangling, `(context)` with no arg,
  plus `sys-info` and `randomize`. Verified by a unit test covering the
  `qa-dictionary` logic (populate → read-verify → delete).
- **Deferred to file I/O slice 2:** `save` / `load` / `source`. The dictionary's
  needs **drive** that slice's format — a deterministic, sorted, round-trippable
  source serialisation — and `qa-dictionary` is wired only once both this slice and
  persistence land.
