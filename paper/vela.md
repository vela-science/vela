---
title: "Vela: Replayable Scientific Standing from Bounded Evidence"
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
which separately implemented readers must derive the same bounded correction
impact, preserve an independent support route, reject authority escalation,
transfer exact state into a second locally governed Frontier, and improve cold
continuation over Git plus the same evidence. The pre-campaign audit of four
mathematical Frontiers found no qualifying historical correction fixture among
2,831 retained Claims. Rust and clean-room Python readers agree on a synthetic
qualification vector, but that result does not satisfy the scientific,
federation, external-reproduction, or user-value gates. The current evidence
also includes one accepted, first-party source-statement supersession and one
failed matched first-party Codex-session pilot. Neither has the consequential
topology or external independence required by the registered benchmark. The
frozen held-out selector subsequently found no qualifying candidate, so confirmatory
two-fixture credit is unavailable in this campaign. The evidence therefore
supports local admission, scoped verification, Decision, replay, and
supersession. It now also supports one bounded, first-party cross-Frontier
transfer with exact receiver Verification, clean-clone replay, and zero
authority escalation. A separate prospective product loop derived a 4,032-node
map from an exact compact repository, selected its first Target, retained and
verified the resulting bounded Run, accepted the exact bounded Claim through a
human Decision, clean-clone replayed the transition, and rebuilt a read-only
candidate map. The map cardinality did not change, while the Claim Standing,
repository root, graph roots, and projection root changed exactly. The Target
packet initially failed to advance, so that candidate was not activated.
Source-local closure and three later exact completions now close producer work
through `10430200` and expose `10430201..10430400` as the next nonduplicate
range. The released Vela Web `v0.430.0` four-Frontier Atlas contains 4,147
graph nodes. Three corrected bounded Erdős results, one current Formal result,
and the quantum-certificate result remain pending human Decisions; one
broader-worded Erdős predecessor Proposal remains retained separately rather
than being rewritten. These facts do not establish acceptance, inheritance
lift, external independence, or cold-use value. The evidence therefore does
not support the full registered protocol claim.

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

At present, the first contribution, one bounded cross-Frontier transfer, and
synthetic implementation qualification are demonstrated. One exact
map-to-target loop reached a terminal human Decision, replayed Standing, and a
deterministic remap. The loop reproduced a stale-Target failure that later
source-local closure and exact completions repaired. The exact four-Frontier
Atlas is now released, but real correction-impact propagation, post-Decision
remapping of the current pending results, held-out confirmation, external
independence, and cold-use lift remain registered experiments.

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
| Verification Record | Scoped result over exact Claim, Submission, Proposal, Artifacts, and implementation | verifier attribution |
| Proposal | Pending requested transition | none |
| Decision | Accept, reject, or withdraw one exact Proposal | local reviewer |
| Event | Append-only semantic transition | local Frontier |
| Standing | Deterministic replay result | derived from local history |

### 2.4 State transition

Let `S` be a valid Submission, `V*` its retained Verification Records, `P` the
pending Proposal, and `D` an authorized local Decision.

```text
submit(S) -> pending(P), accepted-event delta = 0
verify(V, S, P) -> retained(V), accepted-event delta = 0
decide(D, P, V*) -> append(Event), replay Standing
```

Submission and Verification cannot accept a Claim. A Decision cannot omit
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

### 3.5 Transition validation

The reference implementation separates validation from publication. Given a
Frontier `F` and proposed transaction `T`, it derives the complete post-state
before installing any canonical bytes:

```text
VALIDATE-AND-DERIVE(F, T):
  require T.before_repository_root = ROOT(F)
  decode every input under its declared closed schema
  recompute every full object root and resolve every retained reference
  verify producer and verifier signatures over their canonical preimages
  replay the authority keyset and policy to T.before_authority_head

  if T contains no Decision:
    require accepted-event delta = 0
    require every proposed Claim remains pending
  else:
    require one exact pending Proposal
    bind its Claim, Submission, ordered Verification Records, action, and reason
    verify the reviewer principal and local policy at the bound authority head
    derive exactly one semantic Event

  replay the ordered Event log and derive Standing
  derive every canonical postimage and the next repository root
  require T's declared postimages and root equal the derived values
  return the immutable write plan
```

Publication executes that immutable plan through a recoverable Git/filesystem
transaction. A failure before its commit marker installs no canonical
postimage. Recovery after the marker installs the same bytes; it does not
rerun scientific judgment.

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

### 4.4 Cost bounds

Let `b` be the total canonical input bytes, `n` the number of retained
objects, `r` the number of retained references, `s` the number of signatures,
and `e` the number of accepted Events. Full strict replay must inspect every
canonical byte and Event, giving a lower bound of
`Omega(b + n + r + e + s)`. For already decoded objects, one root-indexing and
reference-resolution pass with the reference implementation's ordered maps and
sets costs `O(b + (n + r + e) log n + s)` time and `O(b + n + r)` working
state. This cost model excludes Git object access, schema-specific checks,
operating-system caches, and repeated full-state derivations.
Incremental transaction validation replaces those totals with the exact
transaction read set and postimages, but the final repository-root check still
binds the complete declared state.

For the experimental correction reader, let `c` be the number of Claims and
`m` the number of causal relations in the declared closed slice. The current
fixed-point implementation stores at most `O(c m)` propagated
Claim--relation causes. Its conservative worst-case bound is
`O(c^2 m^2)` time and `O(c m)` memory because a pass scans the relation set
and compares the monotone cause state. This is adequate only for bounded
fixtures; it is not a claim to provide a general graph engine. A future queue
implementation would require byte-equivalent conformance evidence before
replacing this reference behavior.

The registered cost observation used one arm64 macOS machine, Vela 0.940.9 at
binary root
`sha256:b4b85550aed52134ad2e21a3b1a163390ca1f16673811274b55b3b0f2089ed9c`,
an empty home, no credential environment, and a network-denied process
sandbox. Plan root
`sha256:59e400d03b794736c673443f40abcb783e6f9d70e3454502ebd4c639119f8e24`
fixed one warmup followed by seven retained warm-cache samples. The terminal
Erdős 424 Decision preceded the run.

| Frontier | Tracked bytes | `status` median | strict median |
| --- | ---: | ---: | ---: |
| Erdős | 40,672,878 | 1,443.580 ms | 1,463.935 ms |
| Formal | 200,725 | 200.597 ms | 191.449 ms |
| Sidon | 8,133,169 | 194.618 ms | 159.933 ms |
| Quantum | 90,770 | 54.029 ms | 47.502 ms |

Erdős Proposal inspection measured a 1,479.464 ms median. Frozen-witness
reproduction measured 1,583.397 ms. Result root
`sha256:1ba33ce4387c624c7c0381091140db34bb7ff4bf933ce56d0abe5479cf495acd`
retains all samples, normalized outputs, exact source heads, repository roots,
tracked inventory, and machine identity.

A second execution from detached clean clones matched the plan, binary,
source heads, repository roots, counts, tracked inventory, normalized output
roots, and limits. The shared deterministic projection root is
`sha256:f30d4c3464618e0159603ae8adaf58eb7addd63a4ce00f7a1d3fec18d2f85bd3`;
the second raw result root is
`sha256:8ee2588e3745324555862a14a7559d2374984661aa5ce783d6ed7c400b02599b`.
Both executions used the same machine, operator, and implementation. The
numbers describe warm-cache local cost. They exclude clone, network,
compilation, model execution, and human review, and support no independent or
cross-machine performance claim.

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

The released Math Atlas read model keeps source semantics at the boundary.
Each native source has its own adapter for identifiers, revisions, pagination,
deletions, rights, snapshot policy, and completeness. Adapters emit immutable
rooted observations whose identity does not depend on a web release. A release
references those observations and stores separate Frontier bindings for
reference, snapshot, or local admission. Rebuilding a website therefore does
not relabel native source bytes or transport Standing.

The projector loads candidate releases with PostgreSQL `COPY FROM STDIN` in
bounded chunks, verifies counts and table roots, and then moves one atomic
release pointer. Collection reads use release-bound keyset cursors. Graph reads
return bounded typed neighborhoods and an equivalent ledger rather than the
full graph. Neon has one durable production branch; migration and benchmark
branches are temporary and deleted after use.

This design does not justify a scalability claim by itself. The alpha requires
a rooted 100,000-record ingestion and read benchmark. A separate
1,000,000-record benchmark must pass before the paper describes the Atlas as
scalable. Table partitioning, a graph database, vector or embedding
infrastructure, streaming ingestion, or a second read store requires a measured
failed budget in the simpler PostgreSQL implementation.

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

At the pre-campaign audited clean commits, four mathematical Frontiers retain
2,831 Claim records and no accepted correction, supersession, or retraction
relation.

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

**Current status:** the exact Submission was retained as
`vsb_44cd52724425171f` at root
`sha256:4cd059848ce06c943e2cafffac0ffa0f14838b5adba022bc4c076df6acc5af12`.
Its replacement Claim `vcl_4bc14401b203218cb7b9de0141747e0c17cea3a6b0cc522639323ab13e432eaf`
entered Proposal `vpr_23f32f95d4f073e8`; the Submission changed no accepted
state. A deterministic source verifier reproduced both file roots and the exact
Git diff in two object-database contexts. The first verifier draft was rejected
before import because default Git diff output abbreviated blob IDs differently
across those contexts. The repaired implementation forces full blob identities;
its signed first-party Verification
`vvr_ed3383c1cd640d43` was imported with outcome `pass` and accepted-event
delta zero at Frontier commit
`b696ececbf1dfb249dadbbc86f211e9445a09cc6`, repository root
`sha256:b70da05f7fdb93925dc2fed3d7a680b65ef3ac6d68ed51cd2985bd61c1b06cb9`.
The human then accepted that exact Proposal. Event
`vev_c9edac512e2b3307`, root
`sha256:1562259dcbb48e03bf9850da2e2f7b7e145b4cca544c056e8271c281f0cfae23`,
records the terminal Decision, and applied Event `vev_7b5ae15a99689064`
supersedes the predecessor without erasing it. Strict verification and a clean
clone reproduce repository root
`sha256:391c2acb12ea1251b6614803d973fd7785826977b664bebcd7091d261133d8fc`
at commit `c25e11d332cfbc12b048c314880662d507df53e0`. This first-party
qualification proves the current writer, verifier, Decision, and replay path.
It earns no external-participant credit and lacks the consequential diamond,
surviving route, second Frontier, and cold-successor evidence required by the
primary benchmark.

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

The source-only selector was then run against the four exact current heads and
their retained compaction predecessors. It found one accepted transition after
the baselines: the Erdős 424 writer-qualification case. That case is excluded
from held-out use and has no hard dependent, support diamond, or
non-consequential incoming relation. No other Frontier supplied a candidate.
The deterministic result at
`paper/artifacts/heldout-selection/result.v1.json`, byte root
`sha256:f80cf6b81c9b056535ccf17a24b1631d8f3e57d3bc3ecea65d7516c1b831be5b`,
therefore records `no_qualifying_candidate`. The held-out entry gate failed;
the experiment did not substitute a synthetic case.

### 6.5 Living map-to-target qualification

The prospective product loop freezes one read projection before work. Erdős
commit `43c7a1418ccd16c304a3c9c0e62ba0ead26d06ab`, repository root
`sha256:8a98ff1c632232c7b227d87a0f1015aaa3429d38c83592ca66f8e465b06b0ee5`,
produced graph source
`sha256:a6d6d50d56a0c9c2955716998a2adc8387661e0772891fdad56af7434bb15e51`
with 4,027 nodes, 2,524 edges, 1,217 problem records, and 2,771
Claims. The graph layout is derived and non-authoritative. Its first work
offer was exact Target `erdos:1056`, binding the previously uncovered
inclusive range `10429401..10429600` and packet root
`sha256:6d1a2ca87851deb1fa2133f4f6cf7edb28ee843cb0eef57ea09e826b3fdca63b`.

Canopus retained Run `run_8510dd67-c1d7-4c0a-9724-f87192d1a709`.
Submission `vsb_b8ebcd819ac327db` created Proposal
`vpr_80759f390c4880c0` and bounded Claim
`vcl_5c740ebb758107f25179b096d9e1b680d0bc62186eb276c8b907a2c1226fd979`
without changing accepted state. Released Vela 0.950.1 imported scoped
Verification `vvr_eb80b766c730513b`, root
`sha256:47b5e299e93d298e31da2b4c3c9352855b37e07500f03208eb7851efb4c24ea9`,
with outcome `pass` and accepted-event delta zero. Strict and clean-clone
replay agree at Erdős commit
`606f2f4b50193b1feccf1df4e1f31d50d3a8dd99`, repository root
`sha256:8b1c2bbc99b9e9aade2bfb56d3493be02cdad954eefa3cd98a14ac41128ae0d4`.
The shared first-party operator and machine earn no independent-participant
credit.

The post-Verification candidate map has release root
`sha256:fb2665dfaac61f4ba61d11cd4e7ea65421168bb292bf5f7a840ce3207599af02`,
graph source
`sha256:51e69b4d883f89c38b590ac8750753834f96f5abd073cba8ca87bddad7dbb659`,
4,032 nodes, and 2,528 edges. It was inserted and verified in the normalized
read projection but not activated. The exact checkpoint bytes are retained at
`paper/artifacts/map-target-loop/post-verification-map.v1.json`, root
`sha256:439a804908890e4029922cc91cdd0a79122187d573530fc760a419d90786be21`.

The human accepted the exact bounded Claim. Decision Event
`vev_51abc098046d3423`, root
`sha256:fb48df2660288285a8dd838e94e1969cef6da95a13a9f7b483641c7f54d1006e`,
references applied semantic Event `vev_7fa17589c00dd62b` and is covered by
authority record `var_ac64e1806e2a18b0`, root
`sha256:769e2812ed6798f023152b9ff8370069a574670fc08f2062e3e7f7bea6d05504`.
The exact Decision commit
`80606bdccb51fa86524111a1a61876bb08e45d79` clean-clone replays at repository
root
`sha256:9679827bc76de9f6433bfafa8e2e966b9780ca1273c7948d97c2ae042f5cab1a`.
One Claim moved from pending review to accepted; Verification alone changed no
Standing.

The source-only materializer then required that one exact Decision commit,
physical authority-event coverage, semantic applied-event identity, strict and
fresh-clone replay, and a dry-run remap. It produced candidate release
`sha256:d0fc41a9e2d37b798975caa5b7f06a78e674e9da7e42b27f89685b6841558ff6`,
graph source
`sha256:d22eddc7b486907204fc8197c4861e089808523c2993f3864ea1a46f8465cfcc`,
4,032 nodes, and 2,528 edges. Exact terminal evidence is retained at
`paper/artifacts/map-target-loop/post-decision.v1.json`, root
`sha256:b29e8cbb50aff3cc81a4ac6f4cf261b9a3ca9d80dbe69614d9a771116d80151c`.
The materializer cannot invoke a Decision, push Git, activate the read model,
read a human key, or mutate a Frontier.

The initial loop did not produce a safe next handoff. The first offer still
bound packet root
`sha256:6d1a2ca87851deb1fa2133f4f6cf7edb28ee843cb0eef57ea09e826b3fdca63b`
for the completed range, so the first remap candidate remained inactive.

A subsequent source-local closure pass repaired that defect without changing
Standing. Erdős commit
`f2f4a4f5d5c322f5c57f99d100fce97333f7aeb1` retains closure envelope
`sha256:14e93d4cdbb65ddb3b389f8ea219b74340beca6014381fefe88deb64d2e59d7e`,
revalidates the completed packet from exact ancestor Git bytes, and publishes
Target Index
`sha256:84314593f22bbeae251090838273394db6685f7f437070154a085b1f403b7fd3`.
At that checkpoint, the first actionable offer was the contiguous range
`10429601..10429800`, packet root
`sha256:8d879e24a537de3b9b13ad7878dc98db8ce4f5273187c7f45d0d49a93e8fe8ad`.
Formal commit `35fa12bb4115e1561b0865722580a7626ee79016`
similarly closes the already-verified foreign-reference-retention Target with
local Standing effect `none` and exposes zero current offers. This establishes
source-local Target progression for the two demonstrated stale edges. It does
not establish cold-user lift or automatic domain-generic closure.

Subsequent exact completions close Erdős producer work through `10430200` and
leave `10430201..10430400` as the next nonduplicate range. Vela Web `v0.430.0`
now serves the exact current four-Frontier Atlas with 4,147 graph nodes. The
three corrected bounded Erdős results, the current Formal exact-proof result,
and the quantum-certificate result all remain pending human Decisions; their
Verifications do not change Standing. The earlier broader-worded Erdős
Proposal remains retained and pending rather than replacing the corrected
bounded record.

### 6.6 Removability qualification

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
cold-user value, or establish independent reproduction. It proves replay
removability for this one historical pending state, not inheritance lift.

### 6.7 Foreign-transfer contract audit

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

This is a negative result: the audited protocol alone cannot pass B8. It does
not show that a hosted Registry, resolver, global namespace, or federation
service is needed. Rust and clean-room Python readers independently reproduced
the same current-contract inventory from compiled public types and public
source. Their shared negative-inventory fixture root is
`sha256:fffb66f5afa69b8a47824a45bc382d97e4c655dc282a3b3d6ba339f2b70906ae`.
That is implementation diversity, not organizational independence. A minimal
derived envelope became eligible after the real correction reached a terminal
Decision. The resulting non-protocol `vela.foreign-reference.v1` experiment
binds the compacted repository, predecessor transition and origin, Claim,
signed Submission, Proposal, signed Verification, applied and Decision Events,
authority record and keyset, retained object set, completeness, and explicit
local Standing effect `none`.

The real Erdős reference has 11 required objects, reference root
`sha256:b7b330ae6ea4915d5bac218233f0a272ee961060682be6d22f6a8ea1b78c4ed6`,
and object-set root
`sha256:f9cc936b42f7ee624d98583332454dbb46b68c00fa2819d990cea4d6d7daec8a`.
Both readers rederive every identity, verify producer and verifier signatures,
follow the compaction origin, and verify the repository-authority DSSE
signature against the retained keyset. They reject authority escalation,
silent truncation, source and semantic substitution, object-byte drift, path
and symlink escape, and signature tampering. Formal Conjectures has retained
the exact archive through corrected Submission `vsb_bb9b64f5d93b8cad` and
pending Proposal `vpr_7aba66544ffefd99` with accepted-event delta zero.
A credential-free import preflight verified the signed receiver record, every
exact subject, repository, authority history, trust anchor, Cedar
authorization, and canonical transaction set, then stopped at signing without
changing Git or operation journals. Its result root is
`sha256:00b135a27088af1049ffe86cc329a5bec10fde098e32ac8342900c84a8a95c09`.
Repository authority then imported scoped Verification
`vvr_ebc29eae4f5f4edf`. Remote commit
`3fe6bf62afd587b9cdeac39f5eb3c62a28fbc0aa` reproduces repository root
`sha256:5e59e05a5639ac0ec4331ec40fec9f50229b795a1a08d983ba96834d4777b58a`
from a clean clone. The Proposal remains `pending_review`, no Decision exists,
and local accepted Standing is unchanged. Receiver audit root
`sha256:a5867554d4dc9ea4dcd6d415a2be263c84dc0f6fbbe497fb86b427104368d75c`
therefore passes B8 and authority non-escalation. It does not demonstrate
external independence, performance lift, or readiness for a Registry or
Atlas.

The matched Git-versus-Vela state-reading protocol is frozen at root
`sha256:68fb039088302d19f02cf2628c16004e174649b1f952a63a0fd35210c0dd0ef8`.
It gives both arms identical terminal repository, evidence, verifier, Git, and
documentation bytes; only the Vela arm receives the exact read-only CLI. A
dependency-free exact-field scorer is frozen at source root
`sha256:c949f9dec835cbe97bb89a22ccc006e8e767f88b86dfc6c4a0083732ff3fcd63`.
It rejects task-instance drift and reports false authority statements as hard
failures.

The subjects were first-party Codex model sessions, not human reviewers. This
protocol can debug the task and scorer but cannot establish human
comprehension, reviewer efficiency, or external adoption.

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
historical intake receipt, source transition, repositories, binaries, runtime, and model
bindings agree. This prevents selection of an answer key after the Decision
outcome or after model output.

The first matched pair is a registered negative result, root
`sha256:af9af17824e15b14ea77aa2e9afec135b997cdcf026beb050b80cc51563e753a`.
The Git-only arm answered 22 of 25 exact fields in 268.040 seconds using
2,401,939 observed tokens. The Vela arm answered 24 of 25 in 146.425 seconds
using 655,122 observed tokens. Vela was directionally better by two fields,
72.725 percent fewer observed tokens, and 45.372 percent less wall time.
However, both arms exceeded the preregistered 50,000-token hard limit and
neither answer was fully correct. The registered study therefore failed; the six
remaining repetitions were not run. This pilot establishes neither state lift
nor external user value. It identifies a narrower product task: reduce the
question surface and remove evidence ambiguity before registering another
state-reading study.

### 6.8 Formal vertical slice

A separate product slice tested the ordinary math path rather than the
correction-impact benchmark. Canopus Run
`run_585c951f-ed51-49b9-805d-02e7e5a8a0e9` produced a Lean proof Artifact
under a frozen Lean 4.27.0 and Mathlib environment after four retained failed
Runs. Exact replay through the network-denied verifier reproduced kernel
elaboration and an axiom set limited to `propext`, `Classical.choice`, and
`Quot.sound`.

Submission `vsb_c50dc7e85cb76684`, root
`sha256:9adecb4649fa99a7b0945e99f3197cb72489e17b4bd08fe2bfcdff7d0f1c67d3`,
created Proposal `vpr_6c71e12b28f095c9` without changing accepted state.
Scoped Verification `vvr_a898f5218acb57e9`, root
`sha256:70a2f95366d1f9e55fa46c84d3ffa61f54d957427cdf0bf282017a5d26b324a4`,
replayed the exact Run and retained its limitations: kernel acceptance does
not establish informal statement fidelity, novelty, importance, or scientific
acceptance. Strict replay and a clean clone agree at Formal Frontier commit
`84d3064cd7d9170985d04360b579c3c45fc96a80`, repository root
`sha256:66f2244045500eb5838d116a57ef16499b96775d76a77c9e383e8b322734ceab`.
At that retained predecessor commit, the Proposal was pending and
accepted-event delta was zero. The current compact Formal epoch at commit
`35fa12bb4115e1561b0865722580a7626ee79016` does not carry this Proposal as a
current record. It remains recoverable through tag
`pre-compaction/84d3064cd7d9`, archive root
`sha256:88cc1b73546806aefb85839fbed5ae9181dc5a98ac44844567b5ced0ce83e4d1`,
and equivalence-report root
`sha256:2b96ac51fe3d6d9adca0d2394f43b8e07d294c6c8d5f06de5d843bee93a2b455`.
This is historical vertical-slice evidence, not a current pending Proposal.

This slice also reproduced an object-contract mismatch. Current repositories
identify retained Artifacts by full content hash, whereas the optional
Verification Record v1 `artifact_ids` field accepted only legacy `va_`
identifiers. The imported Verification binds the exact Submission root, which
already binds all three Artifact digests, and leaves the redundant list empty.
A compatible repair now accepts exact full lowercase content-hash identifiers
while retaining historical `va_` replay and exact repository-membership
checks. The imported record remains byte-identical; no alias or canonical
history rewrite was required.

### 6.9 Framework-neutral execution evaluation

Stage A was a registered repair replication, not an untouched preregistered
comparison. Three earlier plans stopped on their registered token ceilings.
Completed plan
`sha256:9fbfd54fd40db4e2b02ded895365468c596f54e4dc0e19ae09630a80fb28125b`
then produced 12 outcomes with six verifier passes. After those outputs
exposed an answer-free task-contract defect, repair plan
`sha256:31268241f0f1ada92fd78d245643ad9274308a74d617a965e8e2bcb46195fd47`
amended that completed plan and registered 12 fresh assignments. The repaired
run held task facts, verifiers, model family, custody rules, and publication
rules constant across Canopus, native Codex, and native Codex with the same
packet. Two tasks and two fresh repetitions per arm produced 12 retained
outcomes. Eleven passed their frozen verifier. The repaired report root is
`sha256:43be5378169f2911eb09773b2a9ffbbf8364c080e37dc874b13188ccff144bfb`.

| Arm | Passes | Observed tokens | Wall time | Passes / 1M tokens | Passes / hour |
| --- | ---: | ---: | ---: | ---: | ---: |
| Canopus | 4 / 4 | 853,514 | 324.375 s | 4.687 | 44.393 |
| Native Codex | 3 / 4 | 1,207,939 | 616.799 s | 2.484 | 17.510 |
| Same-packet native | 4 / 4 | 2,253,673 | 449.045 s | 1.775 | 32.068 |

Canopus improved passes per observed token by 88.7 percent over native Codex
and 164.0 percent over same-packet native. It improved passes per hour by
153.5 percent and 38.4 percent, respectively. This is two repetitions per task
and arm, all first-party. It supports baseline utility for the bounded runner,
not a stable population estimate, external adoption, or the protocol claim.
The report does not score all-in cost or expert-minutes, so it does not satisfy
the campaign's primary execution-lift metric. The stopped runs, the six-pass
completed predecessor, and the repaired replication must all ship in any
public artifact package.
The registered call ceiling could not fit Stage B, so no orchestration
framework was evaluated or retained.

### 6.10 Registered benchmark matrix

| Family | Primary | Held-out | Required |
| --- | --- | --- | --- |
| B1 transition bytes | writer qualification pass; primary pending | entry gate failed | exact |
| B2 affected set | pending | entry gate failed | 100% precision/recall |
| B3 route survival | pending | entry gate failed | exact |
| B4 authority containment | writer qualification pass; primary pending | entry gate failed | zero unauthorized delta |
| B5 removability | writer qualification pass; primary pending | entry gate failed | replay unchanged |
| B6 hosted-service failure | writer qualification pass; primary pending | entry gate failed | replay succeeds |
| B7 support diversity | pending | entry gate failed | exact route accounting |
| B8 second Frontier | exact receiver retention, scoped Verification, and clean-clone replay pass with zero accepted delta | entry gate failed | zero imported authority |
| B9 observability | synthetic pass only | entry gate failed | no silent truncation |
| B10 cold inheritance | first pilot failed hard budget | entry gate failed | at least 20% median lift |

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

### 7.3 Containment and residual risk

- **Producer:** a producer can submit a false or overstated Claim. Whole-body
  authentication preserves attribution, closed schemas reject substitution,
  and Submission leaves Standing unchanged. Only scientific review can
  address a plausible but false Claim.
- **Verifier:** a verifier can run an unsound method or lie about its result.
  The record binds the verifier, exact scope, implementation, environment, and
  nonclaims, while Verification has no authority. Vela does not prove the
  verifier implementation sound.
- **Repository writer:** a writer can publish an authorized transaction but
  cannot supply the separate reviewer Decision required for Standing.
  Corrupt or incomplete postimages fail strict replay. Compromise of both
  roles is equivalent to compromise of that Frontier's local governance.
- **Reviewer:** an authorized reviewer can make a poor or malicious Decision.
  Vela preserves its exact inputs, actor, reason, and resulting Event; it
  cannot make the judgment scientifically correct.
- **Publisher or Git host:** a host can censor, delay, or serve an old valid
  history. A reader who retains a known head can detect substitution and
  rollback. Without that head, the reader cannot distinguish a stale
  authentic clone from the latest authentic clone.
- **Read model:** a database or website can omit or misrender records. Readers
  bind projections to canonical Git roots, and canonical replay does not
  require the projection. Users can still be misled if they never inspect its
  source root.
- **Foreign Frontier:** foreign state must be attributed information with no
  imported local authority. A derived edge-layer envelope passes real-source
  semantic, signature, and containment checks without changing the protocol.
  Formal Conjectures retained it as a pending local Proposal, imported a
  scoped Verification, and reproduced the resulting root from a clean clone
  with zero accepted-state delta. B8 passes; external independence and
  performance lift remain unestablished.
- **Implementation:** Rust and clean-room Python reduce shared-code defects but
  share the same specification, fixtures, operator, and project incentives.
  Agreement is implementation diversity, not organizational independence.
- **Credential compromise:** signatures identify the compromised key, not
  human intent. The current paper assumes SHA-256 and Ed25519 security and does
  not claim automatic recovery from a stolen reviewer or repository-authority
  credential.

### 7.4 Out of scope

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

The current corpus is mathematical and first-party. It contains one accepted
source-statement supersession, but no qualifying correction fixture with the
registered consequential topology. The synthetic graph fixes relation meanings
in advance and cannot show that real scientific repositories encode them
correctly. The clean-room implementation is colocated and does not establish
organizational independence.

Repository authority remains operationally heavier than ordinary producer
authentication. The first state-reading pilot used first-party Codex model
sessions and failed its hard token budget; it therefore does not establish
human reviewer-time or cold-inheritance lift. The
benchmark has not measured federation or recurring external use. SHA-256 and
Ed25519 are assumed secure. Git hosting availability is operationally useful
but not canonical.

The Math Atlas has passed a rooted, bounded 100,000-record Neon ingestion and
read qualification on the existing schema, and Vela Web `v0.430.0` serves an
exact 4,147-node first-party Atlas. No 1,000,000-record result exists. These
results establish bounded reconstruction and operation, not general
scalability, adoption, or user value. Current counts describe the audited
corpus, not a capacity limit or a reason to add partitioning, a graph database,
or vector infrastructure.

The public protocol has no foreign-reference object. The historical evidence
envelope passed the real Erdős source qualification in two implementations,
and a second Frontier retained and verified the terminal source correction
without importing authority. This passes B8 for one first-party case. The
reusable Rust reader was subsequently removed from the current runtime because
the experiment did not earn a supported adapter contract. It does not
establish external independence, recurring transfer, or value over a plain
rooted manifest.

If the real correction, second-Frontier, held-out, or cold-use gates fail, the
paper must report that failure and narrow its claim. A failed correction-impact
reader should be deleted rather than promoted into a protocol primitive.

## 10. Reproducibility

Current implementation qualification:

```bash
cargo test -p vela-edge --test correction_impact
python3 conformance/verify_correction_impact.py
python3 paper/artifacts/transfer/verify_foreign_reference.py --json
python3 -m unittest paper.artifacts.transfer.test_scientific_change_package
python3 -m unittest paper/artifacts/cost/test_measure.py
python3 -m unittest paper/artifacts/state-lift/test_score.py
python3 -m unittest discover \
  -s paper/artifacts/map-target-loop -p 'test_*.py'
./conformance/check-core.sh
```

The working source-only artifact builder refuses dirty Vela input and mismatched
external commits, trees, or content roots. At Vela commit
`0332406accb817513ba7e2a55b032892c7b6f226`, two independent invocations
produced identical 522-member archives at root
`sha256:b79920ed1689cd6eea2d24cb31a3dd3cc1c2045039d8a860233ca567496c2f5a`
and manifest root
`sha256:32c170c3d58e909eafddea194ce61a06f196cdd7ee64238a038cc8200f2aa0fa`.
The verifier rehashed every member and rejected unmanifested paths. This is
packaging qualification, not independent reproduction or the final release
artifact.

The same source rendered twice from clean Vela commit
`2267ab27d0a0822231fa12098c1e98b1cde046f7` with pinned Pandoc 3.9 and
pdfLaTeX 1.40.26. Both 11-page PDFs had root
`sha256:8b9e2f89cad06ab8e8bb3c46cd78cf8183d9afd99f6e2c5dea7076728ad0bf4f`.
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
scientific Standing, including one accepted supersession whose predecessor
remains recoverable through its retained compaction origin, predecessor tag,
and archive. The current compact epoch does not expose that full historical
chain through the ordinary `show` and `why` path. Two implementations also
agree on a bounded synthetic correction-impact projection. The first matched
first-party Codex-session pilot was directionally cheaper and more accurate
with Vela but failed its registered token budget and exact-answer requirement;
it is not human-review evidence. A historical foreign-reference envelope
qualified the real source package in two colocated readers, including its
authority signature. A second Frontier retained that exact package through a
pending, non-authoritative Submission, imported a scoped Verification, and
replayed the resulting repository from a clean clone with zero accepted-state
delta. This passes the bounded transfer and authority-containment gate. The
reusable runtime reader was removed after the experiment did not earn a shared
adapter contract; the exact reader and bytes remain with the paper evidence. A
prospective living-map loop reached an exact Submission,
scoped Verification, terminal human Decision, clean-clone replay, and
deterministic remap. Its first candidate exposed a stale-Target failure. A
subsequent source-local closure pass retained the completed packet and proved
its coverage from exact ancestor bytes. Later exact completions close producer
work through `10430200` and expose `10430201..10430400` as the next
nonduplicate range without changing Standing. The exact Vela Web `v0.430.0`
four-Frontier Atlas now contains 4,147 graph nodes. Three corrected bounded
Erdős results, one current Formal result, and the quantum-certificate result
remain pending human Decisions, so their post-Decision remaps remain
unfinished. An earlier broader-worded Erdős Proposal remains retained
separately. The frozen held-out selector also found no qualifying candidate.
The stronger claim, that this mechanism preserves useful scientific
inheritance across real corrections and plural authorities better than Git
alone, is therefore not satisfied by the audited system.

This result narrows the next experiment. Vela retains the admission and replay
mechanisms already shown useful. It does not promote the synthetic
correction-impact reader or claim a protocol breakthrough without a real
consequential correction, a qualifying second fixture, and measured cold-user
lift.

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
