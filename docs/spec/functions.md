# niiLISP Built-in Functions

A quick reference for the built-in **functions** — primitives whose arguments
are all evaluated before the call. Special forms (which control evaluation) are
in [`special-forms.md`](special-forms.md); the language overview is in
[`syntax.md`](syntax.md).

> This is a preliminary, deliberately terse list — one line per function — to be
> organised and expanded later. Signatures use `name arg…`; `[x]` is optional,
> `x…` is variadic.

## Integer arithmetic (wrapping)

| Function | Meaning |
| --- | --- |
| `(+ n…)` | sum |
| `(- n…)` | difference; `(- n)` negates |
| `(* n…)` | product |
| `(/ n…)` | integer quotient; error on divide by zero |
| `(% a b)` | integer remainder; error on zero divisor |

## Float arithmetic and math

| Function | Meaning |
| --- | --- |
| `(add n…)` `(sub n…)` `(mul n…)` `(div n…)` | float `+ - * /` |
| `(mod a b)` | float modulo; NaN on zero divisor |
| `(pow x y)` | `x` to the power `y` |
| `(sqrt x)` `(exp x)` `(log x [base])` | root, exp, log (natural, or base) |
| `(sin x)` `(cos x)` `(tan x)` `(asin x)` `(acos x)` `(atan x)` | trigonometry |
| `(abs x)` | absolute value (integer or float) |
| `(NaN? x)` `(inf? x)` | NaN / infinity predicates |

## Conversion

| Function | Meaning |
| --- | --- |
| `(int x [default])` | to integer (float truncates/saturates; string parses) |
| `(float x [default])` | to float |
| `(char n)` / `(char "s")` | code point to 1-char string / first code point of string |

## Comparison

| Function | Meaning |
| --- | --- |
| `(= a…)` `(!= a…)` | structural equality / inequality |
| `(< a…)` `(> a…)` `(<= a…)` `(>= a…)` | ordering of numbers or strings (NaN compares false) |

## Bitwise

| Function | Meaning |
| --- | --- |
| `(& n…)` `(\| n…)` `(^ n…)` | bitwise and / or / xor |
| `(<< x n)` `(>> x n)` | shift left / right |

## Lists and sequences

| Function | Meaning |
| --- | --- |
| `(list x…)` | make a list |
| `(cons x lst)` | prepend (no dotted pairs; else a 2-list) |
| `(first lst)` `(rest lst)` `(last lst)` | head / tail / final element |
| `(nth i lst)` | element `i` (negative from the end) |
| `(length x)` | list length, or string **byte** length |
| `(append lst…)` | concatenate lists (or strings) into a copy |
| `(sequence from to [step])` | list of numbers, inclusive |

List construction semantics (array-backed values, no dotted pairs) are in
[`types.md`](types.md). Destructive list operators (`push`, `pop`, `reverse`,
`sort`, `rotate`, `replace`, `set-ref`) are **special forms** — see
[`special-forms.md`](special-forms.md).

## Higher-order

| Function | Meaning |
| --- | --- |
| `(map f lst…)` | apply `f` across the lists |
| `(apply f lst)` | call `f` with the list's elements as arguments |
| `(filter pred lst)` | keep elements where `pred` is truthy |
| `(dup x n)` | repeat a string `n` times, or make a list of `n` copies |

## Association lists

| Function | Meaning |
| --- | --- |
| `(assoc key alist)` | the `(key …)` pair, or `nil` |
| `(lookup key alist [i])` | element `i` (default last) of the matching pair |

## Predicates

| Function | Meaning |
| --- | --- |
| `(nil? x)` / `(null? x)` | is `nil` |
| `(integer? x)` `(float? x)` `(number? x)` | numeric kind |
| `(string? x)` `(symbol? x)` `(list? x)` | type |
| `(atom? x)` | not a list |
| `(zero? x)` | numerically zero |
| `(empty? x)` | empty list/string, or `nil` |
| `(not x)` | logical negation (`nil`/`()` are false) |

## Strings

| Function | Meaning |
| --- | --- |
| `(string x…)` | concatenate arguments to a string |
| `(starts-with s prefix)` `(ends-with s suffix)` | prefix / suffix test |
| `(upper-case s)` `(lower-case s)` | ASCII case conversion (bytes ≥ 0x80 unchanged) |
| `(trim s [l [r]])` | strip a char (default space) from both ends, or `l`/`r` per side |
| `(slice seq start [len])` | copied sub-range of a string or list (see below) |
| `(find key seq)` | index of a substring / list element, else `nil` |
| `(format fmt arg…)` | printf-style: flags, width, `.precision`; `d i u f e g x X o s c` |

`slice` and `find` index strings by **byte** (ADR-0013). In `slice`, a negative
`start` counts from the end and a negative `len` drops that many trailing
elements; out-of-range bounds clamp. `find` on a list compares elements
structurally (`=`).

## Evaluation, objects, system

| Function | Meaning |
| --- | --- |
| `(eval expr)` | evaluate a value as code |
| `(new prototype 'name)` | create a context (FOOP class) |
| `(print x…)` `(println x…)` | write to stdout (no quotes; `println` adds a newline) |
| `(time-of-day)` | milliseconds since the epoch |
| `(set-locale ["C"])` | locale (currently a no-op returning `"C"`) |
| `(exit [code])` | terminate the process |

## FFI (Unix, `ffi` feature)

| Function | Meaning |
| --- | --- |
| `(import "lib" "fn" "ret" "arg"…)` | bind a C function; `nil` if unresolved |
| `(callback 'func "ret" "arg"…)` | a C function pointer that calls `func` |
| `(struct 'name "type"…)` | bind `name` to a struct layout (a list of C types) |
| `(pack layout val…)` | serialise values to a binary string |
| `(unpack layout str)` | read a packed string back into a list of values |
| `(get-string addr [len [limit]])` | read a C string at `addr` |
| `(get-int addr)` `(get-long addr)` | read a 32-/64-bit integer at `addr` |
| `(get-float addr)` `(get-char addr)` | read a C `double` / signed byte at `addr` |
| `(address 'sym)` | stable buffer address of a symbol-held string |

Types: `void int long float double char* void*`. See `syntax.md` §10.

A packed struct is passed to C by handing the string to a `void*` argument (no
copy, binary-safe, valid for the call). With a **struct** layout, `pack`/`unpack`
use the native C ABI layout — natural alignment, padding, and byte order — so a
packed string is exactly what a C function accepts as that struct.

`pack`/`unpack` alternatively take a **format string** (packed tightly, no
alignment): `c`/`b` = signed/unsigned 8-bit, `d`/`u` = 16-bit, `ld`/`lu` =
32-bit, `Ld`/`Lu` = 64-bit, `f` = float, `lf` = double, `sN` = an N-byte string,
`nN` = N null bytes; `>` / `<` switch the following fields to big-/little-endian
(default: native). Whitespace between specifiers is ignored — e.g.
`(pack "c c c" 65 66 67)` → `"ABC"`, `(unpack ">ld lf" s)`.

A NULL (0) pointer through
`unpack` (for a `char*` field) or `get-string` raises an error rather than
dereferencing; other invalid addresses are undefined behaviour (the caller's
risk, per ADR-0015). `address` is valid only while `sym` is neither reassigned
nor resized.
