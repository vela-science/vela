# Vela external-validation program

> **Historical launch packet.** The versions, roots, routes, and open gates
> below describe the 2026-08-12 launch-candidate state. They are retained as
> scoped evidence and must not be used as current installation or validation
> instructions.

Status: **open for external participation**.

Vela has a qualified Public Launch Candidate. The external program asks
separately operated producers, verifiers, readers, and authorities to test the
published boundaries. Vela records each returned result with its exact scope.
Interest, clones, parser passes, and same-operator agents do not count as
external adoption.

The launch reference state is:

- signed Vela CLI `v0.975.1`, commit
  `9d1a99dbe0a1c8ffc008cc0f5ee4e06831ea6e14`;
- Mathematics commit `4f422289e6a8991735fced4905d53d9d54c9900f`,
  Repository root
  `sha256:0e24fa1b13d7eda7b4e809564ec414eb1fda09f5dcf9aa8a6bcd6ae69ac96197`;
- Web commit `b454ccc1c66df0a89f7f85e2b01b04ed3db91835`,
  projection root
  `sha256:8513fceafba9136f3ffa1ea3f36ee91160b36ca60ada0a7251995844be45c9dc`;
- public flagship <https://problems.science/problems/erdos-problems/321>.

## Install and read

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.975.1/install.sh | \
  VELA_VERSION=v0.975.1 bash

git clone https://github.com/vela-science/math.git math
git -C math checkout 4f422289e6a8991735fced4905d53d9d54c9900f
vela replay math --json
vela status math --json
vela why math \
  vcl_a618b77ab0f6a4b5b186133e37af555a22c6acb71a4746bab0b144b8973668a6 \
  --json
vela why math \
  vcl_3d4fd59554ccaa2b792b08abae16a8d0fe329d4901ad798fe05c6c7769c9966b \
  --json
```

Expected current facts:

- replay root
  `sha256:0e24fa1b13d7eda7b4e809564ec414eb1fda09f5dcf9aa8a6bcd6ae69ac96197`;
- three accepted Claims and zero pending Claims;
- successor Standing `accepted`;
- predecessor Standing `superseded` while its historical Proposal remains
  `accepted`;
- direct Submission as the current action.

A default local executable older than `0.975.1` cannot report the corrected
supersession read contract. Record `vela --version` and the installed binary
digest with every return.

## Producer packet

A separately operated native workbench should author or emit one bounded
candidate, retain exact artifacts and environment pins in its source-owning
Repository, then submit it through the ordinary CLI:

```bash
vela submit --repo . \
  --claim "<bounded result>" \
  --type <theoretical-or-computational> \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --requires-verification <registered-property> \
  --as agent:<name> \
  --json
```

Return the workbench owner, exact source commit, command, environment, artifact
roots, Submission and Proposal roots, failures, and nonclaims. Do not grant the
workbench Vela authority. A same-operator transport check remains compatibility
evidence.

## Verifier packet

An independently controlled verifier should choose one registered property,
retain its Method and output, disclose provider/model/tool entities, and record
shared dependencies:

```bash
vela verification record . <proposal-id> \
  --profile <profile> \
  --method <tracked-method.json> \
  --property <registered-property> \
  --outcome <pass-fail-or-inconclusive> \
  --does-not-establish "Scientific acceptance or Standing." \
  --independent-of agent:<producer> \
  --output <retained-review-output.json> \
  --as verifier:<name> \
  --json
```

Return the signed Verification Record, Method, output root, exact inputs,
procedure, limitations, and operational or evidentiary links to the producer.
Human and agent performers use the same evidence boundary. Performer kind does
not establish independence or quality.

## Clean-room reader packet

A reader should use a clean account or machine, the signed release, a full
Math clone, and no maintainer explanation. Report:

1. install source, signature result, binary version and digest;
2. replay root, accepted/pending counts, and trust-root source;
3. the successor and predecessor Standing values;
4. the two Verification properties and what each omits;
5. Decision performer and Repository authority principal;
6. the correction consequence and unchanged Claims;
7. the next valid action;
8. elapsed time, questions, interventions, and failure logs.

The public Problem page supplies the human-readable path. The Repository and
CLI supply the exact path. A reader operated by the Vela maintainer or the same
agent provider counts as internal evidence.

## Authority-candidate packet

An authority candidate must operate its own Repository, policy, trust root,
key custody, and mandate. It should retain a bounded Submission and Verification
set, read a fresh inbox entry, make one authorized Decision, replay, and report
whether it agrees or disagrees with another Repository.

Return the Repository ID, origin and sequence-one roots, policy and keyset
roots, performer, authority principal, entry root, Decision and Event roots,
before/after Repository roots, replay result, and governance owner. Do not copy
the Math authority key or import Math Standing. Separate control and a real
capacity to disagree define plural authority.

## Correction-cascade packet

The accepted Erdős 321 correction retires one predecessor and preserves two
unrelated accepted Claims. It has no affected dependent Claim and therefore
does not satisfy the Protocol 1.0 non-empty cascade gate.

A valid cascade starts with a producer-declared scientific dependency. A later
correction or withdrawal must yield exact affected and unaffected sets,
repair obligations, and cross-implementation agreement. Do not retrofit a
dependency to create a benchmark result.

## Provider-loss packet

The direct Web qualification acquired exact public source commits, rebuilt the
projection twice, loaded two empty local PostgreSQL instances, and obtained
byte-identical table and release roots. The SELECT-only reader could read the
result and write none. The retained reconstruction root is
`sha256:7aebb930d7ec4d59706326e63bf83a2d318a8dd3a02d71d17346196bec059863`.

Vela Web source and its retained source-adapter asset require authenticated
access. Anonymous readers cannot repeat the Web reconstruction from public
inputs, and this program does not claim otherwise. A separately operated
provider-loss review must first obtain read access to the exact Web commit and
adapter asset from the maintainer, then run the repository-owned clean-room
`projection:reconstruct` command with disposable local PostgreSQL instances.
The canonical direct-release command requires Vela's GitHub, Neon, Vercel, and
domain authority; read access does not grant it. Request source access through
the public Vela issue below. Return the access grant's scope, exact inputs,
source-adapter set, table roots, release root, reader permissions, elapsed
time, interventions, and disagreements. GitHub Actions remains an optional
redundant path.

## Return channel and acceptance rule

Open a public issue in the owning repository and attach or link immutable
evidence:

- Protocol, CLI, Web, reader, and authority results:
  <https://github.com/vela-science/vela/issues/new>;
- Mathematics source and authority results:
  <https://github.com/vela-science/math/issues/new>.

Name the operator and organization, exact commits and roots, execution date,
hardware and software, credentials or services used, interventions, failures,
shared dependencies, and license or availability limits. Vela maintainers will
classify the return as internal, separately operated, independently controlled,
or plural authority. The evidence keeps its observed outcome even when it
fails.

## Open evidence gates

Protocol 1.0 still requires:

- one external producer emitting a conformant Submission;
- one independent scientific reader reconstructing accepted state;
- one real non-empty correction cascade;
- cross-implementation agreement on inheritance consequences;
- no known imminent wire break and no falsification by the disciplined Git
  plus RO-Crate baseline.

The breakthrough claim adds an independent verifier, two independently
governed authorities, exact repair obligations, measured cold-successor
improvement, recurrence, and operation without a central Vela service.

The Public Launch Candidate claims none of those results.
