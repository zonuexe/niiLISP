# Ch. 12 — Working with XML

niiLISP has no XML (or JSON) support at all — the entire chapter's core function, `xml-parse`, is unbound, along with `xml-type-tags`, `xml-error`, and the tree-navigation helpers `ref`/`ref-all` that the chapter relies on to walk the parsed S-expression; only the incidental `set-ref-all` special form exists and works.

**Coverage: 1 ✅ / 0 ⚠️ / 6 ❌**

| Feature | Status | Notes |
|---|---|---|
| `xml-parse` | ❌ | Unbound; core chapter function entirely missing |
| `xml-type-tags` | ❌ | Unbound |
| `xml-error` | ❌ | Unbound |
| `ref` | ❌ | Unbound (needed to navigate parsed XML tree) |
| `ref-all` | ❌ | Unbound |
| `set-ref-all` | ✅ | Bound and works as a search/replace over a list |
| `json-parse` / `json-error` | ❌ | Not mentioned in this chapter, but checked since related to XML parsing family; both unbound |

## Divergences & gaps

### `xml-parse` — unbound

```
$ ./target/release/niilisp -e '(println (xml-parse "<a>1</a>"))'
niilisp: not a function: nil
```
Book expects a call like `(xml-parse xml 15)` to return a nested S-expression tree, e.g. `((feed ((xmlns "...") (xml:lang "en-gb")) (link (...)) (title "...") ...))`. niiLISP has no such builtin — `xml-parse` evaluates as the unbound symbol `nil`, and calling it errors with "not a function: nil" rather than any XML-related diagnostic.

### `xml-type-tags` — unbound

```
$ ./target/release/niilisp -e '(println (xml-type-tags nil nil nil nil))'
niilisp: not a function: nil
```
Book uses this to suppress default `"ELEMENT"`/`"TEXT"`/etc. type labels before parsing. No equivalent exists.

### `xml-error` — unbound

```
$ ./target/release/niilisp -e '(println (xml-error))'
niilisp: not a function: nil
```

### `ref` — unbound

```
$ ./target/release/niilisp -e "(println (ref 'b (list 'a 'b 'c)))"
niilisp: not a function: nil
```
Book uses `(ref 'entry sxml)` to get the address of the first match inside a parsed XML tree, e.g. `;-> (0 9 0)`. Unavailable in niiLISP, which further blocks any of the navigation workflow shown in the chapter (`chop`, `lookup`, etc. built on top of `ref`'s output).

### `ref-all` — unbound

```
$ ./target/release/niilisp -e "(println (ref-all 'b (list 'a 'b 'c 'b)))"
niilisp: not a function: nil
```
Book: `(ref-all 'title sxml)` returns all matching addresses, e.g. `;-> ((0 3 0) (0 9 5 0) (0 10 5 0) ...)`. Unavailable.

### `json-parse` / `json-error` — unbound

Not covered by this chapter's text, but probed since it's part of the same parsing-function family in real newLISP.

```
$ ./target/release/niilisp -e '(println (json-parse "{\"a\":1}"))'
niilisp: not a function: nil
$ ./target/release/niilisp -e '(println (json-error))'
niilisp: not a function: nil
```

### `set-ref-all` — works (the one bright spot)

```
$ ./target/release/niilisp -e "(println (set-ref-all 'title \"X\" (list (list 'title \"old\") 'b)))"
X
```
This special form is implemented (`src/eval.rs`, handled as `"set-ref-all"` in the special-form dispatcher) and does perform search-and-replace over a list, matching the book's description of `set-ref-all`'s role — though since `ref`/`ref-all` are missing, it's an isolated capability without the rest of the XML-navigation toolchain around it.
