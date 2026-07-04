# Derivation posture: reference-guided fresh implementation, not strict clean-room

niiLISP is written **from a fresh design aimed at the best result**, while **freely consulting and, where useful, adapting the newLISP C source** vendored under `references/newlisp`. We do **not** impose a clean-room separation.

This is possible because niiLISP adopts GPLv3 deliberately (ADR-0002): with no license barrier to avoid, there is nothing to gain from clean-room isolation, and the newLISP source is the most authoritative description of the behaviour we must match.

Compatibility (the top priority, ADR-0001) is measured against newLISP's own behaviour and its `qa-specific-tests` suite, used as the acceptance oracle rather than as code to copy.

## Consequences

- Where newLISP's C is the clearest specification of a corner (e.g. ORO deallocation timing, `import` argument marshalling), we may mirror its logic directly; where a modern design is cleaner, we diverge.
- The implementation language is therefore genuinely unconstrained beyond "must be able to drive libffi for `import`" (ADR-0001) — the language decision is made on its own merits, not forced by the source we consult.
