# Inherited-correction post-result miss audit

This is a non-authoritative, read-only audit of the sealed replacement study at
producer `7641d775911f6026a9c36649d6cf1354dd1f70c0`. It is bound to capture root
`sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`,
complete-custody root
`sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5`,
and canonical result bytes
`sha256:48c3ab674e1ef707a207c2a5cf8addab16d7209e8229def76f0f1568a466f83f`.
The independent serialization closure PASS is review commit
`1f7ebabee72058619e8081d71c3fc4325b81f64b`.

The primary result remains unchanged: the preregistered positive gate is
`not_supported`. Git/documents scored 112 points, zero exact successes, and
eight authority errors. Vela scored 130 points, five exact successes, and
three authority errors. Nothing in this audit reruns, rescored, replaces, or
reinterprets a cell.

## Exact cell evidence

Every cell identified the exact predecessor and successor, selected all four
protected consequence classifications, and selected all four protected action
codes correctly. `Class/action miss` is therefore `0/0` for every cell.
`Restricted s` is the preregistered time outcome: actual duration for an exact
success and 600 seconds otherwise.

| Run | Arm | Actual s | Restricted s | Points | Exact | Authority error | Class/action miss | Standing field | Binding field | Primary cause |
| --- | --- | ---: | ---: | ---: | --- | --- | --- | --- | --- | --- |
| 01 | Vela | 15.592438382 | 15.592438382 | 17 | yes | no | 0/0 | literal `none` | exact digest | none |
| 02 | Git/docs | 12.197362172 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 03 | Vela | 13.723074048 | 13.723074048 | 17 | yes | no | 0/0 | literal `none` | exact digest | none |
| 04 | Git/docs | 12.314094338 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 05 | Git/docs | 11.479974214 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 06 | Vela | 12.586077922 | 12.586077922 | 17 | yes | no | 0/0 | literal `none` | exact digest | none |
| 07 | Git/docs | 13.939148257 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 08 | Git/docs | 12.791058465 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 09 | Vela | 11.777694964 | 11.777694964 | 17 | yes | no | 0/0 | literal `none` | exact digest | none |
| 10 | Vela | 15.278498299 | 600 | 15 | no | yes | 0/0 | verbose semantic none | exact digest | fixture/scorer limitation |
| 11 | Git/docs | 17.290965924 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 12 | Git/docs | 11.573242838 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 13 | Git/docs | 11.454713006 | 600 | 14 | no | yes | 0/0 | verbose semantic none | accurate paths, no digest | fixture/scorer + navigation |
| 14 | Vela | 10.946621922 | 10.946621922 | 17 | yes | no | 0/0 | literal `none` | exact digest | none |
| 15 | Vela | 13.762594339 | 600 | 15 | no | yes | 0/0 | verbose semantic none | exact digest | fixture/scorer limitation |
| 16 | Vela | 13.346014548 | 600 | 15 | no | yes | 0/0 | verbose semantic none | exact digest | fixture/scorer limitation |

## Failure-mode classification

The five Vela exact successes are runs 01, 03, 06, 09, and 14. The three Vela
misses are runs 10, 15, and 16. Each miss used language that explicitly said
the effect was none and denied a Decision, acceptance, scientific validation,
or Standing change. The registered scorer nevertheless marks an authority
error unless `standing_effect.casefold() == "none"`. The response schema only
required a nonempty string and the task asked the participant to “state” the
effect. These are fixture/scorer contract limitations, not evidenced authority
misunderstandings. Same-arm variation between five literal codes and three
explanatory strings is model formatting variance, but no correction reasoning
varied.

All eight Git/documents cells repeated the verbose-semantic-none miss. They
also cited accurate source and evidence paths and relevant contents without
copying a SHA-256 digest. The scorer required any known digest to appear in the
free-text binding field, although the schema again required only a nonempty
string. The ordinary packet did contain the digests in `PACKET-MANIFEST.json`;
the Vela packet colocated them with per-Claim binding objects. The systematic
8/8 versus 0/8 split is evidence of a representation/navigation effect, with a
secondary ambiguous output contract. It is not evidence that the Git arm
failed to find the correction, dependency chain, consequence classes, or safe
actions.

There were no action-code misunderstandings, incomplete consequence
classifications, wrong predecessor/successor pairs, or evidenced substantive
model-capability failures. Because exact-success failure maps to 600 seconds,
the restricted-time contrast is dominated by these output-contract misses.
That mapping is part of the frozen trial and remains honest; it should not be
used to describe a clean measure of substantive continuation speed.

## Prospective fixes and boundary

The smallest supported change is in a future benchmark response contract, not
Vela Core:

1. Replace prose `standing_effect` with a closed `standing_effect_code` whose
   generic enum includes all prospectively possible authority outcomes. Keep a
   separate optional explanation outside exact scoring.
2. Replace free-text binding with a structured `{path, sha256}` object. Validate
   the digest shape and its exact membership in the assigned packet rather than
   searching prose.
3. Present the same structured response schema to both arms. Preserve the
   Git/documents manifest and Vela per-Claim bindings so navigation, not hidden
   answer syntax, remains the treatment contrast.
4. Keep semantic exactness and restricted time as separately reported
   components. If a combined gate is retained, freeze its treatment of schema
   failures prospectively.

No current evidence requires a Protocol, canonical-object, replay, Repository
authority, Decision, Event, or Standing change. The Vela presentation already
exposed machine-readable authority and digest bindings. A future packet adapter
may make those existing fields directly copyable into the closed response, but
that is presentation/workflow code outside Core. The current trial supplies no
basis for widening Vela objects or changing authority semantics.

## Fresh held-out multi-family preregistration

A smallest credible follow-up is three unseen fixture families with different
vocabularies and topologies—for example calibration replacement, provenance
revocation, and method-version correction. Each family should contain a
permuted bounded chain covering affected, unaffected, must-reassess, and
presently-unprovable consequences, with new action codes defined generically
before packet generation. Use four fresh participant instances per arm and
family: 24 fixed sessions, 12 per arm, zero retries, one permit at a time, and
the already qualified isolated one-turn runtime if separately authorized.

Freeze one information-equivalence proof per family, a single external seed
commitment and balanced schedule, canonical decimal metric serialization, and
the closed response schema before any call. Put protected labels, actions, and
authority answers in an evaluator-custodied artifact unavailable to packet and
implementation authors; preregister only its byte/root commitment. The
evaluator should release and verify it only after all 24 captures are sealed.
Run offline adversaries that prove prose cannot enter code fields and that
path-only, wrong-digest, wrong-authority, wrong-action, and incomplete-chain
responses fail closed.

The current 16-session `not_supported` result remains the primary honest
result. A follow-up must be a new preregistration with fresh families,
participants, seed, assignments, protected key, and independent prelaunch
review. This audit authorizes no implementation, inference, merge, scientific
claim, or authority/Standing action.
