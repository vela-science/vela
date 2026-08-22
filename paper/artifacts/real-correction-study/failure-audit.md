# Forensic audit of the taxonomy-remap Vela miss

The reviewed 36-cell result remains exactly as scored at commit
`4524c8f776943a267e04e03e9a237ecaed14bc2c`. Git/documents and the neutral
structured wrapper were exact in 12/12 sessions with zero authority errors.
Vela was exact in 11/12 with one authority error. Every arm was
correction-impact complete in 12/12, and the structure,
governance/inheritance, and total gates were all false. The result supports no
lift claim.

## What the miss was

`orderfix-run-25` found the exact taxonomy predecessor and successor, every
downstream classification and action, and every path/digest binding. It also
reported the observed authority effect correctly as `authorized_status_change`.
Its only miss was pairing that with `record_no_status_change` instead of the
registered historical-action code `accept_authorized_status_change`.

The sealed packet created two temporal readings. Its projection said an
independently authorized Decision was already present and had changed Standing.
The task asked for the correction's authority-action code, but also told the
participant not to issue an acceptance action or alter authoritative status.
The schema named only `authority_action_code`; it did not say whether that
meant the historical admitted action or the participant's safe action now, and
it accepted all effect/action combinations.

The narrow supported classification is that the response contract contains a
current-versus-historical ambiguity and this response is consistent with one
current-safe-action reading. The sealed evidence does not establish why this
one response selected that reading. Representation complexity and one-sample
output variance remain unresolved alternatives. The response does not evidence
a general misunderstanding of Vela authority: it correctly identified the
authorized Standing effect. The other three taxonomy/Vela cells returned the
registered pair exactly, but that does not identify the cause of this miss.

## The time result

The session completed in 16.511676507 seconds with no tool calls, timeout,
stderr, process error, or schema-validation error. The apparent 600-second
outlier is the preregistered restricted-time penalty applied to any non-exact
response. It is an honest frozen trial outcome, but it is not a runtime
outlier and should not be used to claim Vela was intrinsically slow on this
cell.

## Prospective changes

Future evaluations should separate an observed historical Decision effect
from the first safe action now, define the temporal meaning of both codes,
reject impossible cross-field combinations, and report correctness separately
from actual or censored time. All arms should use the same closed response
contract. A live product change is eligible only if a separate source-bound
test independently reproduces a product-surface defect on a current Vela read
surface.

Nothing here requires a change to Protocol 1, canonical objects, Repository
authority, Decision, Event, Standing, or replay. The exact bindings and the
facts/hypotheses split are in `failure-audit.json`.
