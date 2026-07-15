# HorizonMath — verifier-attackable target catalog

`catalog.json` stages the **verifier-attackable subset** of the HorizonMath
benchmark: open/research mathematics problems that map to a frozen Vela verifier
(a `vela foundry campaign` kind) and carry a real incumbent (the value-to-beat). It is
the target surface the foundry attacks, the same role the per-family
`frontiers/<family>/records.json` catalogs play, generalized across verifier families.

Each problem records `{id, domain, level, statement, verifier_kind, params,
incumbent{value, direction, basis}, status, source}`. `source` is the honest
provenance: `horizonmath` for the difference-triangle flagship (DTS(7,5),
value-to-beat 112, from the Constellate ingestion memo), `constructions_board`
/ `oeis` for the construction families whose incumbents come from the recorded
construction catalog and OEIS. The frontier carries the source metadata needed
to audit those incumbents; it does not depend on a bundled web application.

This is **not** the full 101-problem corpus. The corpus-level facts (101
problems, 91 unsolved, 8 domains, the level counts) are recorded under `corpus`;
the problems with no frozen Vela verifier are deferred, not fabricated.

## Use

The catalog is read-only discovery input. Pure adapters may project or rank it
without creating frontier state. A producer that wants to contribute a result
attaches the relevant catalog row and evidence to Receipt v1, then uses
`vela land`. Only the signed policy or human signing ceremony can admit it.
