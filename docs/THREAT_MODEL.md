# Threat model

Vela protects the integrity and authority history of a bounded scientific
frontier. It does not decide whether the science is correct.

## Assets and boundaries

The protected assets are:

- accepted event bytes and their causal order;
- artifact content identities and verifier pins;
- human signing keys and signed policy authority;
- the exact proposal, base root, policy, and effect shown at decision time;
- deterministic replay and public wire contracts.

The system boundary is:

```text
untrusted producer or adapter
    -> Receipt v1 + artifacts
    -> validation, frozen verification, and policy routing
    -> Permit under an existing human-signed policy, or Defer
    -> protected human decision when deferred
    -> accepted events in a standalone Git repository
    -> deterministic replay and disposable read projections
```

Git hosts provide byte distribution and repository access control. Vela events
and policies provide scientific authority. The Observatory, MCP, proof packets,
materialized snapshots, graphs, and wikis are projections and are not trust
roots.

## Attacker classes

1. **Malicious or compromised producer.** Can submit arbitrary receipts,
   artifacts, paths, metadata, and scientific prose, including prompt
   injection and fabricated citations.
2. **Compromised agent runtime or local process.** Can modify files writable by
   the user, race a publication attempt, and lie in its UI output. It does not
   possess the human key by design.
3. **Compromised signing key.** Can create signatures within that key's
   authority until the accepted history revokes or replaces it.
4. **Compromised Git host or repository credential.** Can hide, reorder, or
   replace hosted refs and may publish attacker-controlled bytes.
5. **Compromised Observatory or other projection service.** Can omit, delay, or forge
   index answers, but does not possess frontier authority.
6. **Compromised build or install channel.** Can distribute a malicious Vela
   binary that misrenders or mishandles a ceremony.
7. **Resource-exhaustion attacker.** Can send large or repeated requests to an
   exposed HTTP or MCP service.

## Defended properties

### Content and event tampering

Canonical IDs re-derive from exact canonical bytes. Event signatures cover
their defined preimages, and replay validates event structure, causal bindings,
policy certificates, and hashes. Altering a protected field causes strict
validation or signature verification to fail.

This does not make an unsigned source file trustworthy. The claim must bind the
artifact digest that the verifier and receipt actually used.

### Malicious Receipt input

`vela land` treats every Receipt v1 and artifact as untrusted. Strict JSON,
schema checks, size and path limits, digest recomputation, producer identity,
task and base-root binding, verifier requirements, and signed policy evaluation
run before the transaction commits.

A producer signature proves origin only. Model confidence and a successful tool
run are not acceptance. An uncovered class is deferred to the human ceremony;
the producer cannot create a human decision through another command or MCP
profile.

### Decision substitution and stale review

The protected decision ceremony binds one proposal, action, reason, accepted
base, active policy, observation time, and derived effect. If any bound input
drifts, confirmation fails and the plan must be rebuilt. The cross-platform
signer card names the exact action, proposal, frontier, reason, and Decision
Plan root, so a later caller cannot silently replace the reviewed bytes.

`vela review decide` never retrieves the protected key. After every key-free
check passes, it starts a pinned one-shot helper over inherited pipes. The
helper authenticates a bounded signer session, displays the exact card, and
uses the OS-protected key only after approval. `always` mode adds per-use
LocalAuthentication, Windows Hello, or non-cached polkit authentication. Agent
identities are refused. The local trust record pins Vela. Every request binds
the current sibling helper digest, the helper verifies its own executable, and
the CLI verifies the response. A process that can rewrite binaries and their
local trust records remains outside this `user_session` profile.

### Duplicate or concurrent publication

The publication transaction uses explicit path sets and expected Git state. It
does not consume unrelated staged work. Ref movement or a changed base returns
a stale outcome rather than merging, rebasing, or overwriting another writer.
Retries preserve the operation identity and do not silently duplicate the
accepted effect.

### Git history replacement

Anyone can retain an earlier clone, verify event signatures, and compare roots.
The Hub indexes only descendants of the last promoted commit and refuses a
rewind. This makes one class of force-push visible, but it is not a substitute
for protected branches, independent clones, or repository credential recovery.

If a Git host and every available copy are compromised, Vela cannot recover
missing bytes. Durable replication remains an operator and community
responsibility.

### Hub compromise

The Hub selects repositories through a versioned operator catalog, strictly
replays them, and serves derived rows. It has no frontier-state, source-
registration, deprecation, signing, transparency-proof, or peer-consensus
write surface.

A bad Hub can lie about discovery or freshness. Resolve the dispute from the
configured Git URL and exact commit with `vela check . --strict` and the frozen
verifier. Agreement between multiple Hub deployments is corroborative only; it
does not create authority.

### Service resource abuse

Local `vela serve` and the Hub cap request bodies; the Hub also rate-limits its
public route classes. Keep local serving bound to a trusted interface unless a
reverse proxy supplies authentication and additional budgets. Receipts should
reference large artifacts by content digest and path rather than embedding
unbounded payloads.

## Partially mitigated risks

### Key compromise

A valid compromised key can make authority-bearing signatures within its
current scope. Signed policy can narrow automatic Permit classes, and
multi-party policies can require distinct eligible signers for sensitive
transitions. Revocation and policy-head changes must themselves enter the
accepted history through the authorized human path.

Historical signatures remain historical facts. Recovery cannot rewrite them.
A fully compromised authority quorum requires an explicit social and key-
recovery procedure outside the normal producer loop.

### Verifier compromise or underspecification

A frozen verifier can be deterministic and still check the wrong property. A
producer may also formalize a weaker statement than the prose claims. Vela
therefore records verifier identity, environment, inputs, outcome, and caveats,
and keeps statement faithfulness and significance separate from a kernel pass.

For a deliberately delegated computational lane, AcceptancePolicy v0.2 can
also bind Permit to the full roots of one packet, producer profile, verifier
capsule, and positive result contract. This blocks same-class verifier and
target substitution; it does not make the selected verifier scientifically
adequate. That semantic judgment remains part of the human policy approval.

AcceptancePolicy v0.3 can further bind that exact lane to the full root of one
retained Receipt identity binding. This avoids granting a replaceable producer
frontier-wide registry meaning. It is not open self-enrollment: the protected
policy plan rederives the full binding, the human-signed rule names it exactly,
and even a globally registered actor cannot substitute another key or omit the
v0.3 root.

New verifiers need positive fixtures, meaningful negative controls, resource
bounds, and independent review of what their result means. A verifier maintained
by the producer is evidence with a disclosed conflict, not independent
corroboration.

### Install and build compromise

GitHub releases and source builds remain software supply-chain dependencies.
Pin an exact release, verify published checksums or attestations when supplied,
review changes that affect key custody, and keep the signing binary pin outside
agent-writable sandboxes where possible. Vela cannot use its scientific event
protocol to make a malicious executable safe.

## Explicitly out of scope

- deciding scientific truth, novelty, importance, or ethics;
- preserving restricted source bytes Vela never received;
- protecting a workstation whose user account and human key are both fully
  compromised;
- Git-host availability and organization-account recovery;
- governance institutions, IRB-equivalent review, or legal compliance;
- confidentiality for data committed to a public frontier repository.

A conformant, signed, replayable frontier can still contain a bad human
judgment. Vela makes the judgment attributable and correctable; it does not make
it correct.

## Reporting

See [`../SECURITY.md`](../SECURITY.md). Report security vulnerabilities
privately with reproduction steps, the affected boundary, and the smallest
known malicious input. Scientific disagreements and ordinary data corrections
belong in the frontier's normal Receipt v1 and decision workflow.
