# Ch. 8 — Working with numbers

Core arithmetic, comparison, formatting, and the **bigint model** all work as newLISP specifies. The real gaps are unbound numeric builtins (`series`, `factor`, `normal`, `deg->rad`, `rad->deg`, `~`) and a handful of functions (`int`, `pow`, `round`) with different argument/error semantics than the book describes.

**Coverage: 26 ✅ / 5 ⚠️ / 7 ❌**

> Corrections (verified against the binary + newLISP 10.7.5 manual): four bigint-related verdicts were wrong.
> - `(zero? 0)` → `true` (works; the original `nil` reading could not be reproduced).
> - The `L` suffix **is** honored: `(* 99999999999999999999L 99999999999999999999L)` → exact 40-digit `9999999999999999999800000000000000000001`. The earlier "ignored" reading came from a test whose result still fit in 64 bits.
> - "No auto-promotion on plain-int overflow" is **not** a divergence: newLISP also wraps at 64 bits and only does bigint arithmetic when an operand is already a bigint (or a literal exceeds 64 bits — `(bigint? 123456789012345678901234567890)` → `true` in both). niiLISP reproduces this exactly.
> - Factorial therefore works the newLISP way — seed the base case with `1L`: `(fact 30)` → `265252859812191058636308480000000` (exact).

| Feature | Status | Notes |
|---|---|---|
| `+ - * %` (integer ops) | ✅ | Matches book |
| `/` integer division truncates | ✅ | `(/ 10 3)` → `3` |
| float arithmetic promotion (`(+ 1.5 2)`) | ✅ | `3` shown as `3`... see note below |
| `add/sub/mul/div/mod` | ✅ | Work as float-returning equivalents |
| `PI` constant | ⚠️ | Not predefined; must define via `constant` yourself |
| `int` (string→int, with default) | ⚠️ | Non-numeric string doesn't return `nil`/default; returns `0` |
| `int` with explicit base (hex/octal/binary parsing) | ❌ | `0x`/`0b` prefix autodetection and explicit-base parsing broken/unsupported |
| `integer?` / `float?` / `number?` | ✅ | Match book semantics |
| `zero?` | ✅ | `(zero? 0)` → `true`, `(zero? 5)` → `nil` |
| `integer?` on `div` result | ✅ | `div` returns float, `integer?` correctly `nil` |
| `floor` | ✅ | Matches |
| `ceil` returns float | ✅ | Matches |
| `round` with negative digit count | ⚠️ | `(round 1234.6789 -1)` → `1230` instead of book's `1234.70000` |
| `round` with 0 digits | ✅ | `1235` (format differs but value matches) |
| `sgn` | ✅ | Matches |
| `pow` one-arg form (square) | ❌ | `(pow 2)` errors "expected 2 arguments"; book computes `2^2=4` |
| `pow` two-arg / fractional exponent | ✅ | Matches |
| `sqrt` | ✅ | Matches |
| `exp` | ✅ | Matches |
| `log` (1-arg and 2-arg forms) | ✅ | Matches |
| `sin/cos/tan/asin/acos/atan/atan2/sinh/cosh/tanh` | ✅ | All present and correct (radians) |
| `deg->rad` / `rad->deg` | ❌ | Unbound — errors "not a function: nil" |
| `rand` (random integer list) | ✅ | Present, correct shape/range |
| `random` (random float list) | ✅ | Present, correct shape/range |
| `normal` (normal distribution) | ❌ | Unbound |
| `seed` | ⚠️ | Accepts int seed fine; `(seed (date-value))` fails — `date-value` unbound |
| `randomize` | ✅ | Shuffles list correctly |
| `sequence` | ✅ | Matches book output exactly |
| `series` | ❌ | Unbound |
| `min` / `max` (varargs, mixed int/float) | ✅ | Matches, including float contagion |
| `format "%x"` (hex) | ✅ | Matches |
| `format "%1.16f"` (float precision) | ✅ | Matches once `PI`/inputs are floats |
| `factor` | ❌ | Unbound |
| `gcd` | ✅ | Matches |
| `<< >>` bitwise shift, 2-arg | ✅ | Matches |
| `<< >>` bitwise shift, 1-arg (implicit shift by 1) | ⚠️ | Errors "expected 2 arguments"; book's `(<< 6)` → `12` has no 1-arg form |
| `^ \| &` bitwise xor/or/and | ✅ | Matches |
| `~` bitwise not | ❌ | Unbound |
| 64-bit int overflow wraps (no silent bigint) | ✅ | Matches newLISP: plain `int * int` wraps at 64 bits; bigint only when an operand is bigint |
| too-large integer literal auto-parses as bigint | ✅ | `(bigint? 123456789012345678901234567890)` → `true` |
| `L` literal suffix for big integers | ✅ | `(* 99999999999999999999L 99999999999999999999L)` → exact 40-digit result |
| `bigint` explicit constructor + arithmetic | ✅ | `(bigint 1E+80)`, `(++ atoms)` give correct arbitrary-precision results |
| recursive factorial via bigint seed | ✅ | `(fact 30)` seeded with `1L` → exact `265252859812191058636308480000000` (as in newLISP) |

## Divergences & gaps

### `int` doesn't validate/error on non-numeric strings
```
$ niilisp -e '(println (int "x"))'
0
```
Book: `(int "x")` → `nil`, `(int "x" 0)` → `0`. niiLISP returns `0` in both cases, silently swallowing the error path.

### `int` doesn't parse hex/octal/binary prefixed strings
```
$ niilisp -e '(println (int (string "0x" "1F")))'
0
$ niilisp -e '(println (int "0b100100100101001001000000000000000000000000010100100"))'
0
$ niilisp -e '(println (int "08" 0 10))'
8
```
Book expects `31`, `160881958715556`, and `8` respectively (the last one, base-10 parsing, does work). Hex/binary auto-detection and arbitrary-base parsing are unimplemented; only plain decimal (and default-value fallback) is handled, and the fallback returns `0` instead of `nil`/the supplied default when parsing genuinely fails.

### `round` with negative digit argument does not round to that decimal place
```
$ niilisp -e '(println (round 1234.6789 -1))'
1230
```
Book: `(round 1234.6789 -1)` → `1234.70000` (negative arg rounds to 1 decimal place in newLISP's convention, not to the tens place). niiLISP's negative-argument semantics differ entirely (appears to round to `10^|n|`, i.e. truncating integer digits, rather than rounding decimals).

### `pow` requires exactly 2 arguments (no implicit square)
```
$ niilisp -e '(println (pow 2))'
niilisp: pow: expected 2 arguments
```
Book: `(pow 2)` → `4` (one-arg form squares the number).

### `deg->rad` / `rad->deg` unbound
```
$ niilisp -e '(println (deg->rad 90))'
niilisp: not a function: nil
$ niilisp -e '(println (rad->deg 1))'
niilisp: not a function: nil
```
Both are used throughout the book's trig examples to convert between degrees and radians; niiLISP has no equivalent, so all degree-based trig examples in the book cannot be run as written (radian-native trig itself works fine).

### `normal` (normal/Gaussian distribution) unbound
```
$ niilisp -e '(println (normal 10 5 6))'
niilisp: not a function: nil
```
Book: `(normal 10 5 6)` → 6 normally-distributed samples around mean 10, stddev 5.

### `seed` with `date-value` argument fails (missing `date-value`)
```
$ niilisp -e '(println (seed (date-value)))'
niilisp: not a function: nil
```
`seed` itself works with a plain integer (`(seed 123)` succeeds), but `date-value` — commonly used to seed from wall-clock time — is unbound.

### `series` unbound
```
$ niilisp -e '(println (series 1 2 20))'
niilisp: not a function: nil
$ niilisp -e '(println (series 10 sqrt 20))'
niilisp: not a function: nil
```
Book: `(series 1 2 20)` → `(1 2 4 8 16 32 ... 524288)`; `(series 10 sqrt 20)` iteratively applies `sqrt`. Neither numeric nor functional form of `series` exists; only `sequence` is implemented.

### `factor` unbound
```
$ niilisp -e '(println (factor 42))'
niilisp: not a function: nil
```
Book: `(factor 42)` → `(2 3 7)` (prime factorization). `gcd` is implemented and correct, but `factor` is not.

### `<<` / `>>` require 2 arguments; book's 1-arg shift-by-one form is unsupported
```
$ niilisp -e '(println (<< 6))'
niilisp: <<: expected 2 arguments
$ niilisp -e '(println (>> 6))'
niilisp: >>: expected 2 arguments
```
Book: `(<< 6)` → `12` (implicit shift-by-1), `(>> 6)` → `3`. The 2-arg forms (`(<< 6 1)` → `12`, `(>> 6 1)` → `3`) work correctly.

### `~` (bitwise NOT) unbound
```
$ niilisp -e '(println (~ 5))'
niilisp: not a function: nil
```
Book: `(~ 5)` → `-6`.

### Big integers behave exactly as newLISP specifies (not a gap)

niiLISP reproduces newLISP's bigint model faithfully. Promotion is **operand-triggered**, not overflow-triggered:

- Plain 64-bit overflow wraps (this is also true in newLISP):
  ```
  $ niilisp -e '(println (* 9223372036854775807 2))'
  -2
  ```
- A literal too large for 64 bits auto-parses as a bigint:
  ```
  $ niilisp -e '(println (bigint? 123456789012345678901234567890))'
  true
  ```
- The `L` suffix forces bigint arithmetic and stays exact:
  ```
  $ niilisp -e '(println (* 99999999999999999999L 99999999999999999999L))'
  9999999999999999999800000000000000000001
  ```
- So the book's factorial works the newLISP way — seed the base case with a bigint (`1L`):
  ```
  $ niilisp -e '(define (fact n) (if (< n 2) 1L (* n (fact (- n 1))))) (println (fact 30))'
  265252859812191058636308480000000
  ```
- The explicit `bigint` constructor is likewise arbitrary-precision:
  ```
  $ niilisp -e '(set (quote atoms) (bigint 1E+80)) (println (++ atoms))'
  100000000000000000026609864708367276537402401181200809098131977453489758916313089
  ```

(A plain-int `(fact 30)` with a `1` base case wraps to a wrong value — but that is identical to newLISP, which only does bigint math once a bigint operand is present.)
