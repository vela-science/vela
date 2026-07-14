# Exit and export drill

Status: executable technical drill. It proves transport and replay; it does not
exercise a real human key or appoint a successor.

Vela's exit path uses standard Git. The canonical event log, proposals,
receipts, public artifact bytes, schemas, and conformance fixtures remain
ordinary repository files. Derived state and caches can be deleted and rebuilt.

## Automated drill

From the parent integration repository:

```bash
./scripts/git-native-smoke.sh examples/sidon-sets
```

The drill creates disposable repositories and fake test keys only. It must:

1. create clone A from the named frontier and land a current Receipt v1
   pending;
2. exercise decision installation only in a disposable fake-key fixture;
3. export safe review material and a packet containing the derived decision
   view;
4. create a standard Git bundle, run `git bundle verify`, and record offered
   refs, prerequisites, object format, bundle SHA-256, and exact Git version;
5. create clone B from that bundle with network access denied;
6. verify signatures and digests, replay the frontier, rebuild Decision Brief,
   retain opaque restricted references without payload, recover accepted-parent
   lineage, and build one child fixture from that root;
7. create an incremental bundle, fetch it into the existing clone, and verify
   the new root; and
8. prove that a recipient without the prerequisite commit gets an explicit Git
   prerequisite failure rather than weakened verification.

The bundle is transport, not authority. A commit, clone, fetch, or successful
bundle verification does not accept a scientific claim.

## Manual independent check

An independent operator should repeat the drill on a clean machine:

```bash
git bundle verify frontier.bundle
git clone frontier.bundle frontier-offline
cd frontier-offline
git fsck --full
vela frontier materialize .
vela reproduce .
vela check . --strict --json
vela packet validate path/to/packet
```

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
- public Receipt v1 records and safe review material;
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

The automated drill does not satisfy POSI's living-will, preservation,
succession, reserve, or patent commitments. A key-holding human and the future
governing body must approve those policies and test an actual successor
transfer before Vela can claim them.
