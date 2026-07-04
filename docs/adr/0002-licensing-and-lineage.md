# Licensing, naming, and newLISP lineage

niiLISP is licensed under **GPLv3**. This is a deliberate choice, not merely a legal obligation: even where niiLISP shares no code with newLISP, we adopt newLISP's copyleft to honour the lineage of the dialect we re-implement and to keep the ecosystem's assets interoperable.

Concretely:

- **`LICENSE.md`** at the repo root states niiLISP's own license (GPLv3) and copyright line (e.g. `© 2026 USAMI Kenta`), and acknowledges newLISP (© Lutz Mueller, GPLv3) as the origin dialect.
- **`COPYING`** at the repo root carries the inherited newLISP copyright/license text. Whenever any newLISP-derived code or asset is bundled, **both `LICENSE.md` and `COPYING` must ship together**, and upstream per-file notices are preserved.
- **`README`** explains the relationship between the two files and the newLISP heritage.

## Naming

The project is named **niiLISP**, deliberately distinct from **newLISP** and **Nuevatec**, which are Lutz Mueller's registered trademarks. A compatible re-implementation may reproduce behaviour but must not trade under the original marks; the distinct name discharges that constraint while the GPLv3 + lineage acknowledgment discharges the moral one.

## Consequences

- Distributing niiLISP (or derivatives) obliges GPLv3 source availability — accepted, by design.
- The license choice is orthogonal to the implementation language: adopting GPLv3 voluntarily means we are free to reference and adapt the newLISP C source without a clean-room constraint (see ADR-0003).
