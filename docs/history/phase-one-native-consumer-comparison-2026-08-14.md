# Phase 1 native consumer comparison

Status: INT-03 evidence packet, captured 2026-08-14. This comparison informs a
later Phase 2 decision. It does not extract shared code, move either
source-owned Profile into Vela Core, change Protocol 1, initialize authority,
or imply scientific acceptance.

## Custody

The comparison applies the architecture and execution contract frozen by:

| Approved document | SHA-256 |
| --- | --- |
| Canonical Native Integration Architecture | `3ac5740763db46c2c64a0d2154c6ab464def2cd8371e265d16a9be083f374ead` |
| Native Repository Integration and Authority Plan | `4e499ed9703560bf8f859a709d4e8f9265980e1a089a4e3fe1427583c6a0836f` |

Both consumers implement `Manifest -> Profile -> Binding -> Method` with
`authority_effect: none`. The roots below include the `sha256:` domain prefix.

| Field | INT-01: `lean-proofs` | INT-02: Formal Conjectures contributor fork |
| --- | --- | --- |
| Repository identity | `https://github.com/williamjblair/lean-proofs.git` | `https://github.com/williamjblair/formal-conjectures` |
| Integration commit | `fe4dd4a4089c9c94493e0c7a8e01c129b3f2a018` | `f8bd3dd2fd3065e4922ec169aac59a04595a5f7b` |
| Integration tree | `169f37dede069482f6e1ed991609fe76d49f94a4` | `a3c9f2c005b58f10a2392f6e4b0b3b4f4ea15efb` |
| Bound source packet | `a8c2872a27cf8d11cf6744ca4a2c5b49ace5fea0` | `96eeecf40bc06ddc8bae6d106f461d4fd774858a` |
| Manifest root | `sha256:af09cd762db00af7acdc94a92aa2f63ec1d2b4cdeb6d70c11888ccab616c4b0d` | `sha256:9f839593ee5a72c2fba51d0064b2b5c63c32d0a8e1cf9d44d2e62d979ecae3ec` |
| Publication at capture | Published to the user-owned repository as `codex/native-repository-integration`; no pull request or hosted run | Published to the user-owned fork as `codex/pr-audit-v1`; no upstream branch or pull request |
| Authority state | Absent | Absent |

Formal Conjectures preserves its frozen PR-audit v1 subtree unchanged at Git
tree `3bbf51c0be29d78fc04938ec4b653d63f2443987`. Neither integration changes
source statements, proofs, review governance, Repository policy, Decisions,
Events, or Standing.

## Rooted inventories

### INT-01

| Kind | Source-owned identifier | Root |
| --- | --- | --- |
| Profile | Lean project v0.1 | `sha256:3c9f08205adf4059b7ea06ce547b78989075d0582d542270de86bd68248539ac` |
| Profile | Formal proof v0.1 | `sha256:8358edad46299b717673061f21a607be30ecb8f2224a438163489870edd5d2d0` |
| Binding | Lean project | `sha256:e726145c67dd845600203eaaf2a4de095f1dc2bdb70210da8056c1681eee0153` |
| Binding | Formal proof index | `sha256:52e616471a7888895bb952d1605042a8bca2df0108027f7d52a4f7257dd9fc06` |
| Method | Lean build | `sha256:d758f77d782611e23e633f1bb4640004d39f5ce61df50f6890a17f030fba09f9` |
| Method | Axiom audit | `sha256:1ee9b6d7d629e673639992a66b944a2a789664b999086d9fe1f7343c6296649c` |
| Method | Integration validator | `sha256:3e0e19612953ee1f65129c5bf5151a127bcd252a4fbb6cd4660c4b25d88e3800` |

Key file digests are `vela.toml`
`851470cb1c01eac7309bb303302089f5db6eb31c175460fa48d708180c911f14`,
validator `7366ddef78c99f816f2ff7b8f6c62535b6aee62e82a00d10ea6b52ceadcb4469`,
tests `8697d8a3d029ac5185602816878fe6b719f9bfb3220c5b9ac48914801dbf288a`,
cold script `abf148e6f4bf20a6da7b3702938f0ef9a714174414840913f2120666805e0d5d`,
and portable Erdős 154 example
`53939354895ae2daede30269fb1a2be7cc365f628c19a723a3a17f80131f4160`.

### INT-02

| Kind | Source-owned identifier | Root |
| --- | --- | --- |
| Profile | Audited declaration v0.1 | `sha256:040157f15603d596040d40f95161c7ee14ba08c1bb2787812331e0eedf60051c` |
| Profile | Conditional formal proof v0.1 | `sha256:d3d99125438ff6e8e27953d7fcb8031720e2ca9c4abbbbf149f54efed19b0212` |
| Binding | Erdős 427 conditional proof | `sha256:cb234c49c67565961a61e856099ba6a079fa170c50771aa8fa2b73beda43aca7` |
| Binding | Erdős 887 selected declaration | `sha256:8698957b371fb8fbbc7de04c6d5d4e0f0982f39a92798b43fa8a3cbb5f2216d6` |
| Binding | MinModulus statement fidelity | `sha256:051ab0b812ff3a56caa6732718faabf7669a22172728870b5be65dc9e96e5fde` |
| Method | Formal-proof condition review | `sha256:0a5c29f9ce0d53ce029e963aaf1fa782c7c88d00c3d46074061e55405d0ee944` |
| Method | PR-audit core replay | `sha256:35cad804504ea2371e2c849c840b28f33506a5f4208c5b2352bad6c88532161e` |
| Method | Retained exact-head build | `sha256:84a3a40ef361206dc6c062066ea3bc4ded4dbc0721dea1d04153f4854fc518d6` |
| Method | Source-statement fidelity review | `sha256:b04be736f7a37d43e5d863ee33ba942658d8659a13834d99e3e9adc9da41e311` |
| Method | Answer-slot scope review | `sha256:e7f0f3152f5409e5e6e8eedf94d2c18141c860e981c418486cf74d1e668b264d` |

Key file digests are `vela.toml`
`da7e7100c1e59c2b23f25f7d21ce88430a0f345a22908eb7dcf68caf02b053d1`,
validator `0bc4066e0f6162a69eae031c95dec10a0752b77db9c945b57c6b3db2dae2ad62`,
hostile tests `886f1d89859862a8a4bd99743877cdfa5f488d563f8c2aa53a994bdd14121e40`,
and portable Erdős 887 export
`09f8c8fb9517a79480e0ad8bce7d3f072400ca0bca8453fde7cadb7e6445eea0`.

## Shared behavior

Both implementations independently provide:

- source-local, rooted TOML Manifests, Profiles, Bindings, and Methods;
- schema-domain-separated canonical JSON SHA-256 roots;
- the common nested Exact Reference value shape, with full Git revisions,
  content digests, byte lengths, selectors, canonical locators, and explicit
  mutability (commit-pinned public URLs in INT-01; repository-relative retained
  paths in INT-02);
- closed versioned fields and refusal of unknown schema or Profile versions;
- rooted Manifest inventories that bind Profile, Binding, and Method identity;
- mapping relations kept separate from translation dispositions;
- explicit rights, availability, retention, limitations, and nonclaims;
- deterministic portable Verification inputs without a Vela authority writer;
- separate Agent, Activity, Entity, and Role provenance where work is
  consequential;
- refusal of authority fields or outputs and `authority_effect: none`; and
- hostile mutation tests for root, revision, selector, fixity, path, Method,
  availability, and authority drift.

Neither implementation interprets a successful build, audit, review, merge,
approval, CI result, signature, or publication as acceptance or Standing.

## Source-specific behavior

### Lean proofs

INT-01 maps all 79 `proofs.yaml` entries to native Lean declarations, source
files, toolchains, external problem identities, build coverage, and axiom-audit
coverage. It runs the current native build and `#print axioms` audit. The
portable example emits only a deterministic, unrooted Erdős 154 Verification
input: exact subject, Method, seven-artifact closure, and check request, with no
outcome or evidence-availability claim. Its 2,845 stored bytes have SHA-256
`f018c21f8662253e81bb8fca0200f2a2e70948563a05ed4fffa4701945d7f3b8`.
It is not a fifth integration document or a Protocol object.

The selected closure used `leanprover/lean4:v4.29.1`, Mathlib
`5e932f97dd25535344f80f9dd8da3aab83df0fe6`, and PrimeNumberTheoremAnd
`d7f9e2bfdcc7e34dfb9328b7494a6d424ff50c96`. Its audit reports only `propext`,
`Classical.choice`, and `Quot.sound`. Two `sorry` warnings in the pinned
external PNT package are disclosed; neither is in the selected Erdős 154
dependency cone.

### Formal Conjectures

INT-02 exposes the frozen source-owned PR-audit v1 rather than replacing it.
It keeps exact identities for the repository, problem, declaration, linked
proof artifact, proof condition, retained build, and attributed statement
review. Its five Exact References use only the common nested
`native_identity`, `revision`, `content_fixity`, `selector`, and `locator`
shape; source labels and audit metadata remain separate. The selected Erdős
887 export keeps its mechanical build `pass`,
semantic answer-slot review `fail`, and later `MERGED` / `APPROVED` observation
separate. None changes Standing.

The contributor fork also binds the conditional Erdős 427 proof metadata and
the independently attributed MinModulus statement-fidelity review. Its full
native repository gate used `leanprover/lean4:v4.27.0`, Mathlib
`a3a10db0e9d66acbebf76c5e6a135066525ac900`, and built 9,035 jobs. The
portable inspection path intentionally replays retained exact evidence instead
of rerunning the historical GitHub build or external review.

## Duplicated validation code

The two source validators duplicate the following small contract machinery:

1. TOML loading and canonical JSON root framing with a schema tag and NUL;
2. full-root, version, and closed-field checks;
3. safe repository-relative path and content-fixity checks;
4. Exact Reference revision, selector, mutability, and locator checks;
5. Manifest inventory and Profile/Binding/Method root linkage;
6. mapping-relation and translation-disposition vocabularies;
7. rights, availability, limitations, and nonclaim requirements;
8. recursive authority-field and authority-output refusal;
9. deterministic portable output construction; and
10. mutation-based hostile test helpers.

The dominant code remains source-specific. INT-01 parses `proofs.yaml`, Lean
declarations, toolchains, manifests, build coverage, and axiom coverage.
INT-02 validates PR-audit cores and observations, retained typed results,
review attribution, proof conditions, linked-proof mappings, and semantic
review separation. The duplication is evidence for a Phase 2 evaluation, not
authorization to extract it now.

## Unsupported cases and semantic losses

| Area | INT-01 | INT-02 |
| --- | --- | --- |
| Coverage | All 79 proof-index entries are locally validated, but only Erdős 154 is serialized as the portable example | Only frozen audit targets Erdős 427, Erdős 887, and MinModulus are covered |
| Semantic review | Build and axiom closure do not establish statement fidelity or mathematical truth | Retained reviews cover named properties only; they do not establish mathematical truth |
| Cross-source mapping | Erdős 154 to Formal Conjectures is `close` and `normalized`, not byte identity or equivalence | Linked proof to target is `related`; repository-authority wording is normalized to repository metadata |
| Unavailable evidence | The private Erdős 730 attachment is excluded and cannot become an outcome | Missing evidence remains unavailable and cannot become pass, fail, error, or zero |
| Proof execution | Native Lean build and axiom audit run for the current packet | Erdős 427 review checks condition/link metadata; it does not execute or compare the linked proof |
| Mutable or authenticated sources | The draft supports exact public Git/Lean regular-file inputs only | The draft supports retained public audit bytes only |
| Adoption | No general proof-registry or branch-to-Workspace adoption claim | No claim of adoption by `google-deepmind/formal-conjectures` |
| Product resolution | External identities are source mappings, not public occurrence authority | Erdős 887 has no reviewed `problems.science` occurrence mapping and remains unresolved there |

Neither source-specific Profile is a general theorem-proof, review, or
repository Profile for Vela Core.

## Rights and availability

INT-01 identifies repository-authored material as MIT, discloses dependencies
through `lake-manifest.json` and `NOTICE`, and uses aggregate `NOASSERTION`
because retained Star Fleet material has hosting permission but no published
source license. Anonymous HTTPS availability was observed on 2026-08-13; Git
history is the retention basis, not a perpetual-hosting promise.

INT-02 identifies repository software as Apache-2.0 and repository-authored
non-software material as generally CC-BY-4.0. Imported conjecture, proof-link,
and review material retains source-specific terms, so combined exports use
`NOASSERTION`. The contributor fork and retained bytes were publicly cloneable
without authentication; that observation is also not a perpetual-hosting
promise.

## Cold-consumer measurements and failure modes

The measurements are different workloads and must not be read as a speed
comparison.

| Measurement | INT-01 | INT-02 |
| --- | --- | --- |
| Environment | `git clone --no-local`, no inherited project `.lake`, authority state, `vela-science/math` checkout, credentials, private context, or hosted Vela; pinned Mathlib is fetched into `.lake/packages/mathlib` | `git clone --no-local` under an empty environment, plus a Git archive with no `.git`; no authority state, `vela-science/math` checkout, credentials, private context, or hosted Vela |
| Portable validation | Included in the full cold gate | 0.25 s from clone; 0.18 s from archive |
| Deterministic regeneration | Double emit and byte comparison included in the full gate | 0.09 s from clone; passed from archive, not separately timed |
| Source integration tests | 14/14 control, contract, and hostile tests passed | 11/11 control, contract, and hostile tests passed in 1.80 s |
| Native proof/build work | 8,345 build jobs plus 65-declaration axiom audit | Separate full native gate built 9,035 jobs; portable inspection replays retained results |
| Total cold wall time | Independent run at `fe4dd4a`: 350.38 s, including official content-addressed Mathlib cache retrieval and decompression | The timed portable clone steps above total 2.14 s; the separately run full Lean gate was not timed in this comparison packet |

INT-01's cold gate can fail on source or toolchain drift, an unavailable
content-addressed dependency cache, compilation failure, missing axiom
coverage, false-clean `sorry` evidence, manifest drift, authority state, or
nondeterministic output. Its use of the official Mathlib cache is network
availability, not a reused project build cache.

INT-02's portable gate can fail on source, audit, revision, selector, digest,
root-domain, schema, Profile, Method, rights, availability, private-path, or
authority drift. It also refuses flattened Exact Reference fields, unknown
nested fields, and a retained repository-relative locator presented as
immutable. Review found and closed five concrete fail-open cases before the
final normalization: arbitrary repository revision, a proof object-kind
bypass, a false linked-proof selector, missing proof-condition identity, and a
selector that could disagree with the native identifier. Independent review
of the published final commit reran all 11 integration tests in 1.13 s.

## Phase disposition

The two consumers demonstrate only the four generic integration document
contracts—Manifest, Profile, Binding, and Method—and the Exact Reference,
root, and inventory mechanics used within them. They do not support a shared
Lean, proof, audit, review, or other scientific Profile. They also do not yet
show that Core extraction would delete more maintained code than it adds.
Therefore:

- keep both Profile sets source-owned;
- keep both validators source-owned through Phase 1;
- carry only the ten duplicated validation responsibilities above into the
  Phase 2 extraction analysis;
- evaluate DSSE or in-toto only as optional transport after that analysis;
- do not create a shared proof, audit, review, or source-registry Profile;
- do not add a Protocol 1 object or authority effect; and
- do not treat these two successful integrations as upstream adoption or
  scientific validation.

INT-03 is complete when this exact comparison is reviewed alongside the two
frozen consumer packets. CORE-INT-01 remains a separate, later decision.
