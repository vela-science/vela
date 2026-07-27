# Exit and export drill

Status: manual portability checklist. It checks standard Git transport and
local replay; it does not exercise a real human key, certify a release, or
appoint a successor.

Vela's exit path uses standard Git. The canonical event log, proposals,
receipts, public artifact bytes, schemas, and conformance fixtures remain
ordinary repository files. Derived state and caches can be deleted and rebuilt.

## Independent check

An independent operator can check a frontier on a clean machine with ordinary
Git and the pinned Vela binary:

```bash
git bundle verify frontier.bundle
git clone frontier.bundle frontier-offline
cd frontier-offline
git fsck --full
vela frontier materialize .
vela reproduce .
vela check . --strict --json
```

Run the same commands twice and compare the event-log and snapshot roots. For
an incremental transfer, create a second Git bundle from the newer ref, fetch
it into the existing clone, and repeat the checks. A missing prerequisite must
remain an explicit Git failure; do not substitute an unverified snapshot.

The bundle is transport, not authority. A commit, clone, fetch, or successful
bundle verification does not accept a scientific claim.

Record:

- Vela binary version and digest;
- Git version and object format;
- bundle digest, offered refs, and prerequisites;
- source commit and event-log root;
- replayed snapshot root and accepted-parent lineage;
- missing public witnesses or unavailable restricted references;
- every network attempt and whether it failed closed; and
- the child result's exact parent root.

## What leaves

- public canonical events and proposals;
- public current Submissions, Registration Records, Verification Records, and
  safe review material;
- historical Receipt v1 records retained for replay;
- public artifacts whose recorded licenses permit redistribution;
- schemas, conformance vectors, documentation, and release checksums; and
- derived packets clearly labelled as views.

## What does not leave automatically

- human private keys or signing devices;
- credentials, SSH agents, tokens, local configuration, or caches;
- restricted payloads, openings, equality digests, private locations, or
  machine-local paths; and
- institutional authority, consent, legal rights, or a claim that the new
  operator is an approved successor.

Restricted data transfer follows [POSI_SELF_ASSESSMENT.md](POSI_SELF_ASSESSMENT.md).
If no lawful successor custodian is documented, transfer only the opaque public
reference and report the payload unavailable.

## Failure handling

- Missing Git prerequisites: obtain the named prerequisite bundle or commit;
  never replace the missing history with an unverified snapshot.
- Missing public witness: report the exact content root and availability state;
  replay cannot be upgraded to pass.
- Restricted payload unavailable: retain the opaque reference and caveat; do
  not infer or disclose an equality digest.
- Signature or event-root mismatch: stop. Do not materialize or publish the
  candidate state.
- Derived packet mismatch: regenerate it from canonical events. Never edit the
  packet into agreement.

## Institutional remainder

This technical checklist does not satisfy POSI's living-will, preservation,
succession, reserve, or patent commitments. A key-holding human and the future
governing body must approve those policies and test an actual successor
transfer before Vela can claim them.
