# Vela actor tiers (v0.6+)

Trust tiers permit a registered actor to apply certain low-risk
proposal kinds in one signed call instead of the propose→pending→accept
two-step. Tier is **opt-in** at registration time and **server-enforced**
at write time.

## The doctrine

- **Tiers permit review-context kinds only.** Annotations (`finding.note`,
  `finding.caveated`) are review context — they accrete commentary
  without changing finding state. State-changing kinds (`finding.review`,
  `finding.retract`, `finding.confidence_revise`) always queue for human
  review regardless of actor tier.
- **Auto-apply is opt-in by the caller, not transparent.** The server
  does not silently apply on `propose_note` for tiered actors. Callers
  invoke `propose_and_apply_note` to signal intent; the server enforces
  the tier capability before applying.
- **Unknown tiers grant nothing.** Registering an actor with
  `--tier auto-everything` loads fine, but `actor_can_auto_apply`
  returns `false` for every kind. Forward-compatible: future tiers can
  ship in v0.7 readers without breaking the v0.6 reader contract.

## Recognized tiers in v0.6

| Tier | Permits auto-apply for |
| ---- | ---------------------- |
| `auto-notes` | `finding.note` (notes only — never caveats, reviews, retracts, revisions) |

## Registration

```bash
vela actor add my-frontier.json reviewer:claim-extractor \
  --pubkey "$(cat ~/.vela/keys/public.key)" \
  --tier auto-notes
```

The `tier` is part of the `ActorRecord` serialized into
`Project.actors`. Pre-v0.6 actors load with `tier = None` and behave
exactly as before — the tier system is purely additive.

## Calling the auto-apply tool

```python
from vela import Actor, Frontier

actor = Actor.from_hex_key(
    actor_id="reviewer:claim-extractor",
    private_key_hex=open("~/.vela/keys/private.key").read().strip(),
)
f = Frontier.connect("http://localhost:3848")

proposal = f.propose_and_apply_note(
    finding_id="vf_...",
    text="A monovalent Brain Shuttle module increased Aβ engagement 55-fold (mouse).",
    reason="extracted from NEJM 2024",
    actor=actor,
    provenance={"doi": "10.1056/nejmoa2308719"},  # Phase β
)
print(proposal.status, proposal.applied_event_id)  # "applied", "vev_..."
```

Server-side check sequence:

1. Actor must be registered.
2. Signature must verify against the registered pubkey.
3. `actor_can_auto_apply(actor, "finding.note")` must return `true`.

If any check fails, the call is rejected with a clear, structured error
the SDK surfaces as `VelaError`. No partial state is persisted.

## Idempotency

`propose_and_apply_note` is idempotent under Phase P (v0.5). A retry
with identical content (same actor, target, text, reason, provenance)
returns the same `vpr_…` and the same `applied_event_id`; the frontier
does not gain a duplicate proposal or event. Agent loops can retry
freely without per-call dedup logic.

## Why no `propose_and_apply_review`?

Review verdicts (`finding.review`) change the state of a finding —
specifically `flags.contested` and `flags.review_state`. Auto-applying
state changes from a tier setting weakens the doctrine "every state
change is reviewable." Adding state-changing auto-apply later requires
a broader tier model with explicit doctrine review and is not in v0.6's
scope.

## Cross-implementation conformance

The tier predicate is pinned by `tests/conformance/auto-apply-tier.json`.
A second implementation following the canonical-JSON spec and the
documented predicate produces identical permit/reject decisions for
every (tier, kind) pair.
