# Erdős 264 real correction evidence

This capsule verifies a merged semantic correction in Formal Conjectures. The
definition of an Erdős 264 irrationality sequence changes from bounded natural
perturbations to bounded integer perturbations. The verifier binds both source
commits, trees, blobs, files, the canonical full-index diff, the corrected
definition still present at current upstream head, the five direct theorem
consumers, and the three exact Erdős Frontier Claims that survive, require
supersession, or require a separate compatibility audit.

Run:

```bash
python3 paper/artifacts/erdos-264/verify_source_transition.py \
  --source-repo /Users/williamblair/personal/formal-conjectures \
  --proof-repo /path/to/plby/lean-proofs \
  --frontier /Users/williamblair/personal/erdos-frontier \
  --artifact /Users/williamblair/personal/erdos-frontier/artifacts/fidelity/erdos-264-source-transition.v1.json
```

The full-index diff is deliberate. Git's ordinary patch output abbreviates
blob IDs according to the local object database, so its digest is not a stable
cross-checkout identity.

All five direct theorem declarations remain present at the merged successor,
but their statements inherit the corrected definition and their bodies remain
`sorry`. Presence is not proof. The retained hosted proof is also checked at an
exact upstream commit: it still defines bounded natural perturbations and is
therefore not rebound to the corrected integer-perturbation definition. That
does not invalidate the proof under its own local definition; it creates an
exact repair obligation. This evidence does not prove Erdős problem 264 or
change scientific Standing. A Vela Submission and Verification remain evidence
until an attributed human Decision.

[`decision-packet.v1.json`](decision-packet.v1.json) freezes the one current,
protocol-ready human choice and its exact accept/reject Standing roots. It is
read-only evidence: it does not recommend or perform a Decision. Its successor
section binds the native Lean obligation that may become available only after
acceptance.
