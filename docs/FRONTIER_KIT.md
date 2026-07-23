# Frontier Kit convention

A Frontier Kit is an ordinary Git frontier whose root README gives a new
contributor enough information to choose, verify, and land one bounded result.
It uses the files and contracts Vela already ships. The convention adds no
manifest, package object, Receipt field, registry, or install command.

The producer may land evidence. A signed policy may Permit a narrow class of
result; otherwise Vela places the proposal in the human sign queue. The
producer cannot decide, accept, reject, or sign for a human.

## Root README contract

Write the following sections in this order. Keep pinned values in the README or
an existing lock file. Do not ask contributors to edit `.vela/` state or the
private work-session record.

| README section | It must answer | Existing source |
| --- | --- | --- |
| 1. Question | Which bounded scientific or formal question does this frontier maintain? Which claims fall outside its scope? | Root `README.md` and `SCOPE.md` |
| 2. Start state | Which full Git commit, event-log root, scientific-state root, and repository identity form the base for this task? | `git rev-parse HEAD`; `vela status . --json`; `vela.lock` at that commit |
| 3. First target | Which ranked target should a newcomer take, and which accepted result could it unlock? | `vela next . --json` and the target briefing |
| 4. Environment and verifier | Which OS tools, language/toolchain versions, dependency commits, limits, and frozen verifier apply? | `vela.lock`, checked-in environment files, verifier source, and witness metadata |
| 5. Result classes | Which observable counts as success, informative failure, correction, or inconclusive evidence? Which command checks it? | `SCOPE.md`, task briefing, and verifier contract |
| 6. Authority | Which actions may the producer take? Which decisions require the signed policy or a human key? | `VELA.md`, active policy, and this document |
| 7. Upstream path | Which domain library or source repository should receive reusable definitions, fixes, or methods? | Root README link to the named upstream project and its contribution guide |
| 8. Fork and replay | Which commands let a contributor clone, verify, reproduce, and continue without a Vela service? | Git, `vela check`, `vela reproduce`, and the commands below |

Record the event-log root shown at the pinned start commit and the Profile v1
scientific-state, identity, dependency, and profile roots from `vela.lock`.
Current `vela status . --json` exposes the event root at `roots.event_log`.
`vela work` retains the exact Target Index v2, packet, repository roots, and
claim-time read set in `vela.target-task-binding.v1`; landing copies the same
binding into Receipt v1.

## Copyable contributor path

Run these commands from the frontier root. Replace angle-bracket values with
the values named in the root README.

```bash
git rev-parse HEAD
vela status . --json
vela check . --strict
vela next . --json

vela work <target> --as agent:<name> --json

# Run the selected, pinned verifier from the README.
vela reproduce .

vela land --work <target> \
  --claim "<bounded result>" \
  --type <computational|theoretical|empirical|negative|contradiction> \
  --replayability <exact|bounded|approximate|unavailable|unknown> \
  --artifact <path>:<kind> \
  --caveat "<what this result does not establish>" \
  --as agent:<name> \
  --json

vela status . --json
vela check . --strict
```

Plain `vela land --claim ...` infers the work target when this actor owns one
active session. Automation should pass `--work <target>`. File-based
`vela land receipt.json` remains the public boundary for stateless and foreign
producers.

A committed Permit or Defer closes the session record. Permit records the
policy certificate that authorized admission. Defer leaves the proposal for
protected `vela review decide`. Deny and invalid input leave the session intact and return a
repair action.

Release an abandoned lease through Vela:

```bash
vela work <target> --drop \
  --reason "<why work stopped>" \
  --as agent:<name> \
  --json
```

Vela signs a same-owner zero-TTL update against the exact prior lease. It
removes private scratch after the release event commits. Deleting `.vela/work/`
does not release the frontier lease.

## Named verifier profile

The README must pin the verifier profile, runtime or container digest, source
commit, dependency roots, input paths, and replay command. Vela 0.9 does not
embed target-specific producer adapters in the core binary. Use the released
Canopus profile or a checked-in parent script named by the work response.

The verifier output remains evidence. For a formal declaration, the receipt
must state the exact declaration and source root that the kernel checked. The
producer must retain any gap between kernel checking and the faithfulness or
significance of the formalization as a caveat.

## Result and correction rules

The task contract should distinguish four useful outcomes:

| Outcome | Required record |
| --- | --- |
| Success | Claim, selected verifier result, content-addressed evidence, and caveats |
| Informative failure | Attempted method, environment, failure mode, evidence, and the route ruled out |
| Correction | The prior claim or artifact reference, corrected bytes, reason, and lineage |
| Inconclusive | Test performed, observed result, unresolved alternatives, and the next discriminating check |

Do not land raw search traces as scientific state. Select the positive,
negative, corrective, or inconclusive result that another producer can inspect
and reproduce. Keep incidental failures in private scratch unless they support
one of the result classes above.

## Living frontier and evaluation snapshots

A living frontier may advance its source and toolchain pins through reviewed
changes. An evaluation snapshot pins the full Git commit, event-log root, Lean
version, domain-library commit, verifier bytes, and task set. Give each snapshot
an immutable Git tag or bundle and record its predecessor. Publish a corrected
snapshot with lineage to the old one; do not rewrite the old tag or accepted
event.

Reusable definitions belong in the domain project named by the root README.
The frontier can carry the evidence and proposal while an upstream pull request
is pending. Record the upstream issue, pull request, or commit as provenance
when one exists.

## Offline fork

Git remains the transport. A hosted Vela service is optional.

```bash
git clone <frontier-url> frontier-copy
cd frontier-copy
vela check . --strict
vela reproduce .
vela next . --json

git bundle create ../frontier.bundle --all
git clone ../frontier.bundle ../frontier-offline
cd ../frontier-offline
vela check . --strict
vela reproduce .
```

An offline fork may add its own targets, policies, and evidence through the
same event and Receipt contracts. Keep its authority claims scoped to the
fork's keys and accepted log.

## Maintainer check

Before calling a repository a reference Frontier Kit, ask a contributor who
has no maintainer credentials to use the clone and README. Record:

- time to identify the question, pinned base, first target, verifier, and
  authority ceiling;
- time from `work` to verifier result and from receipt to policy route;
- commands that needed repair and questions the README failed to answer;
- the resulting Receipt root and whether the contributor described its status
  as Permit, Defer, or Deny; and
- the full Git, event-log, toolchain, dependency, verifier, and task-set roots.

The run succeeds when the contributor lands a real Receipt v1 without editing
receipt JSON or private session state and can state who, if anyone, accepted
the claim.
