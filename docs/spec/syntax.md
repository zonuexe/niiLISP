# niiLISP Language Specification

This document specifies the language accepted by niiLISP. In practice it
describes the **newLISP dialect** — niiLISP is a re-implementation of it
(see [`CONTEXT.md`](../../CONTEXT.md)) — as embodied by the current
implementation. It is a pragmatic reference, not a formal standard: where the
language is large the spec favours "how it behaves" over exhaustive rules.

"newLISP" and "Nuevatec" are trademarks of Lutz Mueller; this is an independent
specification of a compatible dialect.

> **Implementation status.** Sections marked _(partial)_ or _(planned)_ describe
> the dialect but are not yet fully implemented in niiLISP. See
> [`docs/CURRENT_WORK.md`](../CURRENT_WORK.md) for the roadmap.

## Notation

Grammar fragments use a light EBNF:

- _italic_ names are nonterminals; literal text and punctuation are terminals.
- `x ::= …` defines a rule; `|` separates alternatives.
- `x?` optional, `x*` zero or more, `x+` one or more.
- `/…/` is a regular expression describing a lexical pattern.

---

## 1. Lexical structure

Source is a sequence of bytes (UTF-8 by convention). Tokens are separated by
whitespace and delimiters; source is read as a stream of s-expressions.

### 1.1 Whitespace and comments

Spaces, tabs, carriage returns, newlines, and **commas** are whitespace. A
comment runs from `;` or `#` to the end of the line (so a `#!/usr/bin/env
niilisp` shebang is a comment).

```
; a line comment
(+ 1, 2, 3)     ; commas are whitespace -> 6
```

### 1.2 Numbers

```
integer ::= /-?[0-9]+/ | /-?0[xX][0-9a-fA-F]+/
float   ::= /-?[0-9]*\.[0-9]+([eE][-+]?[0-9]+)?/ | /-?[0-9]+[eE][-+]?[0-9]+/
```

Integers are 64-bit signed; integer arithmetic **wraps** on overflow. Floats are
IEEE-754 doubles. A trailing `L` denotes a bigint literal (`123L`); bigint is
_(planned)_ and currently rejected with a read error.

### 1.3 Strings

A string is a **binary-safe byte buffer**, not guaranteed-UTF-8 text. There are
three syntaxes:

- **Quoted** `"…"` — processes escapes: `\n \t \r \\ \"` and `\ddd`, a decimal
  byte value (e.g. `\195`). Does not span lines.
- **Braced** `{…}` — raw, may span lines, braces nest, no escape processing.
- **Tag** `[text]…[/text]` — raw verbatim block for large text.

```
"line\nbreak, u umlaut = \195\188"
{raw {nested} braces, no \n escaping}
[text]anything until the closing tag, even ( ) " {[/text]
```

`length` on a string counts **bytes**; character-oriented operations (`utf8len`,
char indexing) are _(planned)_.

### 1.4 Symbols and constants

A symbol is a run of non-delimiter characters that is not a number. Delimiters
are whitespace, `( ) " { ' ;` and `#`. Symbols may contain `.` (e.g. `a.re`).
The colon `:` is the context separator:

```
symbol      ::= /[^ \t\r\n,(){}';#"]+/     ; that is not a number
qualified   ::= context ":" name           ; e.g. MAIN:x, complex:rad
colon-form  ::= ":" name                   ; e.g. :area  (colon dispatch, §7)
```

`nil` and `true` are constants. `'x` is shorthand for `(quote x)`.

---

## 2. Values

The value types are (per-type detail in [`types.md`](types.md)):

| Type      | Examples / notes |
| --------- | ---------------- |
| `nil`     | the empty/false value |
| `true`    | the canonical true value |
| integer   | 64-bit signed |
| float     | IEEE-754 double |
| string    | binary-safe byte buffer (§1.3) |
| symbol    | interned name, optionally context-qualified |
| list      | ordered sequence, array-backed (no dotted pairs) |
| context   | a namespace / FOOP class (§7) |
| lambda    | a function; `fexpr` (unevaluated args) is a related kind |
| foreign   | a C function bound by `import` (§10) |

**Value semantics (ORO).** Values are not shared: they are deep-copied when
stored in a structure or passed to a function, and there is no object identity —
only structural equality (`=`). This is newLISP's "One Reference Only" model.
Lists are therefore ordinary values, and there are **no cons cells or dotted
pairs**: `(cons 1 2)` yields the list `(1 2)`, not `(1 . 2)`.

---

## 3. Evaluation

A program is a sequence of top-level expressions, each read and evaluated in
order.

- `nil`, `true`, numbers, and strings **self-evaluate**.
- A **symbol** evaluates to its current value, or `nil` if unset.
- A **list** `(op arg…)` is evaluated by its first element `op`:

```
form ::= atom
       | ( )                       ; -> nil
       | ( special-form arg* )     ; §4  (some args unevaluated)
       | ( :name obj arg* )        ; §7  colon dispatch
       | ( function arg* )         ; args evaluated, then applied
       | ( fexpr arg* )            ; args passed unevaluated
       | ( context arg* )          ; §7  default functor (construction)
       | ( integer index* )        ; §5  implicit indexing
       | ( list index* )           ; §5  implicit indexing
```

`quote` suppresses evaluation: `(quote x)` / `'x` returns `x` unevaluated.

**Truthiness.** Everything is true except `nil` and the empty list `()`.

---

## 4. Scoping

niiLISP is **dynamically scoped** and has **no lexical closures**. A called
function sees the caller's current bindings for any symbol it does not itself
rebind. Bindings (function parameters, `let`, `local`) are established by saving
a symbol's current value, installing a new one, and restoring it as evaluation
unwinds.

Symbols live in **contexts** (namespaces); the default context is `MAIN`. A
symbol is referred to across contexts as `Ctx:name`. Contexts are first-class
values and double as FOOP classes (§7).

---

## 5. Lists and implicit indexing

Lists are the core structured value. When the operator position holds an
**integer** or a **list**, the form performs indexing instead of a call:

```
(nth 1 '(a b c))     ; -> b
('(a b c) 1)         ; -> b        (list in operator position)
(1 '(a b c d))       ; -> b        (integer in operator position)
(0 2 '(a b c d))     ; -> (a b)    (start count list) slice
```

Indices may be negative (from the end). Implicit indexing composes:
`((L 2) 0)` indexes `L[2][0]`.

---

## 6. Special forms

Special forms control evaluation of their arguments. Syntax lines below use the
notation from the top. `body` is a sequence of expressions; its value is that of
the last one (`nil` if empty). For a per-form reference — which arguments each
one evaluates, with examples and edge cases — see
[`special-forms.md`](special-forms.md).

### 6.1 Definition and functions

```
(define name value)
(define (name param*) body)          ; function-definition sugar
(lambda (param*) body)   |  (fn (param*) body)
(lambda-macro (param*) body)         ; a fexpr: args are NOT evaluated
(define-macro (name param*) body)    ; named fexpr
(constant name value ...)            ; like set, but marks name read-only
```

A `param` is a symbol, or `(symbol default)` for a **default value** used when
the argument is missing.

```
(define (add (a 0) (b 0)) (+ a b))
(add 5)          ; -> 5   (b defaults to 0)
```

A `lambda-macro`/`define-macro` receives its arguments unevaluated (a runtime
fexpr, not a hygienic macro); it decides what to evaluate:

```
(define-macro (my-if c t e) (if (eval c) (eval t) (eval e)))
```

### 6.2 Binding

```
(let (sym expr sym expr ...) body)   ; FLAT binding list; inits eval in outer scope
(local (sym ...) body)               ; bind each sym to nil for the body
```

```
(let (a 3 b 4) (add (mul a a) (mul b b)))   ; -> 25
```

### 6.3 Assignment and places

```
(set 'sym value ...)     ; target is evaluated to a symbol
(setq place value ...)   ; target is a literal place (§8)
(setf place value ...)   ; alias of setq; binds $it to the place's current value
```

`(setf x (+ $it 1))` uses `$it` for the place's prior value.

### 6.4 Conditionals

```
(if cond then [c2 e2 …] [else])      ; multi-way; else is the trailing odd form
(if-not cond then [else])            ; inverse of if
(cond (test body) …)
(case key (label body) … (true default))   ; literal labels; true is the default
(when cond body)  |  (unless cond body)
(and expr …)      |  (or expr …)     ; short-circuit; return the deciding value
```

### 6.5 Iteration

```
(while cond body)
(for (var from to [step]) body)      ; inclusive; direction auto by from/to
(dolist (var list [break]) body)
(dotimes (var count) body)
```

### 6.6 Sequencing, mutation, quoting

```
(begin body)                         ; evaluate in sequence, return last
(push value place [index])           ; insert into a list place; returns the list
(pop place [index])                  ; remove and return an element
(inc place [n]) (dec place [n])      ; in-place numeric change (also ++ / --)
(reverse x) (sort x) (rotate x [n])  ; destructive on a place, else on a copy
(replace target place new)           ; replace matches in a list/string place
(set-ref key place new)              ; deep-replace first match; -all for every match
(quote x)  |  'x
```

### 6.7 FOOP, control, timing

```
(self [index …])                     ; the current object in a method (§7)
(catch expr ['result-sym])           ; §9
(throw value)                        ; §9
(time expr)                          ; milliseconds spent evaluating expr
```

---

## 7. Contexts and FOOP

A **context** is a namespace and, in object use, a **class**. FOOP (Functional
Object-Oriented Programming) represents an object as an ordinary **list whose
head is its class context's symbol** — objects are a convention over lists, not
a distinct type.

```
(new Class 'Point)                   ; create context Point
(define (Point:Point (x 0) (y 0))    ; default functor = constructor
  (list Point x y))
(define (Point:move dx dy)           ; a method; self is the target object
  (inc (self 1) dx) (inc (self 2) dy) (self))

(set 'p (Point 3 4))                 ; apply a context -> default functor
(:move p 10 20)                      ; colon dispatch: run Point:move on p
```

- **Default functor.** Applying a context `(Ctx arg…)` invokes `Ctx:Ctx` if
  defined, else builds the tagged object list.
- **Colon dispatch.** `(:method obj arg…)` resolves `method` in the class named
  by the head of `obj`, and runs it with `self` bound to `obj`.
- **`self`.** Inside a method, `self` is the target object and is a **place**:
  `(inc (self 1) …)` writes back into the stored object.
- **Protection.** A `constant` object cannot be mutated through `self`.

---

## 8. The reference / place model

Destructive operations act on **places** and return **references**, so they
compose. A place is a location within a stored value:

```
place ::= symbol
        | ( place index … )          ; implicit index into a place
        | ( nth i place )
        | ( first place ) | ( last place )
        | ( assoc key place ) | ( lookup key place [i] )
```

Crucially, **most expressions can yield a reference**, not just a copy: control
forms and place-returning builtins pass a reference out, so a following
destructive op reaches the original:

```
(set 'L '(a b c d e f))
(setf (first L) 99)                  ; L -> (99 b c d e f)
(pop (if true L))                    ; pops from L
(replace 'b (begin (and (or L))) 'B) ; edits L through the control forms
(pop (sort s))                       ; sort in place, pop the smallest
(push 'z (setq x '(a b c)) -1)       ; setq returns a reference to x
```

---

## 9. Error handling

Errors and `throw` both unwind to the nearest enclosing `catch`.

```
(catch expr)          ; -> the value, or the caught thrown value / error string
(catch expr 'sym)     ; sym <- result-or-thrown; returns true normally, nil on exit
(throw value)         ; non-local exit to the nearest catch
```

```
(catch (throw 'boom) 'r)   ; -> nil, and r is 'boom
```

There is no tail-call optimisation; deep non-tail recursion is bounded by a
maximum call depth.

---

## 10. Foreign functions (FFI)

`import` binds a C function so it is callable like any function; `callback`
turns a niiLISP function into a C function pointer. Types are:
`void int long float double char* void*`.

```
(import "libm.so" "cos" "double" "double")   ; then (cos x)
(import "lib" "fn" "ret" "argtype" …)        ; returns nil if it cannot resolve
(callback 'f "ret" "argtype" …)              ; -> a C function-pointer address
(struct 'Point "int" "int")                  ; name a struct layout
(pack Point 3 4)                             ; -> a binary string (native C ABI)
(unpack Point s)                             ; -> (3 4)
(get-string addr) (get-int addr) …           ; read a C value at an address
(address 'sym)                               ; buffer address of a symbol's string
```

`char*` arguments are passed as a temporary NUL-terminated copy; a `char*`
return is copied out to a string (NULL becomes `nil`); pointers are integers. A
`throw`/error inside a callback is reported to stderr and does not cross the C
boundary.

The memory API builds and reads C structs. `struct` names a layout (a list of C
type names); `pack`/`unpack` convert between values and a binary string laid out
as that C struct (natural alignment, padding, native byte order). `get-string`,
`get-int`, `get-long`, `get-float` (a C `double`), and `get-char` read a C value
at an integer address; `address` exposes a symbol-held string's stable buffer
pointer (valid only while the symbol is not reassigned or resized). A packed
struct is handed to C by passing the string to a `void*` argument (no copy). A
NULL (0) pointer through `unpack`/`get-string` raises an error; other invalid
addresses are undefined behaviour. FFI is Unix-only for now.

---

## 11. Standard functions (overview)

Not exhaustive; a categorised map of the built-in vocabulary. For a per-function
list see [`functions.md`](functions.md).

- **Integer arithmetic** (wrapping): `+ - * / %`.
- **Float arithmetic**: `add sub mul div`, `sqrt pow exp log`, `sin cos tan
  asin acos atan`, `abs`, `mod` (NaN on zero divisor). `int` / `float` convert.
- **Comparison**: `= != < > <= >=` (numeric or string; NaN compares as false).
- **Bitwise**: `& | ^ << >>`.
- **Lists**: `list cons first rest last nth length append reverse sort`,
  `map apply filter sequence dup`.
- **Predicates**: `nil? null? true? integer? float? number? string? symbol?
  list? atom? zero? empty? NaN? inf?`.
- **Strings**: `string starts-with ends-with char`, `format` (printf subset:
  flags, width, `.precision`, and `d i u f e g x X o s c`).
- **I/O / misc**: `print println`, `time-of-day`, `set-locale`, `exit`, `eval`,
  `new`.

---

## 12. Non-goals and deviations from traditional Lisp

A summary; the full catalogue with a cross-dialect comparison is in
[`compatibility.md`](compatibility.md).

- **No cons cells / dotted pairs**, and lists are values (ORO), so there is **no
  `eq` identity** — only structural `=`.
- **Dynamic scope only**, and **no closures**; contexts provide encapsulation.
- **Macros are runtime fexprs** (`lambda-macro`), not hygienic compile-time
  macros. Code is live data (self-modifying code is possible in principle).
- **No tail-call optimisation** and **no continuations**.
- The numeric tower is minimal (int64 + double; bigint _(planned)_). Arrays,
  the FFI memory API, and networking are _(planned)_.
