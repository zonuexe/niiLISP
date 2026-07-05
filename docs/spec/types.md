# niiLISP Value Types

A per-type reference for niiLISP's values, companion to [`syntax.md`](syntax.md)
(overview in §2), [`special-forms.md`](special-forms.md), and
[`functions.md`](functions.md). Sections marked _(planned)_ describe the dialect
but are not yet implemented.

## Value semantics (ORO)

All values follow newLISP's **One Reference Only** model: they are **deep-copied**
when stored in a structure or passed to a function, and are **never shared**.
Consequences that shape every type below:

- There is **no object identity** — no `eq`. Two values are compared only by
  **structure** with `=`.
- **No cyclic references** can arise, and there is no garbage collector.
- Lists and strings are ordinary values; mutation happens through the
  place/reference model (`syntax.md` §8), not through shared references.

**Truthiness.** Every value is true except `nil` and the empty list `()`.

---

## nil

The empty / false value. An **unset symbol evaluates to `nil`**, and many
operations return `nil` to signal "nothing" (e.g. out-of-range indexing, a
failed `import`, an unmatched `case`).

- Literal: `nil`.
- `nil` and the empty list `()` are both falsy; they are distinct values but
  interchangeable in boolean position.
- Predicate: `(nil? x)` / `(null? x)`.

## true

The canonical true value, returned by predicates and comparisons. Any non-`nil`,
non-`()` value is also true in boolean position.

- Literal: `true`.

## integer

A 64-bit signed integer. Integer arithmetic (`+ - * / %`) **wraps** on overflow
rather than trapping.

- Literals: decimal `42`, `-7`; hexadecimal `0xFF`.
- A trailing `L` (`123L`) denotes a bigint literal — _(planned)_, currently a
  read error.
- Pointers and C handles from FFI are represented as integers.
- Predicate: `(integer? x)`, also `(number? x)`.

## float

An IEEE-754 double. Float arithmetic uses `add sub mul div` (and the math
functions); integer `/` on a NaN operand treats it as 0, and `inf` saturates to
the max integer when converted.

- Literals: `3.14`, `-0.5`, `1e-9`.
- `inf` / `NaN` arise from operations (e.g. `(div 1.0 0)`, `(sqrt -1)`); a
  comparison involving `NaN` is false, and `NaN` is not equal to itself.
- Predicates: `(float? x)`, `(number? x)`, `(NaN? x)`, `(inf? x)`.

## string

A **binary-safe byte buffer** — an arbitrary byte sequence, not guaranteed valid
UTF-8. Strings double as the byte buffer for I/O and FFI.

- Three literal syntaxes (`syntax.md` §1.3): `"…"` (with escapes, including the
  decimal byte escape `\ddd`), `{…}` (raw, nestable, multi-line), and
  `[text]…[/text]` (raw block).
- `(length s)` counts **bytes**. Character-oriented operations (`utf8len`, char
  indexing/slicing) are _(planned)_.
- Predicate: `(string? x)`.

## symbol

An interned name. A symbol **evaluates to its current value** (or `nil` if
unset); use `quote` to obtain the symbol itself. Symbols may contain `.`; the
colon `:` qualifies a symbol by context (`Ctx:name`), and a leading colon
(`:name`) is a FOOP dispatch form.

- Literal: any non-delimiter token that is not a number; e.g. `foo`, `a.re`,
  `MAIN:x`. As data: `'foo`.
- Predicate: `(symbol? x)`.

## list

An ordered sequence, backed by a growable array. Lists are the core structured
value and also the substrate for FOOP objects (a list whose head is a class
context's symbol).

- Literal (as data): `'(a b c)`; built with `(list …)` / `(cons …)`.
- **No dotted pairs.** `(cons x lst)` prepends `x` to the list `lst`; if the
  second argument is not a list, `(cons a b)` yields the two-element list
  `(a b)` — so `(cons 1 2)` is `(1 2)`, never `(1 . 2)`. There are no `car`/`cdr`
  cells (see [`compatibility.md`](compatibility.md)). This is the canonical home
  for list construction semantics; `functions.md` gives the bare signatures.
- Indexing is implicit when a list or integer is in operator position
  (`syntax.md` §5); negative indices count from the end.
- Mutated through place forms (`push`, `pop`, `setf`, `sort`, … — see
  `special-forms.md`).
- Predicate: `(list? x)`; `(atom? x)` is true for non-lists.

## context

A **namespace** and, in object use, a **FOOP class** (`syntax.md` §7). A context
is a first-class value that owns its symbols.

- Created by `(new prototype 'Name)`, or implicitly by defining a qualified
  symbol `Name:member`.
- **Applying a context** `(Ctx arg…)` runs its default functor `Ctx:Ctx` if
  defined, otherwise constructs a tagged object list.
- A context and the symbol of the same name compare equal, so FOOP objects match
  quoted symbol-headed literals.

## functions

Four callable kinds, all applicable as `(f arg…)`:

| Kind | Created by | Arguments |
| --- | --- | --- |
| lambda | `lambda` / `fn` / `define` | evaluated before the call |
| fexpr | `lambda-macro` / `define-macro` | passed **unevaluated** |
| builtin | provided by the interpreter | evaluated (`functions.md`) |
| foreign | `import` (FFI) | evaluated, marshalled to C (`syntax.md` §10) |

Parameters may have defaults: `(param default)`. There are no lexical closures
(scoping is dynamic — `syntax.md` §4). A `callback` turns a niiLISP function into
a C function pointer, represented as an integer address.

---

## Equality and ordering

- `(= a b …)` is **structural** equality across all types (numbers compare across
  integer/float; a context equals the symbol of its name).
- `(!= …)`, `(< …)`, `(> …)`, `(<= …)`, `(>= …)` order numbers, and strings by
  bytes; a comparison involving `NaN` yields `nil`.
- There is no identity comparison, by design (ORO — no sharing to have identity
  over).
