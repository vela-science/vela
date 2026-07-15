# Discovery catalogs

This directory contains read-only target and incumbent catalogs used by Vela's
discovery tools. Despite the directory name, these are **not** Vela frontiers:
they do not own `.vela/events`, keys, proposals, or accepted state.

- `<family>/records.json` is a derived per-verifier incumbent catalog.
- `horizonmath/catalog.json` is an imported target catalog.
- `identity-seed/seed.v1.json` is an identity bootstrap fixture.

Canonical scientific state lives in each standalone frontier's Git repository.
These catalogs may point to its accepted event-log root, but they never replace
or authorize that history. Rebuild a catalog from its stated source rather than
editing accepted values by hand.
