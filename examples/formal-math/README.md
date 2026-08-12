# Formal mathematics: terminal evidence without invented acceptance

The merged Math evidence compares a pinned terminal theorem for Erdős 321 with
two retained fixed variants. It roots exact source objects and an executable
cold-reader protocol. It does **not** prove implication or equivalence, and its
`authority_effect` is `none`.

Clone the complete Repository at the pinned merge and replay its actual state:

```bash
gh auth status
gh repo clone vela-science/math math
git -C math checkout 5be513bd0ce2243b59268d9b185da18497505067
vela replay math --json
vela claims math --json
```

The canonical Math source is private and requires an authorized GitHub account
for acquisition. Replay itself is local and requires no repository-authority
key.

Expected replay facts are frozen in [`flow.json`](flow.json): Repository root
`sha256:db4d435…e90ebdc` and one accepted Claim. The terminal-evidence merge
changed neither `.vela/` nor `records/`; it did not add accepted state.

Inspect the bounded comparison and its nonclaims:

```bash
jq '{authority_effect, comparison, does_not_establish, next_obligation}' \
  math/evidence/erdos-321/terminal-variants/comparison.v0.1.json
```

For the source-dependent reconstruction, acquire complete local object stores
for the exact pinned `lean-proofs`, `mathlib4`, and `PrimeNumberTheoremAnd`
commits, then run the offline builder exactly as the source-owning Repository
documents:

```bash
python3 -B math/evidence/erdos-321/terminal-variants/build.py \
  --lean-proofs-repo ../lean-proofs \
  --mathlib-repo ../mathlib4 \
  --pnt-repo ../PrimeNumberTheoremAnd --check
```

That check verifies the source-local evidence bundle. It is not a Vela
Verification Record and cannot change Standing. A future producer may cite the
exact bundle in a Submission, but the ordinary independent Verification and
human Decision boundary still applies.
