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
  --frontier /Users/williamblair/personal/erdos-frontier \
  --artifact /Users/williamblair/personal/erdos-frontier/artifacts/fidelity/erdos-264-source-transition.v1.json
```

The full-index diff is deliberate. Git's ordinary patch output abbreviates
blob IDs according to the local object database, so its digest is not a stable
cross-checkout identity.

All five direct theorem declarations remain present at the merged successor,
but their statements inherit the corrected definition and their bodies remain
`sorry`. Presence is not proof. This evidence does not prove Erdős problem 264,
validate the hosted partial proof, or change scientific Standing. A Vela
Submission and Verification remain evidence until an attributed human Decision.
