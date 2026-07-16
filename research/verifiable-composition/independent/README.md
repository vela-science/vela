# Independent handoff packet

Status: blocked until the independent roles are named and a new registration
root binds those declarations.

This packet prepares the blind experiment in ADR 0006. It uses released Vela
`v0.800.22`, the existing ADR 0004 experimental profiles, and the matched
Git/DSSE/in-toto/`science.lock` baseline. It adds no protocol primitive and
grants no authority.

## Fixed scientific case

Producer A receives
`../registration/graph-case.json`. The registered graph is the 11-vertex,
20-edge Grötzsch graph with canonical graph root:

```text
sha256:a7656843120187c8232b042f735aa8fd69b0d0fade1ed8f03067ebd26d623b8e
```

A must produce canonical graph bytes, a four-colouring, a SAT encoding of
three-colourability, an LRAT certificate, and reproducible checker commands.

Producer B receives an accepted parent package after the human steward acts.
B applies the Mycielski construction once. The registered child has 23
vertices, is triangle-free, and has chromatic number five. B's checker must
consume the exact parent bytes or certificate. A fresh independent solution
that ignores the delivered parent does not satisfy the handoff.

## Roles

Each participant completes `participant-declaration.md` before the protocol
team assigns a role.

| Role | Worksheet |
| --- | --- |
| Producer A | `producer-a-run.md` |
| Producer B | `producer-b-run.md` |
| Reader C | `reader-c-run.md` |
| Human steward | `human-steward-run.md` |
| Standards baseline team | `baseline-team-run.md` |

Verifier V1, Verifier V2, and the red team use the role fields in the
registration and retain their own repositories, commands, and output roots.
V1 reproduces A from a clean clone. V2 uses a separate graph and certificate
implementation. The red team receives no authority key.

## Allowed support

The protocol team may answer installation questions already covered by the
public documentation. The team records each contact in the intervention log.

The run fails if a maintainer:

- edits a participant artifact;
- explains the scientific answer or dependency status;
- supplies a bespoke command repair;
- exposes a maintainer-only interface; or
- lets an agent, model, runner, or browser process access a human key.

Transport retry is allowed only when the participant received no usable
output.

## Sequence

1. Name all roles and freeze a new registration root.
2. A completes `producer-a-run.md`.
3. V1 and V2 reproduce A from clean inputs.
4. The human steward reviews the pending proposal.
5. B receives only the accepted package and frozen instructions.
6. B constructs and verifies the substantive child.
7. The red team delivers the registered correction and continuity cases.
8. Reader C and the reference reader classify the same manifests.
9. The baseline team repeats the case with the matched standards profile.
10. The scorer records GO, PIVOT, or NO-GO from the registered measurements.

Fresh sessions controlled by the Vela project remain first-party repetitions.
They receive no independent credit.

## Packet check

Run:

```bash
PYTHONDONTWRITEBYTECODE=1 \
  python3 research/verifiable-composition/check_independent_handoff_packet.py
```

The command verifies every registered file digest, released Vela pin, graph
root, role state, custody rule, and stop condition. It prints the canonical
registration root. The check does not run a participant, contact a network, or
read a key.

## Stop conditions

Stop and retain the run after a key exposure, history rewrite, false strict
pass, semantic maintainer hint, mismatched arm fact set, participant
misclassification, reader disagreement, or scorer nonreproduction.

The protocol team may repair a transport failure before usable output exists.
All other repairs count as evidence against cold use.
