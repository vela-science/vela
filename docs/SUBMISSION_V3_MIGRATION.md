# Submission v3 retain/delete matrix

Vela 0.977.0 is the final planned pre-1.0 wire cut. The current runtime reads
and writes one closed Submission shape: `vela.submission.v3` in
`application/vnd.vela.submission.v3+json`.

| Surface | Retain | Delete from current head |
| --- | --- | --- |
| Protocol | Submission v3, Verification, attributed Decision, Event, Standing, correction, replay | Submission v2 schema, media type, Rust type, parser, writer, fixture, example, and conformance selection |
| Execution provenance | Artifact, evidence, Method, exact input/output and source-run references | the `vela.execution-binding.v1` object, Submission field, runtime module, aliases, and fallback interpretation |
| Derived reads | deterministic versioned projection with `authority_effect: none` | duplicate dependency-profile interpreters and the noncanonical experiment |
| Flagship state | five v3 Submissions/Proposals, four scoped Verifications, three current accepted Claims, two superseded correction predecessors, seven Artifacts, four Methods, policy and keyset | every pre-genesis Math Submission, Verification, Proposal, Claim record, Artifact record, Decision, Event, authority record, and unreferenced review artifact |
| Recovery | signed Vela 0.976.1 plus Math `rollback/submission-v2-coh-00` at `508b39adac51e6823ea0d666e789a1e016b20227` | runtime compatibility and duplicate historical stores on current branches |

Against Core `origin/main` at `2460521b`, the rename-aware committed diff
removes 34 current-head files totaling 145,586 bytes and deletes 4,220 lines
while adding 1,474. The removed files include the execution-binding module,
thirteen dependency-profile
experiment files and its three interpreters, and all files from the replaced
pre-v3 Math authority fixture. The new fixture contains the exact six-record,
eleven-Event v3 authority chain from Math commit
`f9b28280881472ccb9c4b1b35d8e741745f0bd99` and terminal Repository root
`sha256:45640c5eea54693df444eada6dd1a7c1f5a4b4ef266fddf79cf51d083233ebba`.

The compact Math checkout replaces all 29 old scientific record files with 26
current v3 record files. It reduces Verification Records from six to four,
Artifacts from eight to seven, retained evidence files from eighteen to
sixteen, and Methods from five to four. Across those records, evidence,
Methods, and authority files it is 16,641 bytes smaller.

Current-head spellings of retired Submission v2 or execution-binding wire
values remain only in historical ADR/changelog material, current boundary and
migration prose, and protocol falsifiers. The prose is consumed by operators
making the one-way cut and can disappear after the pre-v3 rollback window
closes. Rust, schema, Python, JavaScript, and Web draft tests retain exact wire
spellings solely to prove that current readers refuse the old payload type, old
schema tag, and removed field rather than silently interpreting them. Their
removal condition is a replacement negative vector that proves the same
fail-closed boundary.

The archived Sidon, Formal Conjectures frontier, quantum-codes frontier, and
Erdős frontier repositories are not current consumers. Their Git history does
not justify any compatibility branch in Core. The only active downstream
contract is the Problems Web draft/export boundary; its frozen public schema,
types, local signer payload type, SQL clean-schema default, and tests move to
v3 together. Production deployment and activity-data mutation are separate
operator actions and are not part of this release.
