---
title: "Vela: Correction-Aware Scientific State Across Plural Authorities"
subtitle: "Working draft. Not a protocol-breakthrough claim."
---

## Abstract

Scientific work increasingly moves through heterogeneous repositories, model
runs, formal systems, databases, and human review. Git can preserve the exact
bytes of those systems, but it does not define which scientific assertion a
result supports, what a verifier checked, who had authority to accept it, or
which later conclusions need attention when an accepted assertion changes.

Vela is a Git-custodied protocol and command-line system for retaining
authenticated scientific Submissions, scoped Verification Records, local
human Decisions, replayable Standing, and explicit correction lineage. Its
core design separates producer evidence, mechanical verification, and local
scientific authority. Optional workers and read models remain removable.

This paper asks whether that separation produces measurable inheritance and
correction value instead of metadata growth. We register a benchmark in
which independent readers must derive the same bounded correction impact,
preserve an independent support route, reject authority escalation, transfer
exact state into a second independently governed Frontier, and improve cold
continuation over Git plus the same evidence. The audit of four current
mathematical Frontiers found no qualifying historical correction fixture among
2,831 retained Claims. Rust and clean-room Python readers agree on a synthetic
qualification vector, but that result does not satisfy the scientific,
federation, external-reproduction, or user-value gates. The current evidence
therefore supports local admission and replay, not the registered protocol
claim. This draft will report either the completed benchmark or its
falsification.

## 1. Introduction

Abundant computation reduces the cost of producing candidate scientific
artifacts. It does not remove the cost of deciding what was claimed, checking
the scope of the check, preserving rejected and superseded work, or handing
the resulting state to another person or system. These costs become more
important as production becomes cheaper because the number of plausible
outputs, partial failures, and dependent continuations increases.

Existing systems solve parts of the problem:

- version control retains exact file history and supports decentralized byte
  exchange [1];
- workflow engines execute dependency graphs;
- formal systems check terms against fixed statements and kernels;
- provenance models and research-object formats exchange run and artifact
  metadata [4, 6, 7];
- repositories and review systems coordinate publication; and
- databases make current projections searchable.

Vela should preserve those mechanisms and keep its own surface small. The
paper tests a narrower question:

> Can independently governed scientific repositories exchange exact proposed
> state transitions, verify their causal inputs, apply local authority, replay
> the same bounded result, and localize the consequences of later corrections
> without a central writer or global consensus?

Vela treats that sentence as a falsifiable systems claim. The design follows
three constraints.

1. **Exact history is not authority.** Content addressing proves byte identity.
   It does not decide scientific Standing.
2. **Verification is not acceptance.** A verifier reports one scoped property
   over exact inputs. A local human Decision changes local Standing.
3. **Derived convenience is removable.** Workers, search indices, graph
   layouts, databases, and websites cannot become necessary for replay.

The paper makes four contributions only if their evidence gates pass:

1. a small protocol waist for Submission, Verification, Decision, and replay;
2. a bounded correction-impact contract with explicit completeness and
   relation semantics;
3. a two-implementation, adversarial, cross-Frontier evaluation; and
4. a matched cold-use comparison against Git plus identical evidence.

At present, only the first contribution and synthetic implementation
qualification are demonstrated. The remaining claims are registered
experiments.

## 2. Scope and system model

### 2.1 Frontier

A **Frontier** is an independently governed Git repository containing
canonical scientific records and an append-only authority history. A Frontier
has one current repository root:

```text
R_F = H(epoch, accepted Claims, pending Claims, Submissions,
        Verification Records, Decisions, authority heads, Artifacts)
```

`H` is canonical JSON followed by SHA-256. Readable identifiers are routing
handles; full roots are object identity.

A Frontier does not represent global truth. It represents the replayable
scientific Standing produced by one governance context.

### 2.2 Actors and authority

The system distinguishes:

- **producer:** authenticates a proposed Claim and its evidence;
- **verifier:** authenticates one scoped check over exact retained inputs;
- **repository writer:** authorizes an exact repository transaction;
- **reviewer:** makes a semantic Decision under the Frontier's local policy;
- **reader:** derives non-authoritative projections; and
- **foreign Frontier:** retains attributed state but applies its own authority.

No role is inferred from model capability, computational cost, package origin,
or verifier success.

### 2.3 Core objects

| Object | Meaning | Authority |
| --- | --- | --- |
| Claim | Exact assertion, scope, evidence, revision, and relations | none by itself |
| Submission | Authenticated producer request over exact Artifacts | producer attribution |
| Registration Record | Exact intake and routing record | repository writer |
| Verification Record | Scoped result over exact Claim, Submission, Proposal, Artifacts, and implementation | verifier attribution |
| Proposal | Pending requested transition | none |
| Decision | Accept, reject, or withdraw one exact Proposal | local reviewer |
| Event | Append-only semantic transition | local Frontier |
| Standing | Deterministic replay result | derived from local history |

### 2.4 State transition

Let `S` be a valid Submission, `V*` its retained Verification Records, `P` the
pending Proposal, and `D` an authorized local Decision.

```text
register(S) -> pending(P), accepted-event delta = 0
verify(V, S, P) -> retained(V), accepted-event delta = 0
decide(D, P, V*) -> append(Event), replay Standing
```

Registration and Verification cannot accept a Claim. A Decision cannot omit
or substitute the exact Proposal, Claim, Submission, and required Verification
inputs.

### 2.5 Correction without erasure

A corrective Submission names the full identifier and root of one accepted
predecessor Claim `A`. If accepted, successor `A'` enters current Standing and
the predecessor remains retained with its prior Decision and evidence.

```text
A --corrected_or_superseded_by--> A'
```

The correction transition is canonical. Its downstream impact is initially a
derived experiment rather than another Event or universal ontology.

## 3. Protocol mechanisms

### 3.1 Canonical encoding

Current Vela objects use a bounded canonical JSON domain:

- object keys sort recursively;
- array order is preserved;
- Unicode is encoded as UTF-8 without normalization;
- unsupported numerical values are rejected; and
- object roots use full lowercase SHA-256.

Independent JavaScript fixtures reproduce the Rust Submission and Verification
bytes. Canonical hashing vectors cover nested ordering, Unicode, numerical
rendering, empty values, and realistic Event preimages. Vela's encoding is a
protocol-specific bounded domain rather than a claim to invent JSON
canonicalization; RFC 8785 supplies the relevant general precedent [2].

### 3.2 Authentication and repository authority

Producer and verifier records carry Ed25519 identity bindings and whole-body
signatures [3]. Repository transactions are independently signed, root-bound
records. Scientific Decisions require local reviewer authority. The model and
optional worker do not receive reviewer or repository-authority credentials.

The protocol assumes the configured cryptographic primitives and local
authority credentials are not compromised. It does not solve coercion,
malicious governance, key recovery, or the correctness of scientific
judgment.

### 3.3 Transaction and replay boundary

One recoverable repository transaction binds:

- the exact read set;
- execution binary identity;
- canonical object postimages;
- authority history;
- Event postimages when semantic Standing changes; and
- the resulting repository root.

Pre-marker failures produce zero canonical writes. Post-marker recovery
installs the same planned bytes without another semantic authorization.
Derived files may be regenerated; canonical evidence and authority bytes may
not drift.

### 3.4 Portable waist

Submission and Verification are the portable producer and verifier objects.
Git remains the transport and custody layer. A foreign workbench may emit
these objects without importing Vela's reducer, authority implementation, web
application, or worker runtime.

## 4. Bounded correction impact

### 4.1 Input

The experimental reader consumes a closed causal slice:

```text
I = (A, A', C, E, M, B)
```

where:

- `A` and `A'` are exact predecessor and successor Claims;
- `C` is a root-bound Claim set;
- `E` is a root-bound relation set;
- `M` maps every relation kind to one declared consequence; and
- `B` declares count and completeness bounds.

The first experimental vocabulary is intentionally small.

| Relation | Meaning |
| --- | --- |
| `depends_on` | source requires target; affected target requires source repair |
| `supports` | target is one support route for source |
| `discovery` | source was discovered through target; no scientific consequence |

An unknown kind or substituted meaning fails closed.

### 4.2 Algorithm

```text
CORRECTION-IMPACT(I):
  validate all Claim and relation roots
  require complete Claim and relation sets, otherwise return INCOMPLETE
  unavailable := { predecessor }
  repair := {}
  changed := {}
  causes := {}

  repeat until classifications and causes reach a fixed point:
    for each hard dependency source -> target:
      if target in unavailable or repair:
        repair += source
        causes[source] += edge and causes[target]

    for each source with support routes:
      lost := routes whose target is unavailable or repair
      surviving := remaining routes
      if lost is nonempty and surviving is nonempty:
        changed += source
      if lost is nonempty and surviving is empty:
        repair += source
      causes[source] += lost routes and their target causes

  emit sorted affected and unaffected Claims
  emit every lost and surviving support route
  emit one root-bound discharge condition per repair Claim
```

The output is non-authoritative. It contains no Standing field and cannot
write a Frontier.

### 4.3 Required properties

Within the declared closed bound:

1. **determinism:** independent implementations emit identical canonical
   bytes;
2. **history retention:** the predecessor identity remains present;
3. **affected-set correctness:** precision and recall are 100 percent against
   the preregistered answer;
4. **route preservation:** every unaffected independent support route remains;
5. **explicit incompleteness:** incomplete inputs never produce a complete
   affected or unaffected set; and
6. **authority containment:** no projection changes Standing.

## 5. Implementation

Vela is a Rust workspace with four separable layers:

```text
vela-protocol   canonical public objects and validation
vela-authority  policy, authentication adapters, and authority replay
vela-edge       replaceable Git/filesystem and derived analysis adapters
vela-cli        porcelain and repository transactions
```

Canopus is an optional TypeScript producer and evaluation harness in the same
release repository. `@vela-science/protocol` is an independent TypeScript
consumer of public object contracts. Neither is required for canonical replay.

The correction-impact reference reader is Rust in `vela-edge`. The clean-room
reader is dependency-free Python and imports no Rust implementation. Both
consume the same public JSON fixture bytes.

The Observatory and its Neon projection are read-only conveniences. Removing
them does not remove canonical Git state.

## 6. Evaluation

### 6.1 Research questions

The evaluation measures:

- transition fidelity;
- correction localization and repair;
- independent-route survival;
- authority containment;
- removability and hosted-service failure;
- exact transfer into a second Frontier; and
- inheritance lift over Git with identical evidence.

Correctness gates are binary. Speed cannot compensate for a wrong affected
set, hidden truncation, lost route, or unauthorized Standing change.

### 6.2 Historical fixture audit

At the audited clean commits, four mathematical Frontiers retain 2,831 Claim
records and no accepted correction, supersession, or retraction relation.

- **Erdős 281:** has the required dependency shape, but no genuine
  correction.
- **Erdős 128:** has an upstream source correction, but the retained Claim
  lacks an exact source commit, path, and root, and its topology is incomplete.
- **Erdős 1197:** has an exact kernel-clean complete proof plus a conditional
  proof, but this is independent completion rather than scientific correction,
  and the retained relation labels conflict.

This is a failed historical entry gate. The cases remain negative evidence and
adversarial inputs; none may be relabeled into a positive fixture.

### 6.3 Prospective writer qualification

Erdős 424 supplies an exact source-statement transition from
`generatedSet.HasPosDensity` to `generatedSet.HasPosLowerDensity`. A frozen
plan binds both source commits, trees, file roots, predecessor Claim, bounded
replacement assertion, and nonclaims.

This fixture tests exact correction authoring and the human Decision boundary.
It lacks the consequential diamond and cannot satisfy the primary protocol
benchmark.

**Current status:** the exact Submission is registered as
`vsb_44cd52724425171f` at root
`sha256:4cd059848ce06c943e2cafffac0ffa0f14838b5adba022bc4c076df6acc5af12`.
Its replacement Claim `vcl_4bc14401b203218cb7b9de0141747e0c17cea3a6b0cc522639323ab13e432eaf`
is pending under Proposal `vpr_23f32f95d4f073e8`; registration changed no
accepted state. A deterministic source verifier reproduced both file roots and
the exact Git diff in two object-database contexts. The first verifier draft
was rejected before import because default Git diff output abbreviated blob
IDs differently across those contexts. The repaired implementation forces
full blob identities; its signed first-party Verification
`vvr_ed3383c1cd640d43` was imported with outcome `pass` and accepted-event
delta zero at Frontier commit
`b696ececbf1dfb249dadbbc86f211e9445a09cc6`, repository root
`sha256:b70da05f7fdb93925dc2fed3d7a680b65ef3ac6d68ed51cd2985bd61c1b06cb9`.
Strict verification and a clean clone reproduce that state. The separate
human Decision remains pending. This first-party check earns no
external-participant credit and does not change Standing.

### 6.4 Synthetic reader qualification

The synthetic diamond contains one hard dependent, one Claim with two support
routes, one discovery-only Claim, and one independent support Claim.

| Result | Root or count |
| --- | --- |
| Canonical input root | `sha256:68a5094a5a98d60ab1d34c11c5306a202ea44d126f6dc95f33e20d31b5b1f8da` |
| Canonical projection root | `sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6` |
| Implementations agreeing | 2 |
| Repair-required Claims | 1 |
| Route-changed Claims | 1 |
| Independent surviving routes | 1 |
| Unaffected Claims | 2 |

The shared adversarial set covers unknown consequential relations, discovery
rebound as dependency, missing premises, shortened roots, incomplete relation
sets, omitted independent routes, connected cycles, and resource bounds.

This result establishes implementation readiness only. It is not scientific
evidence or external independence.

The held-out selection plan was frozen after both readers existed and before a
held-out case was known. Its canonical root is
`sha256:b9dbf4b86b841b7b09a79e865ae0187a3ed6dcead896cc2446edcacb836af6a8`.
It scans accepted correction transitions after four pinned Frontier baselines
and selects the first qualifying case in canonical Decision order. If no case
qualifies, the benchmark records a failed held-out entry gate rather than
substituting a synthetic or preferred fixture.

### 6.5 Removability qualification

A separately frozen first-party test cloned the exact pending Erdős repository,
set an empty home directory, denied all network access with the operating
system sandbox, and ran strict repository verification, status derivation, and
proposal inspection using only the pinned Vela binary and Git bytes. Canopus,
Vela Web, the Observatory, Neon, hosted APIs, the original producer session,
and repository-authority credentials were absent.

The three commands reproduced repository root
`sha256:69bbc0d35b0f422f9df8e3f9c720ae3c855ab858c5667434fc57e30f11af5553`,
2,771 accepted Claims, one pending Claim, and the exact pending Proposal.
Canonical path-normalized command outputs reproduced byte-for-byte on a
second run. The frozen plan root is
`sha256:659146f45e9c02aa1c1771e9ebdb5a19fce663f1d54b41852262ea478de994d4`;
the result root is
`sha256:979995062e655597084f15ae8e265e6660393e127a38a1da46641cbf57c3ab96`.

This is positive local evidence for B5 and B6. It does not complete those
families for a real correction fixture, test cross-Frontier transfer, measure
cold-user value, or establish independent reproduction.

### 6.6 Foreign-transfer contract audit

A frozen source audit tested whether the current public objects and CLI can
retain one exact foreign accepted Claim and its authority evidence in a second
Frontier without changing local Standing. The audit found only
predecessor-epoch migration lineage:

```text
Claim imported_from    = era, object_id, object_root, predecessor_commit
Proposal imported_from = proposal_id, proposal_root, predecessor_commit
```

The current contract does not bind a source Frontier ID, source repository
root, source Decision, source authority anchor, completeness status, or an
explicit declaration that foreign Standing has no local authority. The public
CLI deliberately exposes no federation or foreign-import command. Reusing the
migration fields would change their meaning and would still omit required
inputs.

The audit plan root is
`sha256:03b774402311a080f6491aa3bf83c336d96aaeebcc08dad706ea373d028d1be3`;
the deterministic result root is
`sha256:1e0ed787c155677a908ee1c006355b5ae18aef2394bec39b5174675284008c15`.

This is a negative result: the audited Vela revision cannot pass B8. It does
not show that a hosted Registry, resolver, global namespace, or federation
service is needed. Rust and clean-room Python readers independently reproduce
the same current-contract inventory from compiled public types and public
source. Their shared fixture root is
`sha256:fffb66f5afa69b8a47824a45bc382d97e4c655dc282a3b3d6ba339f2b70906ae`.
That is implementation diversity, not organizational independence. A minimal
derived envelope is eligible for experiment only after the real correction is
terminal.

The matched Git-versus-Vela reviewer protocol is frozen at root
`sha256:68fb039088302d19f02cf2628c16004e174649b1f952a63a0fd35210c0dd0ef8`.
It gives both arms identical terminal repository, evidence, verifier, Git, and
documentation bytes; only the Vela arm receives the exact read-only CLI.
Execution remains gated on the terminal Erdős 424 Decision and a rooted task
instance. A dependency-free exact-field scorer is frozen at source root
`sha256:c949f9dec835cbe97bb89a22ccc006e8e767f88b86dfc6c4a0083732ff3fcd63`.
It rejects task-instance drift and reports false authority statements as hard
failures. The terminal amendment must still bind that scorer, the answer key,
model, runtime, repository, and binary before any model output. The planned
first-party sessions can qualify the method but cannot earn
external-participant credit.

The terminal materializer was frozen while the Proposal remained pending. A
pre-execution amendment at root
`sha256:432ba0ac55997130db9b7a4f6004f0ec3bbed7f3e419b4faf2eb75fe0c472c0d`
replaces only a non-reproducible prepared Verification binding discovered
before import, Decision, or model output. The amended materializer source root
is
`sha256:fb458e26e1a0d83efc4622d3c670ed798395d22ca59b5f9e6acb5546e01b70e4`.
It has separate, precommitted accepted and rejected next-action rules and a
shared set of scope limits. It refuses to emit the task instance, answer key,
or amendment unless the exact terminal Decision, scoped Verification,
Registration, source transition, repositories, binaries, runtime, and model
bindings agree. This prevents selection of an answer key after the Decision
outcome or after model output.

### 6.7 Registered benchmark matrix

| Family | Primary | Held-out | Required |
| --- | --- | --- | --- |
| B1 transition bytes | pending | pending | exact |
| B2 affected set | pending | pending | 100% precision/recall |
| B3 route survival | pending | pending | exact |
| B4 authority containment | pending | pending | zero unauthorized delta |
| B5 removability | local qualification pass; correction pending | pending | replay unchanged |
| B6 hosted-service failure | local qualification pass; correction pending | pending | replay succeeds |
| B7 support diversity | pending | pending | exact route accounting |
| B8 second Frontier | fail: portable foreign-reference contract absent | not run | zero imported authority |
| B9 observability | synthetic pass only | pending | no silent truncation |
| B10 cold inheritance | pending | pending | at least 20% median lift |

No aggregate score is reported.

## 7. Threat model and failure modes

### 7.1 Protected assets

- canonical object and Event bytes;
- local Standing and authority history;
- producer and verifier attribution;
- exact task, source, Artifact, and implementation bindings;
- human and repository-authority credentials; and
- explicit completeness and scope limits.

### 7.2 Adversaries and failures

The evaluation includes:

- stale or substituted prior state;
- altered Claims, Verification Records, Artifacts, and relation roots;
- producer or verifier attempts to decide;
- foreign Decisions presented as local authority;
- unknown or semantically rebound relations;
- omitted independent support;
- nondeterministic cycles;
- hidden truncation;
- shortened-digest collisions;
- missing canonical Events;
- mutable URLs substituted for exact source identity; and
- loss of optional services and projections.

### 7.3 Out of scope

Vela does not prevent an authorized reviewer from making a poor scientific
Decision. It does not prove source statement fidelity without an appropriate
check. It does not establish identity in the social or legal sense, recover
lost authority keys, or make globally governed consensus desirable.

## 8. Relationship to existing systems

### Git

Git is a content-addressable filesystem with a version-control
interface [1]. Vela uses that substrate rather than replacing it. Git supplies
decentralized transport, byte history, trees, commits, reachability, and mature
operational tooling. Vela's narrower hypothesis is that typed scientific
assertions, scoped checks, explicit local Decisions, correction relations, and
deterministic Standing provide useful semantics that cannot be recovered
reliably from arbitrary file history alone.

This is an empirical boundary, not an assertion that Git is deficient. Every
user-value experiment therefore compares Vela with **Git plus the same source,
evidence, verifier, and documentation**. A result against a weaker baseline
would not support the paper's claim.

### Artifact identity and reproducible environments

SWHIDs provide persistent, intrinsic identifiers for software artifacts and
their version-control structure using a Merkle DAG [10]. Nix derives immutable
software deployments from functional build descriptions [11]. Vela should use
or interoperate with these mechanisms when they identify source and execution
environments; it should not recreate them.

Neither mechanism gives a scientific assertion local Standing. Conversely,
Vela's Claim roots do not guarantee source availability, rebuild an
environment, or replace an archival identifier. A Vela record may bind those
identities, but the underlying system remains responsible for their semantics.

### Process provenance and research objects

W3C PROV defines interoperable provenance exchange around entities,
activities, and agents [4]. DataLad captures command execution and dataset
changes as provenance-bearing Git history [5]. RO-Crate packages research
artifacts and metadata [6], while Workflow Run RO-Crate profiles describe
workflow execution [7].

These standards are complementary to Vela. They answer where an artifact came
from, how a process ran, and how a portable package is described. Vela asks a
different question: which exact assertion the artifact supports, which
property a Verifier checked, and whether a local Decision changed the
assertion's Standing. Export into an external provenance format must include a
loss report because provenance and authority have different semantics.

### Build and workflow systems

Build systems track computational dependencies and decide what to rerun.
Scientific correction additionally requires statement scope, evidential
relations, plural authority, and explicit unknowns. Vela borrows dependency
discipline but does not make every scientific relation a build edge.

### Supply-chain integrity and update trust

in-toto cryptographically records the authorized steps and materials of a
software supply chain [8]. The Update Framework protects software-update
systems against repository and signing-key compromise using delegated roles
and freshness rules [9]. Vela borrows their discipline of exact inputs,
explicit roles, and signed transitions.

The security goals remain different. A valid supply-chain layout or software
update does not establish that a scientific Claim is correct. Vela likewise
does not replace package signing, update freshness, threshold delegation, or
software-build policy. Repository-authority signatures attest exact local
transactions; they are not scientific truth certificates.

### Append-only and verifiable semantic artifacts

Append-only records make equivocation and erasure detectable. They do not
determine scientific acceptance. Vela's authority layer records who authorized
one local transition and retains prior state. Trusty URIs demonstrate that
cryptographic identifiers can make linked-data artifacts and their reference
trees verifiable [13]. Nanopublication-style decomposition is a useful
precedent for small attributed assertions, but cryptographic verifiability
still does not define local scientific Standing.

### Formal verification

Lean 4 is an interactive theorem prover and programming language with a small
trusted kernel and extensible automation [12]. A kernel can establish that a
term checks against an exact formal statement under declared axioms. That
result remains separate from statement fidelity, empirical support, importance,
and local acceptance. The prospective Erdős 424 case exists precisely because
an exact source-statement correction matters even when no proof term changed.

### Artifact evaluation and publication review

ACM's artifact-badging policy separates artifact availability, functional or
reusable evaluation, and independent validation of results [14]. USENIX
artifact evaluation likewise treats artifacts as separately assessed
companions to a paper and asks whether they conform to the paper's claims
[15]. This separation is an important precedent for Vela's insistence that a
Verification Record is not a scientific Decision.

Vela does not replace peer review, publication, or an artifact-evaluation
committee. Its narrower role is to retain the exact objects and local
transitions so that those institutions can be inspected and replayed without
collapsing their distinct judgments.

### Boundary summary

| Existing layer | It establishes | It does not establish for Vela |
| --- | --- | --- |
| Git / SWHID | exact byte or software-object identity | Claim meaning or Standing |
| Nix / workflow engine | reproducible environment or execution | scientific acceptance |
| PROV / DataLad / RO-Crate | provenance and portable run metadata | local authority semantics |
| in-toto / TUF | software supply-chain or update integrity | scientific correctness |
| Lean kernel | proof checks against an exact formal statement | statement fidelity or importance |
| artifact evaluation | artifact availability, function, or reproduced results | a Frontier's local Decision |

The registered benchmark is meaningful only if Vela's added column produces
measurable value after all existing layers are held constant. That column
contains an exact Claim, scoped Verification, local Decision, correction
lineage, and replayable Standing.

## 9. Limitations

The current corpus is mathematical and first-party. It contains no qualifying
historical correction fixture. The synthetic graph fixes relation meanings in
advance and cannot show that real scientific repositories encode them
correctly. The clean-room implementation is colocated and does not establish
organizational independence.

Repository authority remains operationally heavier than ordinary producer
authentication. The benchmark has not yet measured reviewer time, cold
inheritance, federation, or recurring external use. SHA-256 and Ed25519 are
assumed secure. Git hosting availability is operationally useful but not
canonical.

The current public contract cannot express the registered non-escalating
second-Frontier transfer. This revision fails a protocol benchmark; product
polish cannot repair it. Any repair must remain smaller than a hosted
federation system and must be justified by independent readers over the
terminal real correction.

If the real correction, second-Frontier, held-out, or cold-use gates fail, the
paper must report that failure and narrow its claim. A failed correction-impact
reader should be deleted rather than promoted into a protocol primitive.

## 10. Reproducibility

Current implementation qualification:

```bash
cargo test -p vela-edge --test correction_impact
python3 conformance/verify_correction_impact.py
python3 -m unittest paper/artifacts/state-lift/test_score.py
./conformance/check-core.sh
```

The working source-only artifact builder refuses dirty Vela input and mismatched
external commits, trees, or content roots. At Vela commit
`79c3d3e6d777f7734b4a82b9e82cd0c53dec4ba5`, two independent invocations
produced identical 428-member archives at root
`sha256:bad77d78c47f14799422965ce4742b01ec5f0bbca73e19ad5b99044595b45e0e`
and manifest root
`sha256:a2aee9988d2d1b6692a360092ebcbdc402964fa7e708835be06e66a047d5a0a4`.
The verifier rehashed every member and rejected unmanifested paths. This is
packaging qualification, not independent reproduction or the final release
artifact.

The same source rendered twice from clean Vela commit
`94a9450be33cb51497ca0c7700826c8e384f49e0` with pinned Pandoc 3.9 and
pdfLaTeX 1.40.26. Both 11-page PDFs had root
`sha256:b7e0c01e208b68d680d7218b29804552e4edbe2bac2ece0ba6f313a847059b6a`.
The renderer derives PDF timestamps from the Git commit and wraps exact roots
at presentation time without changing source bytes. This qualifies
deterministic rendering, not the paper's scientific claims.

The final artifact package will include:

- frozen plans and amendments;
- exact primary and held-out fixture bytes;
- source commits, trees, object roots, and verifier identities;
- Rust and clean-room sources;
- raw outputs and timing data;
- scripts for every result table and figure;
- threat-model and authority audits; and
- a source-only manifest with SHA-256 for every member.

## 11. Conclusion

Vela currently demonstrates a compact, replayable separation among
authenticated producer input, scoped Verification, local human Decision, and
scientific Standing. Two implementations also agree on a bounded synthetic
correction-impact projection. The current revision lacks the portable
foreign-reference contract required for non-escalating second-Frontier
transfer. The stronger claim, that this mechanism preserves useful scientific
inheritance across real corrections and plural authorities better than Git
alone, is therefore not satisfied by the audited system.

The registered experiments decide the conclusion. If they pass, the paper
will state the exact measured result. If they fail, the paper will publish the
failure and retain only the mechanisms that remain useful.

## 12. References

1. Scott Chacon and Ben Straub. “Git Internals: Plumbing and Porcelain.”
   *Pro Git*, 2nd edition. [git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain).
2. Anders Rundgren, Benjamin Jordan, and Samuel Erdtman. “JSON
   Canonicalization Scheme (JCS).” RFC 8785, 2020.
   [rfc-editor.org/rfc/rfc8785](https://www.rfc-editor.org/rfc/rfc8785.html).
3. Simon Josefsson and Ilari Liusvaara. “Edwards-Curve Digital Signature
   Algorithm (EdDSA).” RFC 8032, 2017.
   [rfc-editor.org/rfc/rfc8032](https://www.rfc-editor.org/rfc/rfc8032.html).
4. Timothy Lebo, Satya Sahoo, and Deborah McGuinness, editors. “PROV-O: The
   PROV Ontology.” W3C Recommendation, 2013.
   [w3.org/TR/prov-o](https://www.w3.org/TR/prov-o/).
5. Yaroslav O. Halchenko et al. “DataLad: distributed system for joint
   management of code, data, and their relationship.” *Journal of Open Source
   Software* 6(63), 3262, 2021.
   [doi.org/10.21105/joss.03262](https://doi.org/10.21105/joss.03262).
6. RO-Crate community. “RO-Crate Metadata Specification 1.3.”
   [researchobject.org/ro-crate/specification.html](https://www.researchobject.org/ro-crate/specification.html).
7. Workflow Run RO-Crate working group. “Workflow Run RO-Crate.”
   [researchobject.org/workflow-run-crate](https://www.researchobject.org/workflow-run-crate/).
8. Santiago Torres-Arias, Hammad Afzali, Trishank Karthik Kuppusamy, Reza
   Curtmola, and Justin Cappos. “in-toto: Providing farm-to-table guarantees
   for bits and bytes.” *USENIX Security 2019*.
   [usenix.org/conference/usenixsecurity19/presentation/torres-arias](https://www.usenix.org/conference/usenixsecurity19/presentation/torres-arias).
9. The Update Framework contributors. “The Update Framework Specification.”
   [theupdateframework.io/spec](https://theupdateframework.io/spec/).
10. SWHID Working Group. “SoftWare Hash IDentifier Specification.”
    [swhid.org/swhid-specification](https://www.swhid.org/swhid-specification/).
11. Eelco Dolstra, Merijn de Jonge, and Eelco Visser. “Nix: A Safe and
    Policy-Free System for Software Deployment.” *LISA 2004*.
    [usenix.org/publications/library/proceedings/lisa04/tech/full_papers/dolstra/dolstra.pdf](https://www.usenix.org/publications/library/proceedings/lisa04/tech/full_papers/dolstra/dolstra.pdf).
12. Leonardo de Moura and Sebastian Ullrich. “The Lean 4 Theorem Prover and
    Programming Language.” *CADE 28*, 2021.
    [lean-lang.org/papers/lean4.pdf](https://lean-lang.org/papers/lean4.pdf).
13. Tobias Kuhn and Michel Dumontier. “Trusty URIs: Verifiable, Immutable, and
    Permanent Digital Artifacts for Linked Data.” 2014.
    [arxiv.org/abs/1401.5775](https://arxiv.org/abs/1401.5775).
14. Association for Computing Machinery. “Artifact Review and Badging.”
    [acm.org/publications/policies/artifact-review-and-badging-current](https://www.acm.org/publications/policies/artifact-review-and-badging-current).
15. USENIX. “NSDI '26 Call for Artifacts.”
    [usenix.org/conference/nsdi26/call-for-artifacts](https://www.usenix.org/conference/nsdi26/call-for-artifacts).
