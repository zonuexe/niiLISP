# bigint: arbitrary-precision integers as a numeric-tower slice

Adds `Value::Bigint` and the surrounding arithmetic so over-long integer
literals and `L`-suffixed literals become arbitrary-precision integers, as in
newLISP. This is the numeric-tower extension deferred by
[ADR-0012](0012-reader-and-numeric-model.md); it revises that ADR's account of
literals (see below). Design was grilled before writing.

## Scope

- **In:** the `bigint` value, reader promotion of over-long / `L` literals, the
  arithmetic promotion lattice for `+ - * / %`, cross-type comparison and
  `zero?`/`abs`, `length` (digit count), the conversions `bigint`/`int`/`float`,
  decimal printing, the predicates, and `gcd`.
- **Deferred:** everything qa-bigint/qa-longnum *also* need but that is
  unrelated to bigint — the RNG family (`random`/`seed`/`rand`/`amb`) and
  `explode`/`chop`/`extend`/`until`/`primes`/`main-args`. Full qa-bigint /
  qa-longnum acceptance stays gated on those.

## Substrate and build

- **`num-bigint`** provides the integer (`num-traits` for `to_f64`/`to_i64`).
  Bignum division, remainder, and `to_f64` are error-prone to hand-roll;
  correctness-first (ADR-0007) favours the battle-tested crate.
- Behind a **default-on `bigint` Cargo feature**, mirroring newLISP's own
  compile-time bigint switch (its scripts probe it via `(unless bigint …)`).
  `--no-default-features` drops the dependency for a truly zero-native-dep,
  pure build — the same discipline as the `ffi` feature (ADR-0018).
- Because `num_bigint::BigInt` does not exist without the dependency, the
  **`Value::Bigint` variant itself is `#[cfg(feature = "bigint")]`** (unlike
  `Value::Foreign`, whose type is always present). Exhaustive `match`es on
  `Value` gain a `cfg`-gated `Bigint` arm; both build configurations stay
  exhaustive. The dependency and `num-traits` are optional and gated on the
  feature.
- Release: `release.yml` builds with default features, so prebuilt binaries
  include bigint. `num-bigint`/`num-traits` are pure Rust and cross-compile
  cleanly on every target (unlike the system-libffi FFI slice).

## Value representation

`Value::Bigint(num_bigint::BigInt)` — a **bare owned** `BigInt`, deep-copied on
store/pass like `Str`/`List` (ORO, ADR-0005). `clone` is `O(digits)`, but
bigints are rare; per ADR-0007 (measure-then-optimise) the copy cost is
accepted. Sharing bigints behind `Rc` (they are immutable — arithmetic yields
new values; `++`/`--` reassign the place) is a later optimisation, folded into
the `Rc`/copy-on-write value-representation work of
[ADR-0016](0016-value-representation-and-copy-strategy.md) alongside `Str`/`List`.

## Literals (revises ADR-0012)

ADR-0012 said "bigint literals use the `L` suffix"; that was incomplete. The
actual rule, and the one adopted:

- A **decimal integer literal that does not fit `i64` becomes a bigint** — no
  `L` needed (e.g. the 40- and 1000-digit literals in qa-bigint / qa-longnum).
- An **`L`-suffixed decimal literal is a bigint regardless of magnitude**
  (`1L`, `12L`, `100005L`), with an optional leading sign.
- **Hex literals stay `i64`** (wrapping); bigint hex is out of scope and its
  newLISP behaviour is unclear.

ADR-0012's **"no auto-promotion" still holds** — it is about *arithmetic
overflow*: `i64 + i64` that overflows wraps (ADR-0012) and does **not** become a
bigint. Only literals promote. With the feature **off**, the reader keeps its
current behaviour: an `L` / over-long literal is a clear error.

## Arithmetic promotion lattice

`+ - * / %` are the **integer** operators: a `float` argument is **truncated
toward zero to an integer** (established newLISP behaviour, already the case in
niiLISP — `to_i64` truncates). They never yield a float. So:

1. **any operand is a `bigint` → the result is a `bigint`** (every operand
   coerced to `BigInt`, a float truncated);
2. else **all-`i64` → `i64`, wrapping** (unchanged, ADR-0012; a float truncated
   to `i64`).

A bigint result that happens to fit `i64` **stays a bigint** (no auto-demote),
matching newLISP: `(/ 1234567891L 1234567890L)` → `1L`. Division and remainder
**truncate toward zero**, remainder taking the dividend's sign (`num-bigint`'s
`Div`/`Rem`, consistent with the `i64` path). Bigint division by zero errors.

The **float** operators `add`/`sub`/`mul`/`div` are separate and still coerce
every operand to `float` (a bigint via `to_f64`). (An earlier draft of this ADR
mistakenly gave `+` a float-result path; the integer operators truncate, as
above.)

## Conversions

- `(bigint x)`: `i64` → exact; `float` → truncated to its integer part (NaN/inf
  error); string → decimal parse (leading sign and a trailing `L` allowed,
  other non-digits error); bigint → itself.
- `(int big)`: the **low 64 bits as `i64`** (wrapping), consistent with the
  wrapping philosophy of ADR-0012; the out-of-range case is not otherwise
  specified.
- `(float big)`: `to_f64` (`inf` if too large, as in newLISP).

## Comparison, predicates, length

- `= != < > <= >=`: if **any operand is a `float`, compare as `f64`**;
  otherwise **lift `i64`/`bigint` operands to `BigInt` and compare exactly**
  (so `1L = 1` is true with no precision loss).
- `zero?` and `abs` handle bigint; `++`/`--` on a bigint place fall out of the
  `+`/`-` lattice for free.
- `integer?`, `number?`, `atom?` are **true** for a bigint; `float?` is false;
  `type_name` gains `"bigint"`.
- `length` of a **bigint is its decimal digit count** (sign excluded) — a
  newLISP quirk (`(length 1234567890123456789012345)` → 25). `length` of an
  `i64`/`float` stays `0`.

## Printing

A bigint prints as its **decimal digits with no `L` suffix** (negative values
lead with `-`), matching newLISP; `display` and `repr` share this. The `L` is
purely lexical and does not survive to output, so qa-longnum's
`string`→`explode`→digit-sum check sees a plain decimal string.

## Acceptance

- **Hermetic unit/integration tests** for the bigint core: 1000-digit `*` / `/`
  round-trip, over-long and `L` literals, the promotion lattice, cross-type
  comparison, `bigint`/`float` round-trip, `length` = digit count, `gcd`, and a
  `--no-default-features` build with the variant compiled out.
- Full **qa-bigint / qa-longnum stay gated** on the deferred RNG and
  string/list helpers; CURRENT_WORK records the co-dependencies.

## Consequences

- The numeric tower is now three-tiered (`i64` / `BigInt` / `f64`); every
  integer arithmetic and comparison builtin carries the three-way branch, and
  reviewers should treat a missing float-or-bigint check as a bug.
- The `Value` enum has a feature-gated variant — the first such — so `match`
  sites without a catch-all need a `cfg`-gated arm.
- `gcd` is implemented by Euclid on `BigInt` (`while b != 0 { (a,b)=(b,a%b) }`,
  then `abs`) to avoid pulling in `num-integer`.
