# Controller/substrate reproduction

This paper-only artifact makes the retained Erdős map-to-Target loop portable
without turning its controller, verifier, or map into scientific authority. It
binds the exact four-commit scientific Repository trace and the exact two-commit
Vela Web implementation trace in shallow Git bundles, then verifies the frozen
map artifacts and authority separation with standard Python and Git.

Run the complete offline check from the Vela source root:

```bash
python3 paper/artifacts/controller-substrate-reproduction/verify.py
```

The command needs no network, Vela binary, authority key, source checkout,
database, Bun runtime, or model. It verifies:

- every manifest-bound byte and both Git bundles;
- exact commits, trees, parents, and the four retained Erdős stages;
- the repository root at each stage from the exact canonical
  `.vela/repository.json` bytes;
- zero accepted-Standing delta after Submission and scoped Verification;
- the later, separate human `review_accept` authority record and its two exact
  Events, followed by one accepted-Standing delta;
- map implementation commits, frozen pre/post projection roots, and the
  post-Decision remap record;
- the retained stale-Target defect: the target-index root changed after the
  Decision, but the first packet root did not advance; and
- the exact materializer's read-only Vela command surface (`status`,
  `review show`, and `next`), with no authority socket or Git push surface.

The bundles are intentionally shallow and carry their boundary commit in
`manifest.json`. The verifier imports the bundle, restores that shallow
boundary, and runs `git fsck --full` before reading any retained state. This
keeps the artifact byte-complete for the exact evaluated trace without copying
unrelated earlier or later repository history.

## Claim ceiling

This artifact supports one bounded internal systems claim: in the retained
trace, an external controller produced a Submission, a separate verifier
recorded a scoped pass, and a read-only map rematerialized state; neither the
controller, Verification, nor map changed accepted Standing. A later exact
human Decision admitted the one bounded Claim, after which replay and remap
exposed the accepted delta and a stale Target packet.

It does not establish scientific correctness beyond the retained Decision's
scope, controller quality, general productivity, external reproduction,
adoption, independence of the original local producer/verifier, or a general
causal effect of Vela. It performs no new Decision, authority, Standing,
source-repository, projection, deployment, or publication action.
