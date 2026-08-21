# Erdős 264 retained real-correction evidence

This non-authoritative paper artifact binds one real, merged source-statement
correction and its retained first-party evidence. In
`google-deepmind/formal-conjectures`, the definition of
`Erdos264.IsIrrationalitySequence` changed from a natural-valued perturbation
to a two-sided integer-valued perturbation. Exactly five theorem declarations
in the corrected source file directly consume that definition. The retained
Erdős Repository records a scoped Verification and attributed Decision for the
correction, followed in the next authority sequence by one accepted exact
`Erdos264.erdos_264.parts.i` proof-repair Claim bound to the corrected
definition.

Run the source-first verifier against Git checkouts containing the exact
objects named in `case.json`:

```bash
python3 paper/artifacts/erdos-264-real-correction/verify.py \
  --source-repo /path/to/formal-conjectures \
  --evidence-repo /path/to/erdos-frontier

python3 -m unittest \
  paper/artifacts/erdos-264-real-correction/test_verify.py
```

The verifier reads every external byte from the retained commits, not from a
moving worktree. It recomputes the two source trees, blobs, files, definition
roots, full-index diff, complete consumer set, Vela object roots, Decision
events, signed authority-record envelopes, Repository roots, and contiguous
event-log roots. It also confirms that the repair artifact retains the
corrected definition and that the accepted repair Claim has no Vela dependency
relation.

## Claim ceiling

This is evidence of a **source dependency**, not five Vela Claim `depends`
edges. The retained history does not establish a support diamond. One accepted
dependent repair does not establish complete propagation, general scientific
lift, or lower continuation cost. The artifact does not prove Erdős problem
264, prove the other four consumer theorems, establish external independence or
adoption, or make a new Decision. It has no authority and changes no Standing.
