# Behavioral component certificates and the existing evidence lifecycle

**Scoped design fit, not a protocol extension or an implemented qualification service.** This note applies the current [Protocol 1](PROTOCOL.md), [architecture](ARCHITECTURE.md) and [repository boundaries](REPOSITORY_BOUNDARIES.md) to external component evidence. It introduces no object type, schema, runtime, registry or acceptance rule.

An external source repository may produce a versioned behavioral datasheet for a declared use class. Its native artifacts should bind the implementation/configuration digest, action and observation contract, preparation/reset assumptions, horizon, qualified domain, data and code hashes, confidence accounting, supplied refinement/invariance witnesses, limits and dependencies. Whether that certificate is mathematically sound or useful is the producer's and verifier's scoped work; Vela does not discover the quotient or run the component.

The existing lifecycle can retain that evidence:

1. The producer stores exact source-owned artifacts and submits a bounded assertion through a Submission.
2. A Review Method and Verification Record identify the exact inputs, property checked, output artifacts and what the check does not establish. A hash establishes byte identity; it does not establish a behavioral refinement.
3. An attributed authorized Decision, admitted under the named Repository's authority and current state, may accept or reject the proposed change. **Verification result ≠ authorized state transition.** A passing certificate does not make a provider authoritative or grant execution permission.
4. Replay reconstructs admitted Standing and its evidence. External consumers decide whether its exact scope matches their use; a display badge cannot widen the contract.
5. A changed implementation, reset, client grammar, observation boundary or dependency may require correction, supersession or requalification. An external dependency analysis can propose the affected set. Any Standing transition still follows the ordinary authorized Decision path, retaining original records and exact replacement evidence.

A certificate for a one-mission property does not establish lifetime safety. A state re-observation does not erase historical mismatches. A statistical invariance test does not automatically justify exact evidence pooling. These distinctions belong in the source claim and review scope, not in new Vela Core abstractions.

Vela supplies identity, content/version binding, evidence custody, scoped verification, authority-local decisions, replay and correction history. Characterization, active experiment scheduling, reset execution, provider calls, learned models and certificate generation remain external. A source-owned requalification plan has no authority over another repository and no automatic admission effect.

This fit is a proposal to use existing objects. Its empirical value depends on independently qualified producers and consumers. The behavioral-componentization experiments in autonomous-science currently preserve negative qualification results; this note does not assert that an interoperable component market, research intelligence architecture or execution control plane exists.

## Imported followup evidence — 2026-09-15

The [autonomous-science source packet](https://github.com/williamjblair/autonomous-science/blob/f39444a0737e5e9c0702721ffcce3a80a001686c/reports/conversation-evidence-2026-09-15/README.md) records reported concurrency, crash/retry, backend-substitution, Git failover, semantic-survey and qualification results from two research chats. Eight original bundles and the exact E7 tarball are now retained with verified hashes; the corrected audit code reproduces its local concurrency, crash/retry, backend and temporal-model controls. The initial missing-bundle conclusion is superseded. Specific missing generators and annotated inputs remain documented in the source packet. It is external evidence, not a new Vela qualification service.

A useful contract may permit multiple behaviors. A verification record must name the contract and assumptions actually checked; failure of exact timed prediction cannot be silently relabeled as success under a weaker contract. Likewise, one successful probe cannot establish arbitrary-future effect safety.

The reported failures reinforce the existing authority boundary: a past permission decision is not necessarily current authority at commitment; disjoint writes may have shared read dependencies; a durable receipt is not proof that an external effect committed atomically with it. Native resource owners define and enforce those semantics. Identity, custody, verification, scientific admission and external effect authorization remain separate.

Same-team adapters, two executors sharing a Git authority, and manually grouped API operations do not establish independent-provider portability or measured customer benefit. These reports may inform a scoped review, but do not justify extracting a new shared core profile or changing Protocol 1.
