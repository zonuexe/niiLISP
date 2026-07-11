# Ch. 12 — Working with XML

The XML/JSON parsing family is implemented under
[ADR-0038](../../adr/0038-xml-and-json.md) (`xml-parse`, `xml-type-tags`,
`xml-error`, `json-parse`, `json-error`), and the tree-navigation helpers the
chapter relies on (`ref`/`ref-all`) landed with the reference model
([ADR-0036](../../adr/0036-reference-and-query-model.md)). The whole chapter now
passes.

**Coverage: 8 ✅ / 0 ⚠️ / 0 ❌**  *(updated 2026-07-06: XML/JSON parsers implemented)*

| Feature | Status | Notes |
|---|---|---|
| `xml-parse` | ✅ | Hand-rolled parser → newLISP's tagged-list tree; option flags 1/2/4/8/16 and skips DTD/PI. |
| `xml-type-tags` | ✅ | Customizes/suppresses the four TEXT/CDATA/COMMENT/ELEMENT type tags (interpreter state). |
| `xml-error` | ✅ | `("message" position)` after the last parse, or `nil` on success. |
| `ref` | ✅ | Index path to the first match (ADR-0036); walks the parsed tree. |
| `ref-all` | ✅ | All index paths; sets `$count`. |
| `set-ref-all` | ✅ | Search/replace over a list. |
| `json-parse` | ✅ | Objects → assoc lists, arrays → lists, `false`/`null` → symbols; queryable with `assoc`/`lookup`/`ref`. |
| `json-error` | ✅ | `("message" position)` after a failed parse, or `nil`. |

## Notes

Both parsers are pure Rust (no dependency, no feature flag) and emit the exact newLISP representation, so the chapter's `ref`/`ref-all`/`assoc` navigation over the parsed tree works unchanged.

```
$ niilisp -e "(println (xml-parse \"<person name='John Doe'>hi</person>\"))"
(("ELEMENT" "person" (("name" "John Doe")) (("TEXT" "hi"))))
$ niilisp -e '(println (json-parse "{\"a\": [1, true, null]}"))'
(("a" (1 true null)))
$ niilisp -e '(println (xml-parse "<a><b/>" )) (println (xml-error))'   # malformed → nil + error
```
