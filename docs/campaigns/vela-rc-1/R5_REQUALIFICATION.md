# VELA-RC-1 R5 independent product requalification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
PASS WITH DEPLOYMENT/REPROJECTION ACTIONS
```

This cross-repository receipt preserves the earlier `HOLD — PRODUCT
SEMANTICS` and binds the independently audited Vela Web repair. It qualifies
source behavior only. It does not qualify the currently deployed legacy
projection and authorizes no deployment, activation, release, push, or public
state change.

## Exact source and audit

| Field | Value |
| --- | --- |
| Vela Web repair commit | `fd2bb321e2331b546ad5f94705707af9d087ddaa` |
| Vela Web repair tree | `1f33b0f65b7fa17be2cadcfc2c3a942ff4acffb4` |
| Independent audit commit | `6c8e7550bf6643f73fd9dfad0d1184e5aa631b1d` |
| Independent audit tree | `f76bcad230494913321a4577a62387179fb808ea` |
| Source audit record | `docs/history/audits/vela-rc-1-r5-second-independent-requalification.md` in Vela Web |

The independent audit reran both prior counterexamples. A manifest that
self-asserts repaired provenance while naming the unadmitted Vela `0.977.3`
generator is refused as `foreign_manifest`. An unknown Repository carrying an
arbitrary syntactically valid authority root is also refused. Missing,
mismatched, duplicate, and unregistered authority selections fail admission.

The product label is actor-neutral: an accepted agent-class Decision is shown
under `Authorized Decisions`, and the regression refuses the former `Human
authority` wording.

The complete source-local suite passed: 61 root scripts, 4 brand tests, 32 UI
tests, 378 projection-data tests with 36 credential-dependent skips, 47
activity-data tests, 628 Problems tests, 3 www tests, and 9 manifest tests,
plus lint, typecheck, brand, design-system, boundary, activity, roots, and
manifest gates.

## Required deployment actions

The deployed projection remains legacy and must not present current strict
integrity. Before activation, a separately authorized release/deployment lane
must:

1. package and independently qualify the repaired Core release bytes;
2. admit their exact tag, commit, platform digests, and governed-read
   attestation in the Vela Web release record;
3. regenerate the projection with registered authority roots;
4. run strict live projection, stored-root, production-build, public-output,
   and runtime-route checks with the read-only database credential; and
5. obtain explicit deployment authorization.

No credential was available during source requalification, so no live reader,
production runtime, activation, or deployment claim is made.
