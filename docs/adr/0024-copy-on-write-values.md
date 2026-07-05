# Copy-on-write for List / Array / Str (implements ADR-0016)

Promotes the leading candidate from [ADR-0016](0016-value-representation-and-copy-strategy.md)
(deferred analysis) into a decision: wrap the deep-copied data variants in `Rc`
and mutate through `Rc::make_mut`, turning ORO's O(n) copy-on-store into O(1)
sharing until write. ADR-0016 keeps the full option analysis (why not NaN-boxing,
etc.); this ADR records the implementation decision and was grilled before
writing.

## Motivation

Under ORO (CONTEXT.md: ORO) every list/array/string is deep-copied when stored
or passed (ADR-0005). Because a variable read (`lookup`) clones, reading a large
container is O(n), so a loop that reads a large container per iteration is O(n²).
This is exactly what gates the array-based sieve in `qa-factorfibo`
([ADR-0023](0023-array-value-type.md)): a 1,000,000-element array read per
iteration. Copy-on-write removes the asymptotic cost while preserving the
observable value semantics the rest of the interpreter relies on.

## Scope

- **`List(Rc<Vec<Value>>)`, `Array(Rc<Vec<Value>>)`, `Str(Rc<Vec<u8>>)`.** These
  are the variants that deep-copy in O(n).
- **Out of scope:** immediates (`nil`/`true`/`Int`/`Float`/`Symbol`/`Context`)
  copy cheaply already; `Lambda`/`Fexpr`/`Foreign` are already `Rc`; `Bigint`
  stays owned (rare, and `num-bigint` is heap-backed internally — it can be
  wrapped later if it ever shows up in a hot copy path).
- **`Rc`, not `Arc`.** The interpreter is single-threaded; concurrency is
  OS processes (ADR-0014), matching the existing `Rc<Lambda>` (ADR-0004).

## Mechanism

The invariant that makes this safe and behaviour-preserving: **reads are
transparent, every write goes through `Rc::make_mut`.**

- **Reads** — pattern binding `Value::List(l)` yields `&Rc<Vec<Value>>`; `l.len()`,
  `l.iter()`, `l[i]`, `l.first()` work unchanged through `Deref`. Most read code
  compiles as-is.
- **Writes** — anywhere a `&mut Vec` is needed (`place_navigate`, and the
  in-place ops `push`/`pop`/`sort`/`reverse`/`rotate`/`replace`/`extend`/`swap`,
  the `setf`/`++`/`--` element writes), obtain it via `Rc::make_mut(rc)`, which
  mutates in place when uniquely owned (refcount 1) and **clones the contents
  when shared** (refcount > 1) — so a mutation to one owner never reaches
  another. `place_navigate` calls `make_mut` at each level as it descends a path.
- **Construction** — `Value::List(vec)` becomes `Rc`-wrapped. Add helper
  constructors `Value::list(vec)` / `Value::array(vec)` / `Value::str(bytes)` to
  centralise the `Rc::new` and keep call sites readable.

## Correctness

The single correctness condition — all mutation goes through `make_mut` — is
**enforced by the type system**: a `&mut Vec<_>` cannot be obtained from a
`&Rc<Vec<_>>` without `Rc::make_mut` (or `get_mut`), so a bypass does not
compile. On top of that:

- **Aliasing-isolation tests**: after a share, mutating one owner leaves the
  other unchanged — `(set 'a '(1 2 3)) (set 'b a) (push 9 a) (= b '(1 2 3))`,
  a nested `setf` variant, and FOOP `self` write-back.
- **The existing suite is the regression guard**: the reference/place model,
  FOOP, and `catch`/`throw` already exercise ORO semantics heavily, so their
  continued passing is strong evidence the observable behaviour is unchanged.

No new dependency (no property-testing framework); hand-written isolation tests
suffice.

## Rollout and acceptance

- **A single compiler-driven refactor.** Changing the enum breaks every
  construction and mutation site at once, so this lands as one change
  (value/printer/reader/builtins/eval/ffi), guided by the compiler errors. It
  cannot be staged — intermediate states do not build.
- **Acceptance:**
  1. the full suite (unit + integration) and the new isolation tests pass —
     behaviour is unchanged;
  2. the clone-on-read O(n²) is gone, **measured** on the sieve (`primes` over a
     large N drops sharply); if `qa-factorfibo`'s million-element sieve then runs
     fast enough in the test harness, wire it into `tests/qa.rs`, else keep it
     release-verified and record why;
  3. **non-regression** on the eval benchmark `(tak 24 16 8)` (~0.80s baseline,
     ADR-0017) — small-int recursion touches no large container, so CoW should be
     neutral; confirm it is not worse.

## Consequences

- The `Value` layout gains three `Rc` variants; `clone()` (ORO store/pass) becomes
  a refcount bump, and the deep copy happens lazily at the first write to a shared
  value.
- Mutation code is slightly more intricate (a `make_mut` at each write / path
  step), but reads are unchanged.
- Orthogonal ADR-0016 items remain available and additive later: safe slot shrink,
  SSO, and — only under an explicit tiny-footprint goal — the uniform-cell
  allocator. The dispatch cache (ADR-0007 / ADR-0017) is a separate, eval-speed
  lever untouched here.
