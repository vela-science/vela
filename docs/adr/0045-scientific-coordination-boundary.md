# ADR 0045: Scientific coordination stops at the state boundary

- Status: Accepted, 2026-08-13
- Reaffirms: ADR 0031, ADR 0039
- Protocol, schema, command, and authority effect: None
- Documentation effect: integration and architecture boundary

## Context

Vela can coordinate work across proof assistants, simulators, workflow systems,
agent workbenches, laboratories, and human review without owning their execution
semantics. Calling that role a control plane is useful only while it does not
suggest that successful execution is accepted scientific state.

A universal `ScientificOperation` would force theorem proving, statistical
inference, simulation, physical experiments, and review into one false
lifecycle. It would also duplicate the queues, retries, credentials, budgets,
safety controls, and package resolution already owned by native systems.

## Decision

Vela Core is the Git-native scientific-state and inheritance protocol. It owns
the portable boundary and deterministic transition sequence:

```text
Submission -> Proposal -> Verification Record(s)
           -> authorized attributed Decision -> Event -> replay -> Standing
```

Core does not own execution scheduling. `Run`, `Task`, `Experiment`, and
`Operation` remain activity-plane concepts owned by native tools, workbenches,
laboratory systems, and external orchestrators. Those systems may retain their
own sessions, checkpoints, attempts, traces, retries, resource use, and failure
states. They cross into Vela only through exact Artifacts, producer evidence,
Submissions, or scoped Verification Records.

Native-system interoperability keeps three operations distinct:

```text
reference  preserve an exact native identity without taking custody
snapshot   retain selected exact bytes and a declared reproduction contract
admit      let one Repository decide one bounded proposed transition
```

Protocol 1 adds no objects for these operations. Reference is not Snapshot;
Snapshot is not Verification; Verification is not admission.

Integration profiles are non-normative package-plane contracts. They reuse
native standards, report semantic loss, and remain outside Repository authority.
A shared profile enters Core only after at least two maintained consumers agree
on exact behavior and the extraction removes more maintained duplication than
it creates. Capability discovery remains a replaceable, read-only projection.

Reviewer kinds are peers at the Verification boundary. A human, AI model,
organization, or deterministic tool does not receive evidentiary weight from
its category. Weight follows the named method, exact inputs, independence,
retained outputs, scope, and limitations. Only the separate authorized and
attributed Repository Decision can change Standing. The performer may be a
human or agent when current Repository policy permits it; authorization,
signatures, exact inputs, and replay remain unchanged.

## Consequences

- Replacing an agent, scheduler, model provider, laboratory controller, or
  execution backend does not rewrite accepted Standing.
- A completed native run cannot imply a passing Verification or a Decision.
- Failed and partial runs may still supply useful retained evidence.
- Hosted indexes, capability maps, rankings, and work packets have no authority
  effect and must be rebuildable from exact sources.
- Core adds no launch, status, retry, cancellation, queue, budget, or credential
  API for external work.
- New integrations use the non-normative integration-profile template and must
  say which native standard they reuse and which bounded gap remains.

## Rejected alternatives

### Add a universal scientific operation object

Rejected. The common lifecycle would be either too weak to protect domain
meaning or broad enough to become a workflow engine.

### Treat execution completion as acceptance

Rejected. A correct execution may check the wrong statement, use a misspecified
model, or execute a flawed physical procedure. Verification remains scoped and
admission remains an authorized, attributed Repository Decision.

### Make capability discovery authoritative

Rejected. Native registries and providers retain their identifiers, availability,
rights, and operational policy. Vela read products may index them without
becoming a package namespace, scheduler, endorsement, or trust root.
