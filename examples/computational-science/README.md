# Computational science: a foreign producer

This bounded example uses exact rational arithmetic to evolve a four-cell
periodic finite-difference heat step for 16 steps. It establishes only that the
discrete total is conserved for these exact inputs and this exact update rule.
It does not establish continuum accuracy or scientific acceptance.

Recompute the retained artifact with Python's standard library:

```bash
python3 examples/computational-science/experiment.py \
  --check examples/computational-science/result.json
```

Then emit the same signed Submission with either independent producer. The seed
is public fixture material and must never be used as a real identity.

```bash
work="$(mktemp -d)"
cp conformance/current-objects/producer.seed.hex "$work/producer.seed.hex"
chmod 600 "$work/producer.seed.hex"

python3 conformance/emitters/python.py submission \
  --draft examples/computational-science/submission-draft.json \
  --seed-file "$work/producer.seed.hex" \
  --actor agent:foreign-compute-example --actor-class agent \
  --declared-at 2026-08-11T00:00:00Z \
  --output "$work/submission.json"

node conformance/readers/javascript/object.mjs "$work/submission.json"
python3 conformance/readers/python/object.py "$work/submission.json"
```

Swap the producer and reader languages to exercise the other direction. Both
readers verify canonical bytes, the DSSE payload type, Ed25519 signature,
content root, derived handle, and signer identity without importing Vela or
Rust. Both reproduce the root and handle frozen in [`flow.json`](flow.json).
Importing the envelope with `vela submit submission.json --repo <repo>`
would create a pending Proposal only; an independent Verification and an
authorized human Decision would still be required to change Standing.
