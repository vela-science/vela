# Independent held-out inherited-correction prelaunch review

## Verdict

**BLOCKED**, bound to producer commit
`b1528898e5877a5dd0c863a28db71c9fd5623f60`, tree
`608fe2ea360fc48c6015f479c53a19b99c6a73d5`, whose sole parent is
`de13073ff8f3a9f2958f8c93c848205c533ddb1e`.

One frozen runtime-provenance binding is incorrect. The artifact binds the
Draft-2020 image
`sha256:1dee2374077c83e3dbdb2e09d32ef4fa3a414d200b800839857353e13d3c4e09`
and runtime-source root
`sha256:398f798daf4b2ebd86a878021025adbc073155e13d9123b140da2bc8fcb32b8a`
to independent review `2ebf1ad8cb0f5d16b7bcee8e5510f3aed5dc1395`.
That review predates the Draft-2020 repair and qualifies the earlier canary-03
runtime lineage, whose image was
`sha256:6274d83356076640d6e4bc810b97d37ac2d1b5ab02546dd7c2ebed16f915b547`.
The exact `1dee...` image and `398f...` source root were instead independently
qualified by review `3c0c8cfa050b30a5d19c9e7e623fc549ac18264b` of producer
`2fc59d5f57e45298f833e65f123ac9eafea2810b`. No held-out byte binds that
qualifying review.

This fails `G08_deterministic_custody`: the frozen claim that this exact
runtime is independently qualified does not resolve to the review that
qualified its bytes. It also prevents complete support for the exact frozen
cold-successor environment under `G06_cold_successor_protocol`.

No permit may be released and no participant/provider call is authorized.
Status remains 0/36, `not_run`; all 36 permits remain held and unconsumed, and
the protected adjudication remains absent and pending.

## Minimal reproduction

1. Read `runtime-binding.json` or the nested runtime object in
   `participant-configuration.json`: both name review `2ebf1ad8...` while
   binding image `1dee237...` and source root `398f798...`.
2. Inspect independent review `2ebf1ad8...`: it is the narrow F03/G08 review
   of corrective producer `4c7bd6a8...` and states that no runtime or canary
   byte changed from the prior canary-03 result. That result binds image
   `6274d833...`.
3. Inspect independent review `3c0c8cfa...`: it explicitly qualifies image
   `1dee237...`, source root `398f798...`, and the Ajv Draft-2020 validator
   repair at producer `2fc59d5f...`.
4. Search the exact held-out artifact for `3c0c8cfa...`: there is no match.

The minimal prospective repair is to bind the exact Draft-2020 qualification
review (or a structured base-runtime plus validator-repair review lineage) in
the runtime and participant-configuration objects, regenerate every dependent
root and manifest entry, retain 0/36 and the pending protected key, and submit
the immutable repaired bytes for re-review. No packet, task, answer, gate, or
scientific fixture change is needed.

## Otherwise passing evidence

The pushed ref reconstructed exactly at the handed-off commit, tree, and
parent. Its diff contains 215 added files and 11,920 added lines, all under
`paper/artifacts/inherited-correction-held-out/`; live `origin/main` was
independently unchanged at `4685462c44b1f073870f31025ae73d1d8770ce73`.

Independent recomputation matched every disclosed byte digest and canonical
root, including:

- registration root `sha256:b179fb4090871003d2d632e21a639420d8d0df8a79b529f372bc28e525355a42`;
- assignment root `sha256:65709707cd75e2de43e3c421a8c0ef1d6439bca8f8f5af32d125cbfd512f0cc1`;
- shared configuration root `sha256:3d2e281d23aa5160ac2f83bf4dc9e69698211d26bd0843ccfaf26e5c5055fc84`;
- input-equivalence root `sha256:12904756aa4683934eb925ae856d6afd50897dc1d855f3b55ce4e51ce6391bc1`;
- 36-permit set root `sha256:3eeea5bc0152616691149519cf5bf47f20f2ee88fd66836e011a6342b82d6794`;
- prelaunch root `sha256:015eb785c136316cac16ed206822cf87ebef7484c2b065753822d36faa002d4b`;
- 214-entry artifact root `sha256:26e3f8256db983c9b37fb580786a3ed766e11c4bec377b2fa13af2bc7850581e`;
  and
- held result bytes `sha256:73c682355f7e5c03362fe256bb33eb8273ddeff4a3db80f41e3e68746fac3797`.

The assignment contains 36 unique run IDs and 36 unique participant IDs, with
exactly four cells for every family/arm pair and 12 cells per arm. Within each
family, all arms carry the same atomic-fact root and exact source/evidence
bytes. Independent prompt ratios are 1.1923, 1.1921, and 1.1965, each below
the frozen 1.20 ceiling. The neutral packets contain none of the six forbidden
Vela-specific vocabulary terms. No answer map or protected adjudication bytes
are present.

The response contract is closed and exact-path/digest-bound. The scorer keeps
all 36 cells in the denominator, charges non-results 600 seconds, snapshots
capture-bound run/response bytes before key access, and uses canonical Decimal
half-even serialization. The structure, governance/inheritance, and preserved
total gates implement the registered additive estimands; aggregate governance
equality cannot pass without a strict increment.

The following independent checks passed from the detached producer checkout:

- Ruff 0.12.11 format and check on all three Python files;
- held-out build verification, held prelaunch verification, and all 18 tests
  under CPython 3.10.8, 3.11.2, 3.13.13, and 3.14.4;
- isolated CPython 3.10 regeneration of all 215 files, byte-identical with
  manifest SHA-256 `0bfd55db426119184bb7b90e7197c91206f4a50339b189c5e889219df25e3a7e`;
- the prior inherited-correction verifier and all 16 tests; and
- `git diff --check`.

The pending evaluator-key amendment and any separately recorded execution
authorization remain future gates. This review performed no inference,
scoring, key access, permit mutation, merge, Core or Protocol change,
authority action, Decision, or Standing mutation. It supports no positive
Vela, scientific, adoption, or productivity claim.
