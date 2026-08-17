# Formal mathematics: correction-aware current state

The current Math evidence binds exact source occurrences for Erdős 321, records
a corrected candidate-answer Claim, and keeps the terminal-to-fixed-variant
relationship unresolved. It does **not** prove implication, equivalence,
resolution, or optimality, and its `authority_effect` is `none`.

Clone the complete Repository at the pinned merge and replay its actual state:

```bash
git clone https://github.com/vela-science/math.git math
git -C math checkout f9b28280881472ccb9c4b1b35d8e741745f0bd99
vela replay math --json
vela claims math --json
```

The canonical Math source is public. Acquisition requires no account, and
replay remains local and requires no repository-authority key.

Expected replay facts are frozen in [`flow.json`](flow.json): Repository root
`sha256:45640c5e…233ebba`, three current accepted Claims, and two retained
superseded predecessors. Both corrections entered Standing only through their
recorded authorized Decisions.

Inspect the bounded comparison and its nonclaims:

```bash
jq '{authority_effect, occurrence_resolution, successor, limitations}' \
  math/evidence/current/erdos-321/correction-chain.v1.json
```

The retained packet names the exact source revisions and raw byte roots. Its
scoped Verification Records report occurrence fidelity and correction-chain
fidelity. They do not accept the Claim; the separately attributed Repository
Decision is the only operation that changed Standing.
