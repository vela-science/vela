# Atlas × Vela consumption sketch

A 150-line Python file (stdlib only, no Vela dependency) that demonstrates
how a downstream researcher-intelligence platform consumes a Vela frontier:

1. Fetch the registry manifest from the public hub
2. Fetch the frontier file from the manifest's `network_locator`
3. Render a researcher-card suitable for embedding (markdown or terminal)

The point is to show that "Vela frontier as data source" is a one-import,
one-fetch, one-parse contract. Atlas at Episteme, an internal lab dashboard,
an agent context panel — anywhere downstream tools surface third-party
scientific frontiers — can consume Vela this directly.

## Run it

```bash
# List what's on the hub
python3 consume_vela.py --list

# Render a researcher card for Will's frontier
python3 consume_vela.py vfr_773f6e60b19b438f

# Same, as markdown (suitable for embedding in a Slack message,
# an Obsidian note, or an Atlas timeline card)
python3 consume_vela.py vfr_773f6e60b19b438f --markdown

# Render the BBB Flagship card
python3 consume_vela.py vfr_093f7f15b6c79386
```

## What it surfaces

For each frontier:
- name, owner, vfr_id, snapshot hash, publish date
- active-findings count (excluding `flags.superseded` ones)
- source registry size (v0.13 inline materialization)
- cross-frontier dependency count (v0.8)
- top 5 active findings with type + confidence
- the supersedes chain map (old vf → new vf) so consumers can walk
  forward to the current version of any cited finding

## What it does NOT do

- **Doesn't re-derive the snapshot hash**. A production consumer should —
  see `scripts/cross_impl_conformance.py` for the full canonical-JSON
  re-derivation discipline. This sketch trusts the hub-side hash.
- **Doesn't follow the locator's signature back to the registry's
  `owner_pubkey`**. A production consumer should verify the manifest's
  Ed25519 signature against `owner_pubkey` before trusting the bytes.
- **Doesn't walk cross-frontier deps**. A production consumer building
  a transitive view should call `vela registry pull --transitive`.

The goal is to communicate the consumption shape, not to be a complete
verifier. Replace this with proper Vela tooling when integrating beyond
demo.

## Why this matters for the broader vision

Atlas (or any Episteme deliverable) accelerates research by turning
fragmented sources into actionable map structure. Vela is the substrate
that lets that map structure persist, be corrected, and be inherited
across institutions. The integration is one Python file because the
substrate doctrine is "dumb signed transport, clients verify on read"
— the simpler the consumption surface, the more places it lands.
