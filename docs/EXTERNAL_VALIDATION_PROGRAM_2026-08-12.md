# Vela external-validation and reversible-publication program, 2026-08-12

Status: **active; reversible publication prerequisites are proven, but the
external-validation program is not complete**.

This ledger is the current handoff for evidence that necessarily crosses the
Vela, Vela Web, and Mathematics repository boundaries. It does not change the
protocol or reopen the completed core architecture. A reconstruction, a green
check, a signed activity event, or a correct reader response is not a scientific
Decision or independent adoption.

## Reversible-publication prerequisites

| Requirement | Current evidence | Status |
| --- | --- | --- |
| Vela Web reconstruction | `vela-science/vela-web` commit `c3a97ec39ee6d2401eddf8512b90d8b5125accc6` retains `packages/observatory-data/evidence/math-atlas/clean-room/2026-08-12-e77526b5.json`. Two empty PostgreSQL reconstructions were byte-identical, the SELECT-only reader could read every projection table and write none, and production matched the exact table, source-registry, source-Repository, manifest-schema, and Vela-version roots. The artifact root is `sha256:ad81a61cc4a550dd3c47ea10376bb8e18a755565eacc56bf53534b25c37f7e0b`. Cross-platform release-root equality is explicitly not claimed. | Proven for production release `sha256:e77526b5fd57c77bb0e6774392ebabf42c2c797873ef2a587b2eb17aeac3aa3c`. |
| Isolated rollback before publication | The same Web commit retains `2026-08-12-isolated-rollback.json`, a live-selector exercise on a disposable Neon child: `A e77526b5… -> B a7db268f… -> A e77526b5…`. Reader verification passed at both roots; first-live timestamps and all three retained releases were preserved; production before and after was identical; the child was deleted and confirmed absent. | Proven for the exact roots and selector code named by the artifact. |
| StandingBench branch custody | GitHub and Codeberg expose only Mathematics `main` at `a8f607af2b0274c87186791900e7aeebd1382e5a`. The three previously public StandingBench histories are absent from hosted branch and tag refs. The private Vela Web release `standingbench-compromised-forensic-archive-e1930f6985034278cb12a2a738a9d43f77c0b3b39bfcf6362d5eb03ea9596235` retains the complete histories as forensic-only custody: bundle SHA-256 `e1930f6985034278cb12a2a738a9d43f77c0b3b39bfcf6362d5eb03ea9596235`, descriptor SHA-256 `a58c75f8dba3cfbb292681ae593ed2aed0c004db3987cbb9df6dfac4e31532c9`, `authority_effect: none`. The exposed benchmark material remains compromised; private custody does not restore benchmark secrecy. | Contained and retained for forensics; never valid as held-out evaluation evidence. |
| Source-rights binding | Mathematics `evidence/erdos-321/terminal-variants/source-lock.v0.1.json` at `a8f607af…` binds content root `sha256:128edd2b0b58597ca05c7c0049ab20c10c9e6ef4e06c2c3ff8371752123a103b`. It records `rights_class: NOASSERTION`, downstream redistribution rights `not_established`, and `handling: reference_only`. The evidence unit copies no Star Fleet theorem bytes and treats pinned hosting permission as neither a portable license nor a redistribution grant. | Resolved fail closed. A future license grant may widen handling in a new rooted record; the current record must not infer one. |

The live Observatory manifest observed on 2026-08-12 still serves the exact
`e77526b5…` production root above, Mathematics commit `a8f607af…`, and the
versioned SELECT-only reader. Continuous hosted reconstruction is temporarily
unproven after the retained artifact: GitHub Actions runs `31617813389` and
`31616962487` created no runner steps because account billing or the Actions
spending limit blocked the jobs before execution. This is an external runner
availability failure, not a successful reconstruction and not a code failure.
The hosted freshness gate remains red until a later run actually executes and
passes.

## External evidence tracks

| Track | Evidence in hand | What is still required |
| --- | --- | --- |
| Pre-registered cold reader | Mathematics freezes plan root `sha256:4067903b9ea8f1fa9d6d0b536846190653a3e029352c6f24eb5540592f570678`, reader-instrument root `sha256:db06c21a6a0d4f24c91dc59903dad0245326edd2311dbdeb3d88ed39640067e7`, and participant-packet root `sha256:35f6c06a1bf8f7abc46caea502f9ccd45db6a569da6856d04f03a2c386266bbe`. Status is `preregistered_not_run`; every measurement is `not_measured`. | Enroll eligible humans under the frozen custody and timing protocol. The target is 12 eligible two-period completers, subject to the preregistered cutoff and under-recruitment rule. Do not insert model or operator observations into the human estimator. |
| External workbench | The stock Buzz compatibility run at Mathematics `b1d1ff4d9a786b5ccf8c9447173f41ca9514fd79` has aggregate evidence root `sha256:0271f0d9d385b2c834ccf461a8e004165ad579e6b12f2ab2f2f44e824e68f625`. It proves transport, storage, and byte-identical readback of an operator-authored packet and result through unmodified Buzz. Buzz performed no scientific reasoning and produced no candidate. | For external-producer evidence, a separately operated workbench must author or emit a bounded conformant candidate and return its rooted result without gaining authority. The same-operator transport run is compatibility evidence, not independent production or adoption. |
| Correction cascade | Current Mathematics has one accepted Claim, root `sha256:d5d77e7d96e390e0bf692d0abd44367eb06a0c6a61534e1c6654962d6c644776`, and its retained Claim record has `relations: []`. Synthetic conformance vectors and historical same-operator usability exercises do not create a live accepted dependency. | A producer must declare a real Claim dependency before acceptance. A later correction or withdrawal must then produce exact affected and unaffected state, followed by attributed human Decisions and a preregistered comparison with the correct repair. Do not retrofit a dependency solely to satisfy the experiment. |
| Independent adoption | No separately maintained scientific consumer, independent authority, or externally governed Repository currently adopts the Dossier, translation profile, or Vela Standing. A private mirror, a second repository under the same operator, and an automated reader are not independence. | A maintained external consumer must reproduce the applicable roots and use the output. Independent authority additionally requires separate governance, key custody, trust root, mandate, and capacity to disagree. |

## Next admissible operations

1. Restore GitHub Actions runner availability, then require one actually executed
   refresh and reconstruction success before treating hosted freshness as green.
2. Recruit and enroll readers without changing the frozen cold-reader
   instrument, scoring key, assignment schedule, target, or cutoff.
3. Offer the rooted workbench packet to a separately operated workbench. Keep
   its activity outside Vela authority until an ordinary Submission and scoped
   Verification are produced.
4. Select a scientific case only after a genuine producer-declared dependency
   exists. Preserve an empty result rather than manufacturing a cascade.
5. Record independent use only when the external maintainer, roots, purpose,
   and observed use are attributable. Do not translate interest, a clone, or a
   passing parser into adoption.

Completion requires observed evidence in all four tracks. The program remains
open while any track is `not_measured`, same-operator only, synthetic, or
unattributed.
