# First-party correction-handoff rehearsal

Status: complete internal engineering rehearsal; no human, independent,
external, scientific, causal, or protocol-promotion credit.

## Result

Registration
`sha256:c9afabdac6ec868f286583a995e27cdad2055c95b655bad6f91cdbcc30d11482`
completed with canonical result root
`sha256:6c6a443c9a04a2901f98e54247a307a8e62fba1b7933d5ec140f57a39554dff3`.
The eligible run used 14 commands in 8,456 ms. It recorded zero repairs,
semantic interventions, network requests, key reads, authority attempts,
accepted-state claims, and historical rewrites.

One pre-eligible run failed on relative artifact paths. A later local pass
could not reproduce outside the source checkout for the same reason. Both
roots and failure classes remain in the registration history. The final
controller accepts an arbitrary output directory.

## Scientific artifacts

Producer A generated the registered 11-vertex, 20-edge Grötzsch graph,
a valid four-colouring, a three-colourability CNF, and an LRAT certificate.
Two separately implemented graph verifiers agreed. CaDiCaL generated the
transient DRAT proof, `drat-trim` converted it, and `lrat-check` accepted the
retained 4,354-byte LRAT certificate.

The child checker consumed parent root
`sha256:a7656843120187c8232b042f735aa8fd69b0d0fade1ed8f03067ebd26d623b8e`.
It generated the exact 23-vertex Mycielski child, a valid five-colouring, a
four-colourability CNF, and a 252,965-byte LRAT certificate. Both graph
verifiers agreed that the child is triangle-free and has chromatic number
five. Parent substitution fails closed.

The eight scientific artifacts have manifest root
`sha256:fb629316afe2f097db3f78ccc397be815890cd84a96c900fb18585a91c679379`.
A second run under `/tmp` produced the same eight byte roots and the same
deferred-route, accepted-delta, correction, standards, and authority
invariants. Its result root
`sha256:8ae69df6830b37df8984a94063e38504f8afc0e70168070cb4dea1fc05a9db1e`
differs because the agent key, timestamps, Receipt and proposal IDs, output
paths, and wall time are run-specific.

## Handoff and correction behavior

The key-free handoff package is explicitly `pending_review`,
`hard_dependency_usable: false`, and `accepted_state_claim: false`. Released
Vela `v0.800.22` produced Receipt
`sha256:c3c32d6ac91e9929132e1c4934cfc0752d0ea9f231718184311b51674546f005`,
proposal `vpr_3a64dd6359c03f35`, route `deferred`, and accepted-event delta zero.
Its strict check passed. The child
mechanics use only the labeled internal fixture-authority profile; they do not
claim a human decision.

All 54 registered fact-manifest vectors agree between the reference resolver
and Reader C:

| Status | Cases |
| --- | ---: |
| `satisfied` | 2 |
| `warning` | 2 |
| `review_required` | 4 |
| `blocked` | 2 |
| `stale` | 1 |
| `forked` | 1 |
| `unresolvable` | 42 |

Every case leaves child truth `not_assessed`. Offline same, descendant, stale,
and fork delivery passes its focused check. The matched
Git/DSSE/in-toto/`science.lock` wrapper passes 13 of 13 mutation vectors.

## Effort and gap verdict

The new rehearsal code is 914 source lines across two graph verifiers, the
controller, and focused checks. Certificate generation and checking took
81 ms. The released-Vela pending transaction took 672 ms. The Vela-profile
graph, fact-manifest, and offline-continuity checks took 4,662 ms; the
standards-wrapper check took 2,097 ms. These are local
execution measurements, not independent integration time, human minutes, or a
30-percent usability comparison.

No missing protocol invariant reproduced:

- ADR 0007: full finding and event roots already disambiguated every registered
  collision and substitution case.
- ADR 0008: exact Git ancestry plus event-prefix inspection classified
  descendant, rollback, and fork cases offline.
- ADR 0009: the existing experimental observation and fact-manifest profile
  classified corrections and verifier withdrawals without mutating the child.

ADRs 0007 through 0009 therefore remain Proposed and unimplemented. The result
cannot select GO, PIVOT, or NO-GO under ADR 0006 because it has no outside
participants or human ceremony.
