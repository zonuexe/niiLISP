# XML and JSON: hand-rolled pure-Rust parsers

The *Working with XML* chapter is entirely unbuilt, and JSON (its close cousin in
newLISP) with it. This ADR adds `xml-parse`, `xml-type-tags`, `xml-error`,
`json-parse`, and `json-error`. Both parsers translate their input into the exact
newLISP S-expression shape so vendored scripts read the result with `assoc` /
`lookup` / `ref` unchanged.

The overriding goal (per `CONTEXT.md`) is compatibility with existing newLISP
assets; the output representations below are fixed by the 10.7.x manual, not
chosen.

## Hand-rolled recursive-descent parsers, no dependency

- **Chosen:** write both parsers by hand in pure Rust, always compiled in. JSON
  (ECMA-262) is a small recursive-descent grammar; XML is the **well-formed 1.0
  subset** newLISP itself supports — elements, attributes, text, `CDATA`,
  comments — skipping DTDs and processing instructions (no validation), exactly
  as the manual states. Neither needs a crate.
- **Rejected:** pulling `serde_json` / `quick-xml` / `roxmltree`. They are capable
  and fast, but they are heavy dependencies that the zero-dependency
  `--no-default-features` build avoids, and — more importantly — they produce
  their *own* data models. We would then have to translate those into newLISP's
  tagged-list shape anyway, so the crate buys little over a direct parser that
  emits the target representation as it goes.
- **Rejected:** a feature flag (`xml`/`json`). Unlike `ffi`/`mt`/`net`, these are
  pure and platform-independent; a script that parses XML should not depend on a
  build switch. They compile in unconditionally, like `parse` and the string ops.

## Output representation is newLISP's, verbatim

- **Chosen (XML):** the parse result is a list of nodes. An element is
  `("ELEMENT" name attributes children)` where `attributes` is an association
  list `(("k" "v") …)` and `children` is a list of nodes; text/CDATA/comment
  nodes are `("TEXT" str)` / `("CDATA" str)` / `("COMMENT" str)`. An element with
  no attributes or no children carries an empty list in that slot. This is the
  manual's structure:
  `(xml-parse "<person name='John Doe'>hi</person>")` →
  `(("ELEMENT" "person" (("name" "John Doe")) (("TEXT" "hi"))))`.
- **Chosen (JSON):** objects → association lists `(("key" value) …)`, arrays →
  lists, strings → strings, numbers → integer or float, `true` → `true`, and
  `false`/`null` → the **symbols** `false`/`null` (as newLISP does, so they are
  distinguishable from a string and from `nil`/empty-list).

## `xml-type-tags` and the parse options are interpreter state

- **Chosen:** `xml-type-tags` sets the four type tags ("TEXT"/"CDATA"/"COMMENT"/
  "ELEMENT" by default) that `xml-parse` emits; `nil` in a slot **suppresses** that
  tag (dropping it from the node, SXML-style). The tags live on the interpreter
  (a `RefCell<[Value; 4]>`), persist across `xml-parse` calls, and no-arg
  `(xml-type-tags)` returns the current four — mirroring how regex captures and
  the current context are interpreter state. The `int-options` bit flags are
  honoured: `1` suppress whitespace-only text nodes, `2` suppress empty attribute
  lists, `4` suppress comment nodes, `8` intern element/tag names as symbols,
  `16` emit SXML `(@ …)` attribute groups.
- **Rejected:** threading the tags/options through every recursive call as
  explicit parameters only. The options *are* threaded (they are per-call), but
  the *type tags* are deliberately sticky state, because `xml-type-tags` is a
  separate statement the user runs once before parsing — matching newLISP.

## Errors via a stored last-error, read by `xml-error`/`json-error`

- **Chosen:** on a malformed input each parser returns `nil` and stashes a
  `("message" scan-position)` list on the interpreter; `xml-error`/`json-error`
  return it (or `nil` when the last parse succeeded). A successful parse clears
  it. The scan position is a 0-based byte offset into the source, as newLISP
  reports. This is the same last-error pattern newLISP exposes and keeps the
  happy-path return a clean value rather than a tagged result.

## Scope: parse + errors + type tags; context/callback deferred

- **Chosen:** `xml-parse` (string + options), `xml-type-tags`, `xml-error`,
  `json-parse`, `json-error`.
- **Deferred:** `xml-parse`'s optional 3rd `sym-context` (parse into a context)
  and 4th `func-callback` (a per-node callback) arguments — advanced, unused by
  the WikiBook and coverage targets, and addable later without changing the core.
  XML entity handling covers the five predefined entities (`&lt; &gt; &amp; &quot;
  &apos;`) and numeric character references; full DTD-defined entities stay out of
  scope (newLISP skips DTDs too).

## Consequences

- Unblocks the *Working with XML* chapter and JSON parsing, which the reference
  model (`ref`/`ref-all` over the parsed tree, ADR-0036) is designed to query.
- Two self-contained modules (`src/xml.rs`, `src/json.rs`) with no new
  dependency; the pure `--no-default-features` build gains them for free.
- Serialization (`json`-encode, an XML writer) is **not** in newLISP's core and is
  not added here; only parsing is.
