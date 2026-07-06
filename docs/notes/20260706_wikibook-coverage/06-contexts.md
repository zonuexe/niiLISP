# Ch. 6 — Contexts

Core context mechanics (creation, `Ctx:sym` prefixing, `symbols`, `dotree`, dictionaries-via-functor, `save`/`load`, basic FOOP dispatch) work, but several documented forms are broken or incomplete: the no-arg `(context)` query, the 3-arg `(context 'Ctx key val)` create-and-set shortcut, `def-new`, full implicit-context registration, and `:`-dispatch on a zero-arg inline FOOP constructor call.

**Coverage: 12 ✅ / 2 ⚠️ / 4 ❌**

| Feature | Status | Notes |
|---|---|---|
| `(context 'Name)` — create/switch context | ✅ | Returns context name symbol as documented |
| `(context)` — query current context, no args | ❌ | Errors instead of returning current context symbol |
| `Ctx:sym` prefixed access/assignment | ✅ | Read and write both work |
| `(context 'Ctx key val)` create+set shortcut | ❌ | Silently no-ops; value never set, context not even registered |
| `(context 'Ctx key)` retrieval form | ⚠️ | Returns the context name symbol itself, not the symbol's value |
| `symbols` | ✅ | Correct list of `Ctx:sym` symbols, in both quoted/unquoted-bound forms |
| `dotree` | ✅ | Iterates context symbols correctly |
| `term` | ✅ | Works on a symbol value (e.g. from `dotree`) |
| `prefix` | ✅ | Returns context symbol as documented |
| `name` | ✅ | Returns bare symbol name string |
| `sym` | ✅ | Creates/returns qualified symbol in target context |
| Implicit context creation via `(define (C:fn) ...)` | ✅ | Function defines and calls fine |
| Implicit context creation via `(define Ctx:sym val)` / `(set 'Ctx:sym val)` | ⚠️ | Symbol itself works, but the context is not fully registered (see gaps) |
| Default functor (`(define (Double:Double x) ...)`, call as `(Double 3)`) | ✅ | Works as documented |
| Context-as-dictionary via functor calls | ✅ | `(Doyle key val)`/`(Doyle key)`/`(Doyle)` alist form all work, once context exists |
| `(define Ctx:Ctx)` empty-dictionary one-liner | ❌ | Does not create the context at all; subsequent functor call errors `not a function: nil` |
| `context?` | ⚠️ | True for explicitly-created contexts; false for implicitly/`define`-created contexts (see gaps) |
| `new` (copy a context) | ✅ | Copies functions/symbols into new context correctly |
| `def-new` | ❌ | Not implemented — unbound symbol, calling it errors `not a function: nil` |
| `save` / `load` context round-trip | ✅ | Produces valid, reloadable `.lsp` source |
| FOOP: constructor + `(:method obj)` via bound variable | ✅ | `(set 'x (Ctor args)) (:show x)` works |
| FOOP: `(:method (Ctor args))` inline, with explicit args | ✅ | e.g. `(:show (Duration 122))` |
| FOOP: `(:method (Ctor))` inline, zero args (all defaults) | ❌ | Errors `colon dispatch: argument is not a FOOP object`; only fails with zero explicit args |
| FOOP: polymorphism / immutable "modifier" pattern | ✅ | Both work when going through a bound variable |

## Divergences & gaps

### ❌ `(context)` with no arguments errors instead of returning current context

```
$ niilisp -e '(println (context))'
niilisp: context: missing name
```
Book: `(context)` alone returns the current context symbol (e.g. `MAIN`).

### ❌ `(context 'Ctx key val)` create-and-set shortcut silently does nothing

```lisp
(println (context 'Doyle "villain" "moriarty"))
(println (context? 'Doyle))
(println Doyle:villain)
```
Output:
```
Doyle
nil
nil
```
Expected (per book): returns `"moriarty"`, `Doyle:villain` is set, and `Doyle` becomes a real context. Instead the extra arguments are ignored, the value is never stored, and the context isn't even registered (`context?` is `nil` right after).

### ⚠️ `(context 'Ctx key)` retrieval form returns the context symbol, not the value

```lisp
(context 'Doyle)
(set 'Doyle:hero "holmes")
(context 'MAIN)
(println (context 'Doyle 'hero))
```
Output: `Doyle` (should be `holmes` per book, mirroring `Doyle:hero`).

### ❌ `(define Ctx:Ctx)` empty-dictionary declaration doesn't create the context

```lisp
(define Doyle:Doyle)
(Doyle "key1" "val1")
```
Output:
```
niilisp: not a function: nil
```
`(context? 'Doyle)` is `nil` right after the `define`. Workaround: explicitly `(context 'Doyle) (context 'MAIN)` first, then the dictionary functor calls work fine. The book's idiomatic one-liner form is broken.

### ⚠️ Implicit context creation via `define`/`set` doesn't fully register the context

```lisp
(define D:greeting "hi")
(println D)               ; -> nil   (book/real newLISP: bare symbol resolves as context name, like MAIN or an explicitly (context 'X)'d symbol)
(println (context? 'D))   ; -> nil   (should be true — D is a real context with a symbol in it)
```
`(symbols 'D)` does correctly return `(D:greeting)`, so the context and its symbol both functionally exist — but the context is not registered as a first-class context object the way `(context 'Doyle)` registers `Doyle`. This is a partial implementation of implicit context creation.

### ❌ `def-new` is not implemented

```
$ niilisp -e '(println (def-new))'
niilisp: not a function: nil
```
Unbound symbol; no `def-new` builtin exists at all.

### ❌ FOOP `:`-dispatch fails on a zero-explicit-arg inline constructor call

```lisp
(define (Duration:Duration (d 0)) (list Duration d))
(define (Duration:show) (string (self 1) " days"))
(println (:show (Duration 122)))   ; -> "122 days"   (works)
(println (:show (Duration 0)))     ; -> "0 days"      (works, explicit arg)
(println (:show (Duration)))       ; -> ERROR
```
Output for the last line:
```
niilisp: colon dispatch: argument is not a FOOP object
```
Calling `(:show obj)` on a bound variable holding `(Duration)` works fine; the bug is specific to nesting a zero-argument constructor call directly as the `:`-dispatch argument.
