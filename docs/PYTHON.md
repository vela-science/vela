# Vela Python SDK (v0.5)

Single-file Python client for `vela serve --http`. Read-and-write access
to the substrate from any Python agent in ~50 lines.

## Install

The SDK is `bindings/python/vela/__init__.py`. For a checkout, add
`bindings/python` to `PYTHONPATH`. A `pip install vela-python` flow ships
in a later release.

Dependencies: `requests`, `cryptography`. Both are standard.

```bash
pip install requests cryptography
```

## Quick start

```python
from vela import Actor, Frontier

actor = Actor.from_hex_key(
    actor_id="reviewer:will-blair",
    private_key_hex=open("~/.vela/keys/private.key").read().strip(),
)

f = Frontier.connect("http://localhost:3848")

# Reads
stats = f.stats()
findings = f.list_findings(limit=10)
finding = f.find("vf_08c81dd507f6a047")

# Writes (each call signs locally with the actor's Ed25519 key)
proposal = f.propose_review(
    finding_id="vf_08c81dd507f6a047",
    status="contested",
    reason="conditions narrower than claim",
    actor=actor,
)

# Phase β (v0.6): structured provenance on note proposals
proposal = f.propose_note(
    finding_id="vf_08c81dd507f6a047",
    text="FUS+aducanumab reduced Aβ in 3 humans over 6 months",
    reason="extracted from NEJM 2024",
    actor=actor,
    provenance={"doi": "10.1056/nejmoa2308719", "pmid": "38169490"},
)

# Phase α (v0.6): one-call propose-and-apply for trusted-tier actors
# Requires `vela actor add ... --tier auto-notes` first.
proposal = f.propose_and_apply_note(
    finding_id="vf_08c81dd507f6a047",
    text="FUS+aducanumab reduced Aβ in 3 humans over 6 months",
    reason="extracted from NEJM 2024",
    actor=actor,
    provenance={"doi": "10.1056/nejmoa2308719"},
)
print(proposal.status, proposal.applied_event_id)  # "applied", "vev_..."

# Streaming reads (for agent loops)
cursor = None
for event in f.events_since(cursor):
    print(event.kind, event.target.id)
    cursor = event.id
```

## Idempotency

`propose_review`, `propose_note`, `propose_revise_confidence`,
`propose_retract` are idempotent at the substrate layer (Phase P). A
retried call with identical content returns the same `vpr_…` and the
server returns the existing record without duplicating state.

```python
# Both calls produce the same vpr_… and the same applied_event_id.
p1 = f.propose_review(finding_id=fid, status="contested", reason=r, actor=a)
p2 = f.propose_review(finding_id=fid, status="contested", reason=r, actor=a)
assert p1.id == p2.id
```

## Canonical JSON

The SDK exposes the canonical encoding for cross-implementation parity:

```python
from vela import to_canonical_bytes, sha256_hex

data = {"a": 1, "b": [2, 3]}
print(to_canonical_bytes(data))   # b'{"a":1,"b":[2,3]}'
print(sha256_hex(data))           # ed1f...
```

This is the same canonical-JSON rule used by the Rust kernel and the
cross-impl validator at `scripts/cross_impl_conformance.py`.

## Doctrine

- **Reads are open.** No identity required.
- **Writes are signed.** Every write call signs the canonical preimage
  with the actor's Ed25519 key. The server verifies against the
  registered pubkey before persisting.
- **The SDK is one expression of the protocol.** A second Python
  client following only the documented canonical-JSON rule and tool
  argument shapes produces byte-identical signatures and proposal IDs.

## Hello-world agent example

`examples/python-agent/extract_and_propose.py` walks the canonical loop:
paper → optional Claude extraction → propose findings → events_since
print-out → pointer at the Workbench. Runs against a local
`vela serve --http` in under 50 lines of agent code.
