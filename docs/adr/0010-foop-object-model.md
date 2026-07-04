# FOOP object model: a convention over tagged lists, no dedicated object type

niiLISP reproduces newLISP's FOOP (CONTEXT.md: FOOP) faithfully, because compatibility (ADR-0001) fixes the model. The one genuine Rust choice — whether objects get a dedicated value type — is decided **against**: an object is a plain `Value::List` whose head is its class Context's symbol. FOOP is a convention, not a type.

## Why tagged lists, not a dedicated type

newLISP objects *are* lists: a constructor returns `(list complex r i)`, methods read fields with `(self 1)`, and ordinary list operations (`cons`, `map`, indexing) apply to objects directly. A dedicated object type would cache the class tag and speed dispatch, but would break every program that treats an object as the list it is — a direct compatibility loss. FOOP is a convention in newLISP too.

## Mechanisms (all reproduced, all riding existing decisions)

- **Object** = tagged list, head = class Context symbol. Uses the Vec-backed list (ADR-0005); no new representation.
- **Default functor** (`Ctx:Ctx`): applying a Context value as a function invokes its default functor; in FOOP that constructs the object. The evaluator special-cases "operator evaluates to a Context".
- **Method** = a function in the class Context.
- **`self`** = a dynamically-bound symbol set to the target object around a method call — it rides the save/restore rebinding stack from ADR-0006, including correct behaviour under nested method calls.
- **Colon dispatch** `(:method obj …)`: resolve `class-tag(obj):method` at runtime. The reader recognises `:method`; the evaluator dispatches on `obj`'s head tag.

## Consequences

- No `Value::Object` variant; object-ness is structural (head is a Context symbol).
- If method dispatch becomes a bottleneck, accelerate it via the ADR-0007 derived cache (memoise `head-tag → resolved method`) rather than by introducing a type — keeping the tagged-list identity intact.
- `qa-foop` (ADR-0009) is the acceptance test for this model, including its nested-`self` cases.
