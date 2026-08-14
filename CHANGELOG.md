# Changelog

The `0.967.0`, `0.968.0` and `0.968.1` headings are reconstructed. Those three
releases shipped while this file still called their notes "Unreleased", and the
heading sat below `0.969.0` rather than above it. The split is taken from the
commits that moved `Cargo.toml`'s version: everything the file held at `5d40bb5`
is `0.967.0`, what `5cd6f01`, `b478530` and `f9c127c` added is `0.968.0`, and
`0.968.1` is `6c500dc`. Those sections carry what was written at the time and
not a note per commit, so they are shorter than the releases were.

## Unreleased

## 0.975.1

- **Superseded Claims now report their current Standing.** `vela why` and
  `vela show` give an admitted predecessor supersession precedence over the
  retained accepted Proposal that first introduced it. Machine output keeps
  current Claim Standing (`superseded`) distinct from historical Proposal
  status (`accepted`), and human output names the exact successor Claim. This
  is a read-projection repair; it changes no Protocol 1 object, Decision,
  authority rule, Event, accepted set, or repository root.

## 0.975.0

- **Native repositories have one stable, non-authoritative integration
  waist.** `vela integration check` validates the shared Manifest, Profile,
  Binding, Method, and Exact Reference structure, while
  `vela integration inspect` renders the exact rooted inventory without
  executing a native Method. Lean proofs and the Formal Conjectures contributor
  fork retain their source-specific proof, build, audit, review, rights, and
  availability semantics behind that common boundary.

- **Integration remains outside Protocol and scientific authority.** The
  package-plane contract has `authority_effect: none`; it creates no Decision,
  Event, or Standing and treats a successful build, proof check, review, or
  integration check as neither acceptance nor scientific lift. Protocol 1
  remains a release candidate, and this release makes no external-adoption or
  Protocol 1.0 claim.

## 0.974.2

- **Decision Inbox language matches capability-based authority.** The rooted
  read contract is now `vela.decision-inbox.v3` and records
  `attributed_decision_required`, removing the last human-only gate label while
  preserving the same exact Proposal, Verification, policy, root, and signing
  requirements for human and agent performers.

## 0.974.1

- **Decision read surfaces preserve performer provenance.** Proposal and
  `review show` output now carry the performer class, optional source-owned
  session reference, and distinct Repository authority principal so downstream
  readers never infer them from an actor name.

## 0.974.0

- **Decision authority is capability-based and performer-attributed.**
  `vela review accept|reject` now records a human or agent performer, optional
  source-owned session reference, and the distinct Repository authority
  principal. Both performer classes use the same Proposal, Verification,
  policy, current-root, signing, and replay gates. Agent reviewer principals
  remain structurally barred from Repository governance actions.

## 0.973.1

- **Published acquisition guidance names the release that actually exists.**
  The README, quickstart, citation, release qualification, and ecosystem
  declaration now point to the signed `v0.973.1` patch release instead of
  retaining the pre-publication 0.972.1 fallback text shipped in 0.973.0. The
  protocol and scientific-state semantics are unchanged.

## 0.973.0

- **Repository writes have one exact authorization and recovery boundary.**
  The policy-neutral `vela-repository` runtime now owns durable transaction
  plans, commit markers, installation, and idempotent recovery, while the CLI
  binds and revalidates a move-only authorization immediately around the
  marker. `vela recover --repo <PATH> <OPERATION_ID>` opens only the named
  journal, aborts an exactly uncommitted plan, or completes an already
  authorized installation without reacquiring a signer or replaying the
  semantic command. Native genesis can resume only its fully revalidated,
  deterministic Git and trust-pin tail.

- **Git publication and repository reads fail closed over exact ordinary
  history.** Publication rechecks the prepared delta at descriptor and Git
  boundaries and rejects a post-preflight transplant. Offline reads reject
  shallow, partial, alternate, grafted, or promisor-backed object stores and
  ignore replacement refs instead of allowing ambient Git state, missing
  history, or network retrieval to change an exact result.

- **The shipped product surface is smaller.** The unused Target Index and its
  `next` and `start` commands, the composite repository Action, the standalone
  verifier binary, the retired Lean replay candidate, and other unconsumed
  compatibility layers are removed. Workspace crates remain internal
  implementation boundaries released as one `vela` binary.

- **Protocol 1 remains a release candidate with executable conformance.** The
  normative specification, JSON Schema selection, digest-bound manifest,
  independent Python and JavaScript readers and emitters, current-object and
  authority-chain vectors, three reference flows, and deterministic release
  checks now run as one qualification surface. A green result demonstrates
  implementation agreement only; it is not `v1.0.0`, external adoption, a
  scientific Decision, or a change in Standing.

- **Candidate and published-release metadata are explicit and truthful.**
  Workspace packages, citation metadata, and the local ecosystem declaration
  name candidate `0.973.0`, while install examples remain on the latest signed
  published release, `v0.972.1`, and the candidate citation withholds a release
  date. Current acquisition docs and ecosystem status also record Math's
  authenticated private source access and the absence of a current Math read
  replica. The separately versioned `vela-source-manifest` runtime now reports
  the same `2.0.0` version as its package metadata after removal of the unused
  `home` schema alias.

## 0.972.1

- **Release smoke accepts the UUID authority trust-pin layout.** The `v0.972.0`
  workflow built the macOS archive, SBOM, and checksums, then correctly stopped
  before creating a draft release because its smoke test still expected the
  retired `vrepo_` filename. The smoke test now requires a canonical lowercase
  RFC 9562 UUIDv4 filename and cleans up that exact trust pin. The failed tag
  remains historical evidence; `0.972.1` is the replacement release.

## 0.972.0

- **Repository identity is a standard RFC 9562 UUIDv4.** `vela init` now mints
  lowercase canonical UUIDv4 text from the operating system random source, and
  every protocol reader, authority boundary, schema, conformance reader, and
  derived view rejects the retired `vrepo_` shape. The identifier remains an
  opaque routing identity rather than a security root; origin, repository, and
  authority roots continue to carry the security commitments. This supersedes
  the interim 128-bit custom identifier described in the 0.971.0 notes before
  that shape reaches another release. There is no dual reader: the live
  mathematics authority receives one UUID when it is re-genesised for this
  already-breaking release.

- **Submission provenance has one external run identity.** The unreleased v2
  payload carried both `source_run` and a bespoke `source_attempt` with a
  `vat_` identifier, while CLI authoring populated only the latter and the
  duplicate-run guard depended on the former. The redundant field and custom
  identifier are deleted before release. `--source-run` now writes the one
  standards-neutral `provenance.source_run` value; Vela records that external
  identity but does not mint or govern workbench runs.

- **The required gittuf deletion spike selects the native authority path.** An
  isolated gittuf v0.15.0 policy protected the same Repository in which Vela
  completed Submission, independent Verification, an authorized Decision, and
  strict replay. Gittuf correctly rejected an unauthorized RSL signer, but the
  combined design deleted zero Vela code, added a second root and policy
  lifecycle, and still required every scientific authority check. The evidence
  and measurements are retained in `docs/GITTUF_AUTHORITY_DELETION_SPIKE.md`;
  gittuf remains an optional external publication-integrity layer.

- **The public contract closes its remaining standards gaps.** Repository
  Profile licenses are parsed as SPDX expressions and initialization uses
  `NOASSERTION` instead of free-form `varies`; the dev-only Action test uses
  maintained `serde-saphyr` rather than the `serde_yaml` fork. Generated JSON
  Schema 2020-12 documents now cover Repository Profile, authorization request,
  authorization evaluation, and the stable `vela.error.v1` CLI failure
  envelope. RFC 8785 vectors run independently in Rust, Python, and JavaScript,
  including a UTF-16 property-order case. CodeQL default setup, a pinned
  OpenSSF Scorecard workflow, CODEOWNERS, and GitHub private vulnerability
  reporting complete the repository security surface. The independent Python
  emitter now pins `cryptography` 50.0.0, clearing the six open advisories that
  affected its previous 46.0.5 pin.

## 0.971.0

- **One DSSE implementation, and every signed Vela object uses it.**
  Repository-authority records already used a DSSE 1.0.2 envelope. Submission,
  Verification Record and Proposal Withdrawal signed a bespoke preimage
  instead: the canonical object with its own identifier and signature fields
  cleared to a fixed value, hashed, signed, and then reassembled — a convention
  a foreign implementation had to reproduce exactly and could get subtly wrong
  in silence. All three are now DSSE payloads under their own versioned payload
  types, carried in the envelope `crates/vela-protocol/src/kernel/dsse.rs`
  produces and reads, and the zeroed-field convention is gone from the
  protocol.

  This is a wire break, and every payload type and schema tag moved with it, so
  a predecessor object fails to parse rather than parsing differently:

  ```text
  application/vnd.vela.submission.v2+json            vela.submission.v2
  application/vnd.vela.verification-record.v2+json   vela.verification-record.v2
  application/vnd.vela.proposal-withdrawal.v2+json   vela.proposal-withdrawal.v2
  ```

  Each parser verifies the exact payload bytes once and hands those same bytes
  to the strict payload parser, so there is no window in which a reader has
  checked one serialization and parsed another. The accepted cost is that a
  retained Submission is base64 in a Git diff rather than readable JSON, which
  is the shape authority records have always had; `vela show`, `vela why` and
  `vela review show` decode it.

  `IdentityBinding` became `SignerIdentityV1` and lost its ceremony — no
  `binding_id`, no nested signature, no `vib_` prefix. The outer signature
  proves possession of the key the payload declares, and a second raw signature
  repeating the same fact is a second thing to keep in agreement.

- **Full roots are canonical; short handles are derived.** No object stores its
  own short identity any more: `proposal_id` is gone from `ProposalV1` and
  `origin_id` from `RepositoryOriginV1`, and `vpr_` and `vro_` come from
  `derive_handle` over the object's own root. A handle stored as a *reference*
  is now checked by re-deriving it from a full root in the same object, which
  is why `VerificationSubject` gained `proposal_root`: naming a Proposal by
  handle alone left a reference no reader could check. `vcl_` keeps its 64 hex
  characters — it is content-derived over an intentional scientific subset, not
  a truncation.

- **A repository origin is a genesis, and nothing else.**
  `RepositoryOriginKind`, `RepositoryOriginPredecessorV1` and the `compaction()`
  constructor are deleted, along with the eleven predecessor fields and the
  `pre-compaction/` tag rule. The path was written for one pre-release repair
  (ADR 0027), used once, and no repository the current binary can read needs
  it. Continuity across a future lineage change belongs in a separately signed
  attestation over exact commits, trees and roots — not as eleven permanent
  fields on every origin that will never be a compaction.

- **Cedar is retired, and the parity corpus that nothing read now decides it.**
  `conformance/fixtures/epoch1/authorization-profile-parity.json` held every
  retained epoch-1 authorization request and was, by `AGENTS.md`'s own account,
  read by no code. `crates/vela-authority/tests/authorization_profile_parity.rs`
  reads it: it recomputes all seven historical Allows under the closed Vela
  Authorization Profile and checks seven negative boundary cases for their exact
  fail-closed reasons. The test was written before the deletions, so a
  disagreement would have stopped the cut with Cedar still in the tree.

  Then the deletions. `cedar-policy` is out of both manifests,
  `crates/vela-protocol/tests/engine_pin.rs` is gone, and `PolicyBundleV1` is
  `AuthorizationModelV1` — sorted members, each with a principal class and one
  of two roles, and no engine, version or profile to pin. `crates/vela-authority`
  went from 738 lines to a module doc and its re-exports.

  ADR 0035 §4's history gap closed with it. An authority record retains the
  exact `vela.authorization-request.v1` it was written under, so strict replay
  recomputes the decision under the rooted model instead of trusting a retained
  `Allow`.

  ADR 0042 said this was unreachable, and it was right about the mechanism: a
  reader that refuses `vela-science/math`'s retained bundle turns a live
  authority read-only, and no rotation writer exists. What dissolves it is the
  wire break above. `math` must re-genesis, genesis mints the authority chain
  and its model fresh, and there is no retained bundle left to contradict a
  Cedar-free reader. The rotation writer was never needed. ADR 0042 is
  Superseded; ADR 0035 is Accepted.

- **`vela-science/math` must be re-genesised to load under this release.**
  This is the second such requirement in one version and it is the same
  ceremony, which is the point: `AGENTS.md` and the 2026-08-08 architecture
  memo both say to bundle breaking changes into one cut so an operator performs
  one re-genesis rather than one per change. Until it happens the binary
  refuses the current `math` head with a schema error, exactly as 0.970.0 did.
  It needs the authority key in a local OpenSSH agent and cannot be done from
  CI.

- **Every file has its plain name.** A version suffix on a filename is a
  promise the repository does not keep — `submission_v2.rs` reads as though
  `submission_v1.rs` sits beside it, and it does not, because a superseded
  surface is deleted rather than kept as a second mode. Sources, schemas,
  fixtures, docs, paper artifacts and the Lean replay package all dropped
  theirs. The version stays where it is load-bearing, which is the wire:
  `schema` tags still read `vela.submission.v2`, payload types still read
  `application/vnd.vela.submission.v2+json`, and each published schema's `$id`
  still names the file it is published as.

  Two frozen roots moved as a result, and both were re-taken by the readers
  that own them rather than edited to match:
  `research/lean-replay-contract` from `sha256:5653a31b…` to `sha256:a72d2e26…`,
  agreed by `build_root.py` and the independent Rust reader, and the
  scientific-change-package plan from `sha256:72d84fd4…` to `sha256:b719174a…`,
  which the builder cascaded through its amendments and generated outputs.

  `research/lean-replay-contract-evidence/qualification.json` was recomputed to
  the package's current identity, with the superseded root kept under
  `predecessor`. Its two-consumer gate went to `false` in the same edit, and
  that is the substantive change rather than the root: Formal and Erdős agreed
  on the predecessor root at the commits the record names, and ADR 0039 has
  since archived both repositories read-only, so those runs cannot be repeated
  and no live repository consumes the package. The gate is unrepeatable against
  those consumers rather than merely unpassed, and Level 1 promotion needs it
  earned again on a live one. A new test holds the record's `package.root` to
  the root the Rust reader measures, because `conformance/repository_lint.py`
  reads that field to qualify a repository's dependency on this unreleased path
  and a record naming a stale root qualifies nothing, silently.

- **`vela why` explained every Claim as having no Verification.** It filtered
  the stored Verification Record JSON on `/subject/claim_id`, which the DSSE cut
  moved inside the envelope, so the pointer matched nothing and every record was
  dropped. Nothing caught it because an empty list is what a Claim with no
  Verification legitimately looks like — the failure had no shape of its own.
  `review show` also grew `verification_record_id` beside the root it derives
  from, which the payload no longer carries.

- **`repository_id` is 128 bits.** It was 64 — `vrepo_` and sixteen hex
  characters truncated from a SHA-256 digest for no reason beyond brevity. The
  identifier is a routing handle rather than a security root, which `docs/ROOTS.md`
  states plainly and this release does not change: the cryptographic roots still
  do every security-critical job. But it is also the durable name of an
  authority, expected to distinguish independently created repositories for as
  long as any of them exist, and 64 bits puts a birthday collision at roughly
  five billion repositories. Widening it costs nothing that was worth saving.

  Pre-1.0, so there is no compatibility branch: `vrepo_<16 hex>` is now invalid
  rather than accepted beside the wide form. Every identifier that carried the
  narrow shape was minted by a genesis that has since been replaced. The width
  is one named constant, `REPOSITORY_ID_HEX_LEN`, where it was the literal `16`
  at six call sites across two crates.

  **`vela-science/math` must be re-genesised to load under this release**, the
  same way 0.970.0 required it: `vela status` on the 0.970.0 checkout answers
  `profile.repository_id must be vrepo_<32 lowercase hex>` and refuses. Nothing
  already signed is invalidated — the accepted Claim keeps its `claim_id`,
  which is derived from the bytes of the record and not from the repository.

- **`vela status` leads with the repository, not with its identifier.** The
  first two lines are now the name and the origin remote as a person refers to
  it — `Vela Mathematics`, `github.com/vela-science/math` — with `repository`,
  `state` and `commit` underneath. `repository_id` has not moved and is no less
  load-bearing; it is machine identity, and it sits where an identity, trust or
  debugging question finds it. This is the split Git already draws between
  `main` and the commit it points at. `state` is new to the human surface: the
  exact repository root was in `--json` and nowhere a reader could see it.

  `vela.status.v4` is unchanged. Only the rendering moved.

- **The protocol docs stop calling a Frontier a container.** ADR 0039 settled
  that a Repository is the authority boundary that holds canonical state and a
  Frontier is a derived query that owns nothing — and `docs/PROTOCOL.md` still
  opened §3 with "A current Frontier contains:" above a directory listing.
  `docs/TERMINOLOGY.md` had already defined the word correctly two sections
  above the places that used it the old way. Twelve sentences across six
  documents now say Repository where they mean the thing with the bytes.

## 0.970.0

- **The retired vocabulary is gone from the code, not just from the wire.**
  v0.967.0 renamed `Frontier` to `Repository` across the schemas, the help output
  and the identifiers, and left the codebase itself speaking the old word: 813
  occurrences of `frontier` in `crates/`, and every file, type and function that
  said `current_` to distinguish this epoch from a predecessor epoch ADR 0039 had
  already deleted. Both are finished here. `frontier` survives in 25 places, each
  of which is the word as a subject rather than a name — the retired path
  literals the verifier must keep refusing, the guard that holds the product
  surface to zero, the comments quoting the drift as evidence, and one
  breadth-first search boundary where it is simply the right word.

- **Authority events bind to the repository, not to a literal.** Three checks
  compared `target.type` against `"frontier"` while the test that means something
  sat beside them:

  ```text
  || event.content.target.r#type != "frontier"
  || event.content.target.id != record.content.repository_id
  ```

  The first checks a value this binary also writes, against itself.
  `initialization_payload_from_event` had it and *no* id test, so an
  initialization event could name one repository in its target and another in its
  payload and pass. The type tests are gone and the missing id test is added.

- **`entity Frontier` is `entity Repository`.** The Cedar schema hashes into
  `policy_bundle.cedar_schema_root`, which `authority_transaction.rs` compares
  against the retained bundle before every write, so it could not move while
  `vela-science/math` held a bundle computed over the old bytes. That repository
  is re-genesised in this release. `frontier_administrator` becomes
  `repository_administrator`, and `AuthorityResourceTypeV1::Frontier` becomes
  `::Repository` — under `rename_all = "snake_case"` the variant name *is* the
  wire token, so its doc comment claiming it already emitted `"repository"` was a
  statement the code contradicted. Same for `FrontierMismatch`.

- **Authority records written by 0.970.0 differ byte-for-byte from 0.968.1's for
  the same intent, and this is deliberate.** Five internal request schemas
  (`vela.current-submit-request.v2` and four siblings) lost the epoch word and
  feed `intent_digest`; `PlanCommitment` and the read-set tags `frontier_file:` /
  `frontier_directory:` feed `transaction_read_set_root`; the Cedar and
  authorization vocabulary above feeds `AuthorizationRequestV1::root()`.

  Nothing already signed is invalidated. `intent_digest` and
  `transaction_read_set_root` are shape-checked on read and compared *within* a
  record — `approval.intent_digest == record.content.intent_digest` — and never
  recomputed from the plan. `verify_canonical_hashing.py` stays at 8 vectors, 0
  failed: the protocol objects a foreign implementation is held to are untouched.
  `docs/INTEROPERABILITY.md` Rule A says of signed preimages that there is no
  compatible change; this release makes an incompatible one, on purpose, in the
  last window before 1.0 where it is cheap.

- **Removed, each with no consumer:** `EventKind::FrontierCreated` (nothing
  emitted its wire string), `target_os = "android"` from 33 support predicates
  (Vela ships Linux x86-64 and macOS Apple silicon; CI builds neither Android nor
  anything else), the `opposes` relation alias (the fixture recorded
  `retained_uses: 0` and "written into no record"), the Action's deprecated
  `frontier` input (its only pinned consumers resolve to a commit predating the
  alias), and `exact_git_blob_at` with its test — `pub`, uncalled, and documented
  as serving an agent runner ADR 0031 removed.

- **Three functions were being written more than once**: `read_claim` twice
  byte-identical, `publication_error` three times, and — found by the rename
  collapsing them onto their own names — two `pub(crate)` wrappers whose entire
  body was a call to the epoch-prefixed version of themselves.

- **Consolidated:** `canonical::sha256_root` (18 inline sites across four
  crates), `shape::is_prefixed_lower_hex` (six implementations of one identifier
  rule, three for `vrepo_` alone), `shape::require_sha256_root` (three identical
  kernel copies), seven `canonical_root` methods, seven `require_sha256` copies,
  and `TEXT_MAX_BYTES`, which already existed and was `pub` while both of its
  readers restated `16 * 1024`.

- **Documentation held to the tree.** `docs/ECOSYSTEM.md` §8 said "15 verbs" for
  as long as there had been sixteen and named one emitter for as long as there
  had been two; both are now bound by tests. §6 described `epoch1/` as an
  outstanding TODO when ADR 0039's own amendment had withdrawn it, and
  `scripts/ecosystem-status.py` had encoded that misreading as a *checked*
  assertion. §4 restated a withdrawn §9. The documentation index walked one level
  deep, so the interop profile and the Genesis dossier were published where no
  page could link them.

- **ADRs 0040, 0041 and 0042** propose, without implementing: a producer-declared
  dependency on `vela.submission.v1`, a language-independent conformance vector
  for the authority contract, and the policy-bundle rotation this repository
  cannot perform.

## 0.969.0


- **`vela correction impact` reaches the correction-impact projection.**
  `crates/vela-edge/src/analysis/correction_impact.rs` has implemented
  `vela.correction-impact-projection.v1` — dependency traversal, lost and
  surviving support routes, repair obligations — since it was written, and
  nothing called it. The new verb calls it over the accepted claim index of a
  real repository. It adds no object: the derivation, the input schema and the
  projection schema are the ones already under conformance.

  The argument is the successor — the Claim carrying `corrects` or
  `supersedes` — so a correction still in the review queue can be asked what
  accepting it would cost. The projection root is identical before and after
  the Decision.

  A Claim Record may declare a discharge condition under the `vela.correction`
  extension; where none is declared the protocol's own default applies, and the
  verb reports per obligation which of the two it used.

- **Fixed: accepting a correction made the repository unreadable.** Acceptance
  retires the predecessor, so it left the accepted index while its own Proposal
  stayed retained saying `accepted`. `validate_current_proposal_standing` read
  those two facts as a contradiction, and every read verb failed afterwards —
  `status`, `claims`, `replay`, `why`, `review list`. A protocol whose central
  move is correction could not be read after making one. The loader now
  identifies a retired predecessor from the successor's own retained Claim
  Record, using the protocol's `moves_standing` test for which relation kinds
  acceptance acts on. Held shut by
  `crates/vela-cli/tests/correction_impact.rs`.

- **A second independent emitter.** `conformance/emitters/python.py` produces
  byte-identical signed Submissions and Verification Records from a clean-room
  implementation, and `verify_current_objects.py` holds both it and the
  JavaScript emitter to the same fixtures. The two differ where it matters:
  the JavaScript emitter sorts keys by UTF-16 code unit, this one calls
  `rfc8785`, which sorts by code point as JCS specifies.

- **`docs/interop/scientific-state-profile.md`.** The seven contracts an
  external implementation must satisfy, each paired with the conformance check
  that decides whether it does. It names existing schemas and creates no
  parallel object model.

- **A correction driven through the live authority.** `vela-science/math`
  corrected its own accepted Erdos 321 Claim after an adversarial review of the
  repository's own records found two overstatements in its evidence. Nothing was
  rewritten: a `corrects` Submission was verified and ruled on, the predecessor
  retired, and the repository stayed readable afterwards — which is the fix
  above, exercised against a real Decision rather than a fixture.
  `vela correction impact` returned the same projection root before and after
  that Decision, `sha256:a31aaa80…`.

- **Sequencing, learned the hard way.** A loader fix has to ship *before* the
  state that needs it exists. The correction above was made against a `main`
  build, and the projection pipeline pins the released generator by design — so
  released v0.968.1 now cannot read `vela-science/math` at all:

  ```
  err · current Proposal vpr_74d3674dbe1954f2 standing disagrees with the
  repository Claim indexes
  ```

  The refresh therefore fails, loudly, and activates nothing; the Observatory
  keeps serving the last consistent state. That is the pin behaving correctly —
  it refuses to project state its generator cannot read rather than projecting
  something wrong — but the read side cannot advance until this release ships
  and `vela-release.v1.json` is bumped. The correct order was release, bump,
  then correct.

- **Known and unclosed:** the correction-impact projection traverses `depends`
  and `supports` claim-to-claim edges and the write path authors neither.
  `vela.submission.v1` has no field for a producer to declare a dependency, so
  a repository built with this release records corrections and cannot record a
  cascade. That absence is ADR 0004's standing position, and this release
  supplies the first real evidence in that lane rather than settling it.

## 0.968.1

- **A release is signed before it becomes immutable, not after.** `v0.968.0`
  published its manifests and then refused the signatures — `HTTP 422: Cannot
  upload assets to an immutable release` — because publication closed the door
  and signing had to come after it. `release.yml` now creates the release as a
  draft, and `scripts/sign-published-release.sh` signs each manifest, checks its
  digests against the published assets, uploads the sidecars, and then
  publishes, which is where immutability takes hold. A release that fails any
  check stays a draft. `action_contracts.rs` asserts `--draft`, because dropping
  it restores the deadlock silently.

  `v0.968.0` is immutable and carries no signature. It cannot be repaired and
  stands as the last unsigned release.

## 0.968.0

- The published Action takes `repository`. `frontier` remains, declared and
  documented as a deprecated alias, because four pinned consumer workflows pass
  it and a pin cannot be edited from here. A composite action has no alias
  mechanism, so the two are separate inputs coalesced in one step: `repository`
  wins when set, `frontier` warns, and two different non-empty paths fail
  rather than being resolved silently to one of them. Both inputs default to
  empty so that unset stays distinguishable from set — a `"."` default on
  `repository` would make every legacy caller passing a subdirectory look like
  a disagreement. The internal `FRONTIER` variable is `REPOSITORY_PATH`.

  ```text
  frontier:  <path>   ->  repository: <path>
  ```

- The last two wire tokens spelling the retired word moved, each with the
  version bump that was the reason they survived the prose sweep:

  ```text
  vela.repository-verification.v2  ->  vela.repository-verification.v3
    frontier                       ->    repository_path
  vela.reproduction-summary.v1     ->  vela.reproduction-summary.v2
    scope accepted_frontier        ->    scope accepted_repository
  ```

  `replay` was reporting the directory it read, which is a Repository; a
  Frontier is a derived query with no directory to report. The reproduction
  scope is printed verbatim by `vela reproduce`, so the human surface moved
  with the schema, and `docs/VERIFICATION.md` documents the new token.
  `wording_contract.rs` now pins both new tokens and asserts the retired
  spellings absent, which is the half that was missing: the old contract
  asserted `frontier` present and never ran `reproduce` at all.

  `integrity.replay: "verified"` in `vela.status.v4` is untouched. It is the
  one wire token left that a prose sweep must not take, and `vela-web` pins it
  as `z.literal("verified")`.

## 0.967.0

- **Epoch change.** `Frontier` was doing three incompatible jobs — authority
  boundary, topic boundary, product slice — and ADR 0039 separates them.
  `Repository` is the authority boundary, `Source` the provenance boundary,
  `Problem` a bounded question, and `Frontier` the derived boundary of
  unresolved state, which owns nothing and has no identifier. A repository
  exists because there is a new authority, never because there is a new topic.

  Breaking, with no aliases and no compatibility layer:

  ```text
  frontier_id                -> repository_id
  vfr_<16 hex>               -> vrepo_<16 hex>
  vela.frontier-profile.v2   -> vela.repository-profile.v1
  vela.status.v3             -> vela.status.v4
  status.frontier            -> status.repository
  git.role frontier_head     -> repository_head
  vela.frontier-init.v3      -> vela.repository-init.v1
  frontier.toml              -> vela.toml
  --frontier <path>          -> --repo <path>
  finding.asserted           -> claim.asserted   (and .noted/.retracted/.superseded)
  attempt.claimed            -> target.claimed
  ```

  An Event id is derived from its content, so the kind renames change the id of
  every Event that carried an old one. The four pre-0039 repositories are
  archived rather than rewritten: they keep their history exactly as signed, and
  `0.966.4` is the last release that reads them.

- Five terms leave the controlled vocabulary: Finding, Frontier Commit, Review
  Packet, Frontier map, Attempt. Each named something that already had a name,
  something derived from an existing object, or something Vela does not govern.

- Schemas published for Claim Record, Proposal and Repository Origin. Those
  three had no `JsonSchema` derive, so the schema for the object carrying the
  science could not be generated at all. Eight schemas now.

- `TERMINOLOGY.md` no longer claims Claim and Evidence have no established
  equivalent. Nanopublications reached the same decomposition in 2010, and
  `paper/vela.md` already said so.

- `vela.status.v3` is a type, and a published schema. It was two `json!`
  literals, one per branch, which is how the uninitialized branch came to spell
  its own `schema` field `vela.status.v1` through the whole life of v2 and v3.
  Both branches now build one `StatusV3`, so a per-branch schema literal is
  unwritable rather than merely wrong, and `wire_schema::published()` renders it
  to `schemas/status-v3.schema.json` for the consumer that reads it —
  the Observatory in `vela-web`, which followed three shape changes here in six
  days by watching its projection refresh break. Emitted bytes are unchanged,
  measured against the pre-change binary on the Erdős Frontier.
- The status schema states that null is not absence. Every field is `required`,
  including the ones that are null on a Frontier with no commit or roots yet,
  and `conformance/verify_wire_schemas.py` holds it to that with two documents
  and twelve rejections from an independent implementation. `schemas/` now
  carries one read surface beside the four signed objects; `schemas/README.md`
  says which is which and why.
- `benchmarks/` is retired. It held the generators for the product-compression
  and erdos-264-proof-repair evaluations, and no CI job invoked them, nothing
  outside the directory imported them, and `freeze_campaign.py` requires
  `--frontier` to name erdos, formal-conjectures, quantum-codes and sidon-sets
  as live checkouts, all four of which are archived — so it could not run at
  all. The results it produced are frozen in `paper/artifacts/`, which is what
  the white paper cites, so retiring the generators invalidates no published
  number. Git history keeps the sources; `paper/README.md`,
  `paper/vela.md` and `paper/artifacts/state-lift/README.md` link into that
  history rather than into the tree — at `e6859041`, the last commit that
  carried the directory, so the citations resolve permanently instead of
  resolving to whatever `benchmarks/` next means.

- `docs/CLI.md`'s verb grid is bound to the parser. The published reference
  named the same commands a third time — after the two printed grids, which
  were bound last release — and nothing held it to them, so a renamed verb left
  the documentation site advertising a command the binary would not run. Its
  daily block, its daily table, and its advanced block are now asserted against
  `vela help` and `vela help advanced`, which are asserted against
  `Cli::command()`. `completions` stays undocumented without an allow-list
  naming it: the reference's own `Hidden utility:` group is the partition.

## v0.966.4

- One command, one document. `vela status` answered a Frontier whose repository
  authority had not finished initializing with `vela.status.v1` and a replaying
  one with `vela.status.v3` — not two versions of a contract but one literal
  that never moved when the contract did. Both branches now report
  `vela.status.v3` with the same key set: the phase travels in `integrity` and
  in a third `actions.work.mode`, `authority_uninitialized`, whose command is
  the `vela init` that clears it. `phase` and `next_action` said the same two
  things in a shape only that branch had and are gone with it.
- `review show --json` returned the Proposal's status under the key `standing`,
  the last place in the CLI where a Proposal word travelled under the Claim
  word. It is `status` now, which is what `review list` has called it on every
  row and what `--status` filters.
- Bind the printed help to the parser. `vela help` and `vela help advanced` are
  hand-set grids, and nothing held them to the commands clap accepts, so either
  could advertise a verb the binary does not have. The advanced reference is now
  asserted equal to the parsed surface and the compact grid is asserted to be
  drawn from it.
- One declaration of the Rust version. It was written out in `Cargo.toml`,
  `rust-toolchain.toml` and both workflows; the workflows now read the toolchain
  file, and a test holds `rust-version` to its `channel`.
- `docs/README.md` covers `docs/`. It had never heard of `AGENT_QUICKSTART.md`,
  `ARCHITECTURE.md` or `FRONTIER_REPOSITORY_PROFILE.md`, all three of which the
  website publishes; a test now reads the directory and holds the index to it.
- `repository_lint` gains `generator-pin`. One of the four Frontiers declared the
  shared source-manifest generator in a manifest where `uv` locks the rev; the
  other three carried the same `uvx --from git+…@rev` invocation in prose that
  nothing read, so `@main` there would have looked exactly like a pin. The rule
  refuses a generator reference naming anything but a 40-character commit, and
  refuses a Frontier that names two.

## v0.966.3 — 2026-08-06 — Answer a person, keep the contracts

- Point the published GitHub Action at `replay`. v0.966.2 renamed the integrity
  verb and retired `check` with no alias, but shipped an `action.yml` still
  invoking `check`, so every consumer Frontier's only verification gate would
  have failed on its first pin bump. The contract test that should have caught
  it asserted the literal command string, so it passed on the stale verb; it now
  asserts the shape, and a new test parses the verb out of the Action and runs
  it against the built binary.
- Answer a person when a person asks. `show`, `why` and `log` advertised
  `--json` and printed the same JSON either way, so `vela why` answered "why
  does this stand" with several hundred lines including the compaction
  predecessor archive. Each now renders for a reader, and `--json` is unchanged.
- Print the Decision's reason, not the Proposal's. `review show` and `review
  list` showed the Submission's retention boilerplate under a heading naming a
  terminal standing, and never the sentence the deciding human wrote.
- Read `standing_basis` per Claim. It was derived from the repository's
  compaction history, so every accepted Claim on a compacted Frontier reported
  `compacted_origin` — including Claims decided in the current authority chain,
  whose Events the same payload listed. The count of current-chain events now
  ships alongside so the derivation is checkable.
- Make the exit-code contract real and publish it. `fail()` hardcoded the domain
  kind across every call site, so a missing object and a malformed flag were
  indistinguishable to a caller. Unambiguous sites are reclassified, the rest
  deliberately remain domain, and `docs/CLI.md` now carries the table.
- Keep the JSON envelope on `review inbox` and `reproduce`, which had lost it.
  The inbox carries it beside the projection rather than inside, because its
  root is computed over the struct.
- Scaffold the Frontier the profile describes. `vela init` wrote an agent
  charter named `VELA.md` that no Frontier uses, and a `.gitignore` covering
  three of the nine runtime directories, so a fresh Frontier staged its task
  leases, workspaces and key material into Git.
- Report a trust-pin collision as a trust-pin collision. `vela init` called it a
  signing failure and told the operator to load an SSH key that was never the
  problem.
- Declare which Claim standings this release actually emits. Four of the six the
  vocabulary declares appear in no crate, and two that are emitted are not
  declared. The gap is now stated rather than left for a consumer to implement.
- Retire the last uses of `check` from `install.sh`, `SECURITY.md` and the
  Action's own name, collapse fourteen restatements of the sha256-root rule and
  five copies of the canonical-time parser onto one predicate each, and remove
  an unused cargo feature, an unreachable observed-preimage path and an
  actor-kind heuristic that contradicted the authoritative classification.

## v0.966.2 — 2026-08-05 — Canonical replay and actionable initialization

- Make `replay` the sole repository-integrity verb across the CLI, generated
  Frontier instructions, JSON output, and current documentation. Retire the
  ambiguous `check` command instead of retaining an alias; `reproduce` remains
  the separate operation that reruns scientific witnesses.
- Make failed first-time `vela init` actionable without adding key custody or
  another setup command: human and JSON errors now name standard OpenSSH
  recovery, preserve the exact resume command, and link to secure macOS and
  Linux Ed25519 setup instructions. Human recovery no longer switches the
  retry into JSON mode.
- Remove stale two-step initialization language from the protocol and agent
  quickstart. Current documentation now consistently describes one recoverable
  `vela init` operation.
- Adopt the problem-centred map and frontier-to-commons foundry as Vela's next
  product programme. `problems.science` will use the existing Vela Web codebase,
  root-bound read model, source registry, and release manifest; native
  Frontiers and maintained libraries retain their authority. The programme adds
  no protocol surface, package registry, or second database.

## v0.966.1 — 2026-08-04 — Linux certification correction

- Keep launchd socket discovery macOS-local without triggering an unused-mut
  error under Linux's strict clippy gate. No protocol or product behavior
  changes from `v0.966.0`.

## v0.966.0 — 2026-08-04 — Session-authenticated authority

- Replace per-signature SSH confirmation with session-authenticated local
  repository authority. A dedicated repository service key is loaded once per
  operating-system session while every Decision retains its exact Cedar,
  compare-and-swap, semantic, read-set, DSSE, and replay checks.
- Replace the broad private SSH-agent implementation with a bounded Unix
  adapter for the two standard messages Vela uses. RustCrypto's maintained
  `ssh-key` and `ssh-encoding` crates own key, signature, and RFC 4251 encoding;
  Vela owns only capped socket framing, exact Ed25519 selection, deferred
  access, DSSE PAE, and local verification. This avoids both home-grown
  cryptography and a lightly governed agent-client dependency.
- Resolve the standard macOS launchd agent at signing time when a long-running
  GUI process lacks an inherited `SSH_AUTH_SOCK`; an explicit socket remains
  authoritative and Linux behavior remains unchanged.
- Prepare and root-check a Decision once under the recovery lock instead of
  rebuilding the same repository projection several times before signing.
- Delete the remaining native Windows implementation branches and path rules.
  The supported runtime is Linux x86-64 and macOS Apple silicon; unsupported
  platform code no longer complicates the authority, trust, or write paths.

## v0.965.3 — 2026-08-04 — Focused supported release surface

- Narrow supported release distribution to Linux x86-64 and macOS Apple
  silicon. Remove the Windows artifact, installer, action path, and smoke
  harness instead of spending the release cycle on unused compatibility work.
- No protocol behavior changes from `v0.965.0`.

## v0.965.2 — 2026-08-04 — Deterministic release signer fixture

- Provision a disposable Ed25519 SSH-agent identity in the Unix release smoke
  so clean runners exercise successful one-command Frontier initialization
  deterministically.
- No protocol behavior changes from `v0.965.0`.

## v0.965.1 — 2026-08-04 — Release contract correction

- Correct the release-bundle smoke tests to exercise the current one-command
  Frontier initialization contract: `vela.frontier-init.v3`, replay-verified
  `vela.status.v3`, zero integrity blockers, and the direct-submission next step.
- No protocol behavior changes from `v0.965.0`.

## v0.965.0 — 2026-08-04 — One-command Frontier creation

- Make `vela init` create a signed, replayable Frontier in one command. Remove
  the separate `vela authority init` command and its user-visible bootstrap
  ceremony. A signing failure retains only the exact Profile; after loading an
  Ed25519 key, rerunning the same `vela init` safely completes initialization.
- Make an empty Frontier actionable: `status` and `next` now lead to direct
  Submission when no Frontier-owned Target Index exists, while configured
  Target indexes continue to lead through `next` and `start`.
- Retire the unrun multi-reviewer Result Dossier program and its qualification
  machinery. Public Dossiers remain exact read-only records; no usability,
  reviewer-efficiency, adoption, or productivity claim is made.

## v0.964.0 — 2026-08-04 — Phase-aware CLI and campaign evidence

- Make the native pre-authority Frontier phase a first-class CLI experience:
  upward discovery now finds a fresh bootstrap, every command that requires
  repository authority fails with the same structured diagnostic and exact
  recovery command, and review Decision preflight preserves the JSON error
  contract. Complete missing positional and `--json` help text and retain the
  cold-start behavior in an end-to-end regression test.

- Record two bounded Erdős 203 exclusions without importing their machinery
  into Vela: exact overlap counting, reproduced by a separately implemented
  source-first checker, excludes the 31-tile `n | 5040` family and the larger
  33-tile `n | 10080` family. A separately preregistered `n | 55440` pass
  returned `no_conclusion`, so that global inequality is retired for the next
  tranche. These results remain source-local, claim-credit-false,
  non-authoritative, and do not imply global nonexistence or Standing.
- Close the Astra release map at its current evidence ceiling and activate the
  source-owning Erdős 203 finite-cover producer lane. The existing rooted
  briefing preserves the corrected lattice kernel, retracted 99.98% result,
  exact certificate contract, and independent verifier; Astra 146/180 now have
  a passing source-first mechanical check and a chronology-preserving Erdős
  180 correction packet, while the Erdős 183 human checkpoint remains open.
- Retain the complete native Astra release-map replay: all ten advertised
  result families, twelve JSON-declared Comparator profiles, and 41 terminal
  theorem declarations pass the rooted network-disabled Linux path, Nanoda,
  and Lean's default kernel. The exact axiom audit reports only `propext`,
  `Classical.choice`, and `Quot.sound`. The unadvertised source commit, stale
  aggregate challenge target, Docker disk incidents, compatibility wrapper,
  and all scientific nonclaims remain explicit; fidelity and human Decisions
  are still open.
- Add the rooted ten-family consequence map without inventing external review
  or acceptance. Source-local matrices assess Erdős 146 as faithful producer
  evidence, preserve the verified Erdős 183 Claim as pending human Decision,
  and identify a material Erdős 180 boundary: the retained statement is
  unrestricted, while the manuscript and Lean formalize the corrected cyclic-
  family conjecture. The explicit counterexample refutes both formulations.
  A source-first checker now qualifies both matrices at their non-authoritative
  evidence ceiling, and an exact correction packet preserves both 180
  formulations without performing a source or Standing change.
- Open the ADR 0019 Level 0 experiment with a source-local exact Lean replay
  contract candidate, closed schemas, maintained RFC 8785 root construction,
  file-root verification, and fail-closed axiom vectors. The experiment adds no
  package release, CLI, Frontier lock, registry, protocol object, or authority
  effect; promotion still requires two consumers, independent root agreement,
  clean network-disabled reconstruction, and net deletion.
- Add an independent Rust/RFC 8785 reader for the frozen Lean replay candidate
  root. Agreement with the Python reader is package-integrity evidence only;
  it does not satisfy cross-platform replay or net-deletion gates.
- Retain the first package qualification as a Level 1 promotion no-go: Formal
  and Erdős consume the same exact root and leave historical authority evidence
  unchanged, but the experiment added 1,017 maintained lines and deleted zero.
  No package repository, release, CLI, index, or registry is earned.
- Retain the second frozen Erdős 730 Result Dossier instrument without
  rescoring it: all eight fields were materially correct under post-hoc review,
  but the registered Dossier median regressed by 27.60%. Retire the timing lane
  without making a usability or reviewer-efficiency claim.
- Record that Vela Web replaced the numbered Observatory read-model contract
  with the stable `vela.observatory-release-manifest` and rooted forward
  database migrations. This documentation update changes no Vela protocol
  object, writer, authority path, or release version.
- Publish checked JSON Schema 2020-12 descriptions, current-object fixtures,
  and cross-reader conformance checks for existing authority, Submission,
  Verification, and Withdrawal structures without changing their bytes,
  writers, authority, or Standing semantics.
- Accept the removable protocol-edge and independent evidence-lane boundaries.
  Vela retains its scientific semantics and human Decision authority while
  commodity standards and read projections remain replaceable.
- Reconcile the paper and cross-repository portfolio against exact Frontier and
  Vela Web evidence. Case-specific execution plans now live with their owning
  Frontier or product repository; this documentation pass is not a core release.

## v0.963.0 — 2026-08-02 — Current contract and standards boundary

- Exercise an initialized current Frontier from every staged release archive.
  The prior version-only smoke could not detect a binary built from stale
  profile-reading code.
- Remove unused Authority and Edge dependencies, drop Chrono's unused Serde
  feature, and use the existing `rand_core` operating-system RNG directly.
  This removes `rand_chacha`, `ppv-lite86`, and `zerocopy` without changing
  protocol bytes or cryptographic behavior.
- Make TOML the sole current Frontier Profile encoding. The one-time file cut
  preserves each profile's canonical JSON root while rejecting retained
  `frontier.yaml` files and removing the deprecated runtime YAML parser.
- Adopt RFC 8785 canonical JSON, DSSE authority envelopes, and the bounded
  RO-Crate 1.3 transfer profile while retaining Vela's own Decision and
  Standing semantics.
- Remove obsolete fallback readers, private event-journal fields, identity
  revocation machinery, duplicate Git/process adapters, and unused runtime
  dependencies.
- Retain the root-bound Math Source Registry and Atlas as disposable read
  projections, and keep package distribution behind the earned two-consumer
  boundary.

## v0.962.1 — 2026-08-01 — Compacted lineage and Decision freshness

- Reconstruct `vela why` across signed compacted-origin predecessors so an
  accepted Claim still exposes its Proposal, scoped Verification, human
  Decision Events, and exact standing basis after live-record cleanup.
- Re-check a reviewed Decision packet against the latest repository root before
  signing, preventing stale evidence or Standing from reaching authority.
- Retain the current exact Decision Inbox projection and the native Harbor
  quantum-correction result without adding a runner, hosted authority, or new
  protocol object.

## v0.962.0 — 2026-08-01 — Direct proposal lifecycle and native evaluation

- Add producer-owned withdrawal for one exact still-pending Proposal. The
  retained Submission identity signs the append-only lifecycle record; no
  repository-authority key, Event, Decision, or accepted-Standing mutation is
  involved.
- Replace the historical product-compression diagnostic with a native Harbor
  0.20.0 comparison that scores the exact next command and full proposed Claim
  semantics. Across four clean trials, Vela-guided work was exact twice while
  Git/files alone was exact zero times, with lower median time and cost.
- Keep Codex OAuth, execution, retries, timing, cost, and separate-verifier
  rewards inside Harbor. Vela owns only the frozen task materialization,
  semantic answer contract, exact scorer, and compact rooted result.
- Remove stale Receipt-era comments and clarify that routine evidence intake
  and producer withdrawal are not scientific authority operations.
- Remove the duplicate `doctor` projection. `status` owns frontier orientation,
  `check` owns integrity, and native Git owns worktree diagnostics.
- Remove unused agent, verifier, and provisional platform-presence authority
  adapters. Routine evidence uses its native signed records; consequential
  Decisions use the local principal plus a confirmation-constrained OpenSSH
  authority key.

## v0.961.0 — 2026-08-01 — Lean identity and release boundary

- Derive the reusable Frontier Action's Vela release from its immutable action
  commit instead of requiring consumers to supply a second version pin.
- Run the current `vela check --json` contract and remove the stale `--strict`
  flag.
- Repair Windows checksum filenames so cross-platform smoke can publish the
  release.
- Remove the unused private TypeScript protocol package and root Bun workspace;
  standalone Python and JavaScript readers keep the cross-language conformance
  boundary without a second package or version.
- Remove the duplicate `vela id` profile and keygen surface. Locally authored
  producer work now needs only `--as` or `VELA_ACTOR_ID`; the existing
  per-actor key resolver handles local custody on first use.
- Derive the actor for an imported signed Submission from its verified identity
  binding instead of requiring the importer to repeat `--as`.

## v0.960.0 — 2026-08-01 — One direct scientific-state product

- Make repository v4 the only live Frontier contract. Submission records bind
  their Proposal and Claim lineage directly; live Registration Records and
  predecessor-era compatibility readers are removed after all four canonical
  Frontiers passed exact state-preserving compaction.
- Make Target Index v5 one directly generated, atomically replaced tracked
  projection. Remove candidate, seal, apply, repair, lease, and local Attempt
  ceremony while retaining exact source, packet, repository, and Git binding.
- Authenticate routine Submission and Verification evidence with its own
  producer or verifier identity. Repository authority is used only for the
  consequential human Decision that changes Standing.
- Replace the bundled agent runner and retired Canopus product surface with
  native tools plus direct Harbor task contracts. Preserve only rooted
  benchmark evidence and Vela-specific semantic scoring.
- Add status v3 and Decision Inbox v2 so review and producer work remain
  independent, protocol-satisfied evidence is distinguished from a human
  recommendation, and every possible Decision shows its exact Standing delta.
- Remove unused capability grants, local workflow settings, vendor-specific
  agent configuration, completed recovery payloads, and the unearned reusable
  foreign-reference runtime. Historical evidence remains reproducible beside
  the paper.
- Ship one GitHub-attested `vela` binary instead of publishing five internal
  Rust crates and an unused npm package. Keep the TypeScript 0.2.0 boundary
  private and tested in the workspace until a real external consumer exists.
- Move CLI presentation out of the protocol crate, remove Tokio's unused
  runtime, replace bespoke release-trust sidecars with GitHub provenance, and
  stop running historical paper executors in routine product CI.
- Remove negative conformance machinery whose only purpose was to prove that
  retired directories and a rejected foreign-transfer feature stayed absent.
- Collapse `claim show` into the existing `show` and `why` readers, remove dead
  platform-signature smoke branches, and make the pinned composite action use
  the native installer on Unix and Windows.

- Order the Decision Inbox by actionable protocol-satisfied entries before
  blocked cleanup, while preserving oldest-first order within each group.
  This changes only the deterministic read projection and its root; it does
  not recommend or perform a Decision.
- Replace the initialized `vela.status.v2` scalar `next_action` with
  `vela.status.v3` independent `actions.review` and `actions.work` lanes.
  Pending human Decisions no longer hide a fresh Target or active Attempt, and
  producer work remains explicitly independent of scientific acceptance.
- Replace the ambiguous Decision Inbox Standing fields with one explicit,
  target-scoped `standing_delta`. The rooted v2 projection shows the affected
  Claim set, accepted Standing before/after each possible Decision, exact
  hypothetical repository roots, unchanged accepted count, and global counts.
  Inspection remains read-only and fails closed if a hypothetical transition
  changes accepted Standing outside its declared Claim scope.
- Classify Decision Inbox Verification Records as requirement-satisfying,
  complementary, or blocking with the same predicate used by the protocol
  gate. Human output now says that a satisfied protocol gate still requires
  human judgment and is not a recommendation.
- Replace ambiguous Decision Inbox `ready` counters with explicit
  `protocol_ready_count` and `protocol_blocked_count`; no authority or Standing
  behavior changes.

## v0.950.1 — 2026-07-30 — Compacted journal-chain repair

- Recognize every completed transaction in the exact repository-manifest
  transition chain bound by a signed compaction origin, rather than only the
  final predecessor transaction. This lets compact current repositories keep
  operating after predecessor object records are intentionally archived.
- Keep current-generation journals and unrelated missing postimages fail
  closed. The regression covers an earlier archived predecessor Verification,
  a mismatched compaction root, and current-generation evidence drift.
- Publish the dependency-ordered Rust crate graph before registry install
  smokes using crates.io Trusted Publishing and a short-lived GitHub OIDC
  token. Stable release automation no longer depends on an out-of-band local
  `cargo publish` ceremony or a long-lived registry secret.

## v0.950.0 — 2026-07-29 — One compact current repository

- Replace the predecessor epoch and repository-v2 readers with one signed
  repository-origin and repository-v3 boundary after all four controlled
  Frontiers passed exact state-equivalence compaction.
- Remove retired Artifact wrappers, imported-object compatibility fields,
  migration commands, one-time compaction code, and predecessor-only runtime
  schemas. Exact predecessor tags and archives remain the historical reader.
- Simplify the public verification action to verify the closed current
  repository directly. It no longer installs an obsolete out-of-band epoch
  trust pin.
- Preserve accepted Claim meaning, evidence content, relations, Standing, and
  repository authority while reducing the live protocol and operational
  surface.

- Make `authority trust pin` idempotent and allow a migrated Frontier's public
  local pin to advance only through an exact
  `--previous-record-root` compare-and-swap. The new root must still match the
  current sequence-one authority record; no key is read and no Frontier byte
  changes.
- Retire orphan Atlas summaries, derived discovery catalogs, predecessor-era
  research prototypes, the unrefined top-level Lean model suite, and its dead
  manual CI lane. Git history remains the archive.
- Keep independent Python and TypeScript reducers plus the JavaScript current
  object emitter as focused `conformance/` implementations rather than inactive
  client SDK products.
- Keep only the two active frozen-witness example sets used by public
  reproduction CI.
- Remove unconsumed source-adapter samples, predecessor schema experiments,
  unused graph-analysis projections, and the superseded
  `vela-protocol-core` Conjecture/ProofPacket crate. Current Claim, Submission,
  Verification, and repository contracts remain in the protocol waist.
- Remove Vela's abandoned research-trace, packet-export, agent-attestation,
  proposal-embedded Run/tool/permission trace, Diff-Pack attestation link,
  agent-object registry, and MCP-server configuration surfaces. Canopus owns
  optional execution Runs, traces, capsules, and evaluation; Vela retains only
  local producer identity plus exact CLI intake and canonical state.
- Make the deterministic Attempt identifier vector part of the routine
  cross-implementation conformance run instead of leaving it documented but
  unexecuted.

## v0.940.9 — 2026-07-28 — Decision publication verification repair

- Verify an accepted Decision's linked scientific event by its
  signature-independent semantic identity and exact authority transaction,
  without depending on authority-event filename order.
- Verify the transaction-owned Target Index against the exact new repository
  root before publication, then retain the tracked-HEAD check after the Git
  commit exists.
- Recovered the already-signed Erdős Decision without retrying authority,
  rewriting history, or changing any signed bytes.

## v0.940.8 — 2026-07-28 — Completed-journal rolling-head repair

- Fixed the completed-history recovery barrier so a valid later current-object
  transaction may replace the authenticated rolling `.vela/repository.json`
  head without being rejected by an older completed transaction generation.
- Kept active transaction installation and completion byte-exact, and kept
  immutable events, authority records, Proposals, Receipts, and evidence
  postimages fail closed.
- Added a regression reproducing the Erdős Verification import failure and
  proving that unrelated canonical-evidence drift still blocks recovery.

## v0.940.7 — 2026-07-28 — Current Target Index writer repair

- Fixed current Submission and Verification writers so an exact Target Index
  rebind is verified as a transaction-derived postimage before publication and
  strictly against Git only after the publication commit exists.
- Added `targets.json` to the closed public Frontier topology so the exact
  derived rebind is included in the same local commit as its repository
  transition.
- Added a clean-clone regression covering Submission registration in a current
  repository with a sealed Target Index. Tracking, repository roots, accepted
  state, and replay remain fail closed.

## v0.940.6 — 2026-07-28 — Retained artifact reference reuse repair

- Reuse an exact retained Artifact reference when a new current Submission
  binds the same root and canonical path.
- Preserve the predecessor reference bytes instead of treating its historical
  schema label or raw-hex identifier as a repository-identity collision.
- Continue to reject the same Artifact root at a different path.
- No protocol, object, authority, accepted-state, or canonical-history
  semantics change.

## v0.940.5 — 2026-07-28 — Terminal imported Verification replay repair

- Resolve a retained predecessor-scoped Verification Record through the
  Proposal's immutable rooted Claim after rejection removes that Claim from
  the pending index.
- Preserve fail-closed canonical Claim, Proposal, Submission, and full-root
  checks without rewriting imported evidence.
- Report any future post-commit verification failure as an already-committed
  repository-authority transaction so operators do not retry a Decision.
- No protocol, object, authority, or accepted-state semantics change.

## v0.940.4 — 2026-07-28 — Agent Decision boundary guidance repair

- Remove the retired protected-request language from the generated Frontier
  skill and state the current repository contract precisely: agents may
  prepare or explain one exact Decision command, but may not invoke it.
- Keep the authored plugin skill and both generated skill adapters
  byte-identical, with a regression rejecting the stale authority language.
- No protocol, repository, object, authority, or CLI execution semantics
  change.

## v0.940.3 — 2026-07-27 — Migrated Verification lineage repair

- Resolve retained predecessor-scoped Verification Records through the exact
  current Proposal and Claim `imported_from` lineage.
- Include the same exact migrated observations in `review show` and the
  repository-authority Decision Plan without rewriting their signed bytes.
- Reject mismatched Proposal, Claim, or Submission lineage and retain the
  existing strict unique-mapping requirement.

## v0.940.2 — 2026-07-27 — Current Submission integration repair

- Admit current Claim records through the canonical-evidence write boundary
  while preserving the separate Proposal and review classes.
- Verify every mutable repository-manifest root through its signed authority
  delta chain instead of comparing current bytes with the initialization
  record.
- Publish Profile v2 Submission transactions as exact local Git commits.
- Add an end-to-end authenticated Submission regression proving pending
  Standing, zero accepted-state delta, strict replay, and clean-clone
  reproduction.

## v0.940.1 — 2026-07-27 — GitHub Action attestation fix

- Expose the implicit workflow token to the composite action's release
  attestation checks, so consumers can use the documented action without
  duplicating `GH_TOKEN` wiring.
- No protocol, repository, object, authority, or CLI behavior changes.

## v0.940.0 — 2026-07-27 — One current repository contract

- Make Profile v2, Claim Record v1, Submission v1, Registration Record v1,
  Verification Record v1, Proposal v1, and repository authority the only live
  repository contract.
- Verify a signed repository-epoch boundary while removing the Era-0 parser,
  reducer, writers, compatibility projections, migration command, and
  one-time migration machinery from the daily binary.
- Initialize fresh repositories directly in Profile v2 and bind native
  `vela.repository-genesis.v1` through the same sequence-one authority
  transaction used by current repositories.
- Preserve historical audit through immutable predecessor commits, tags, and
  source archives rather than same-binary legacy replay.
- Keep the public product loop object-first:
  `inspect -> attempt -> submit -> verify -> decide -> continue`.

## v0.930.0-rc.13 — 2026-07-26 — Contract authority to standard providers

- Replace the custom protected-signer path with one attributed
  repository-authority decision: local OS principal authentication,
  restricted Cedar authorization, and exact DSSE signing through the standard
  OpenSSH agent.
- Remove the `vela-signer` crate and binary, identity-v2 custody, signer
  sessions, binary/helper rebind, platform prompt adapters, and one-time
  migration writers.
- Remove copied root/time confirmation from `review decide`; the exact command
  is the semantic human action and the recoverable transaction remains the
  write boundary.
- Contract release archives to one `vela` binary and the workspace to six
  crates while retaining byte-identical Era-0 replay.
- Retire obsolete signer-session schemas and rewrite current authority,
  command, verification, repository-profile, quickstart, and threat-model
  guidance around the single live writer.

## v0.930.0-rc.12 — 2026-07-26 — Preserve repository configuration during landing

- Keep `frontier.yaml` and every other repository-configuration surface out of
  repository-authority derived drafts. Transactional landing rematerializes
  only the closed Vela-owned view set (`frontier.json`, `vela.lock`,
  `.vela/proof-state.json`, and `proof/**`), preserving migrated frontier
  configuration byte-for-byte.
- Render those views from canonical repository bytes plus the new pending
  proposal, not from the workflow's effective in-memory lease overlay.
  Detached repository-authority claims remain available for work-session
  validation without leaking into `frontier.json` or `vela.lock`.
- Add the migrated-frontier regression exposed by the retained Erdős landing;
  the invalid rc.11 attempt failed before a transaction marker or proposal
  mutation.

## v0.930.0-rc.11 — 2026-07-26 — Materialize repository-authority landings atomically

- Install `frontier.json`, `vela.lock`, proof exports, and proof-state
  bookkeeping through the same recoverable transaction as a
  repository-authorized Receipt landing.
- Bind the exact derived postimages to the transaction journal and active
  recovery checks without adding them to the signed authority object delta or
  scientific write-set root.
- Reject derived-only authority attempts, unsupported derived paths,
  post-signing derived drift, and canonical/derived path overlap before a
  commit marker can authorize writes.
- Preserve completed-journal rematerialization behavior and every canonical
  event, Receipt, proposal, artifact, authority, and scientific-state
  commitment.

## v0.930.0-rc.10 — 2026-07-26 — Separate repository authorization from acceptance

- Record `urn:vela:policy:none` in a repository-authorized Receipt. Repository
  Cedar policy authorizes the exact canonical write; it does not constitute
  scientific acceptance and cannot occupy Receipt v1's AcceptancePolicy field.
- Keep the signed-agent `receipt_land` permission check fail-closed before
  Receipt authoring.
- Canonicalize signed agent event and activity-record authentication
  observations to whole-second UTC while retaining the full signed object root.
- Preserve every canonical Frontier byte and the independently verified Erdős
  repair artifact while unblocking its deferred, pending-review landing.

## v0.930.0-rc.9 — 2026-07-26 — Repair migrated strict state and producer preflight

- Count only strict blockers in compact status totals. Advisory migration and
  audit signals remain visible, but they no longer inflate the blocker count
  or make a migrated Frontier appear more broken than its exact strict state.
- Preflight repository-authorized producer work before any write. A missing
  routine producer policy now returns the exact protected administrator repair
  command while preserving the closed `vela.offer.v1` machine contract.
- Preserve all canonical Frontier bytes, scientific roots, authority records,
  and the rc.8 migration boundary. This candidate exists to prove the first
  useful repository-authority Receipt landing before the stable `0.930.0` cut.

## v0.930.0-rc.7 — 2026-07-25 — Stabilize generated statistics

- Serialize derived category and link-type counts through ordered maps so
  `frontier.json` is byte-identical across processes.
- Reproduce the defect against the exact Sidon Profile v1 checkout, then prove
  three-process materialization, strict replay, and unchanged canonical roots.
- Preserve the rc.6 protected migration behavior and the completed Formal
  authority history without adding a protocol or authority primitive.

## v0.930.0-rc.6 — 2026-07-25 — Canonicalize protected migration approval time

- Canonicalize the protected helper's authenticated approval observation to
  whole-second UTC before authority preflight, matching the closed
  `vela.authentication-observation.v1` contract.
- Add a regression for the nanosecond timestamp returned by the macOS helper
  and prove that the resulting local authentication observation validates
  without weakening the fail-closed transaction boundary.
- Preserve the rc.5 Target Index compatibility and all pre-migration history.

## v0.930.0-rc.5 — 2026-07-25 — Restore historical Target Index inspection

- Preserve bounded, unique Target Index v1 labels in their historical byte
  order during read-only inspection and migration planning. Target Index v2
  remains strictly sorted and canonical. This restores Quantum Codes' exact
  protected Profile v1 migration path without rewriting its derived v1 index
  or weakening producer-work sealing.
- Remove the unpublished, unconsumed `vela-hub` compatibility crate, its
  database/server dependency graph, deployment files, and active service
  documentation. Historical tags retain the complete implementation; local
  `vela serve` and the optional Observatory remain the supported read paths.
- Include `vela-authority` in release-version checks and crates.io publication
  order so the complete seven-crate `0.930` graph can be published.
- Advertise proposal-scoped reproduction only when the pending proposal retains
  a valid frontier-local frozen witness. Proposals backed by an external
  producer replay bundle remain fully inspectable, but no longer receive a
  broken `vela reproduce --proposal` next action.
- Constrain retained Receipt and witness reads to regular, non-symlink files
  inside the Frontier and verify their exact roots before use.
- Reproduce the real Quantum Codes v1 index at its unchanged Git commit and
  roots with zero writes while rejecting duplicate, oversized, invalid, or
  otherwise ambiguous legacy labels.

## v0.930.0-rc.2 — 2026-07-24 — Restore feature-independent historical replay

- Canonicalize synthetic source commitment preimages before hashing them.
  Ordinary `serde_json` object bytes changed when the CLI dependency graph
  enabled `preserve_order`, causing one historical Git anchor to derive a
  different source registry and snapshot root than the reusable library.
- Pin the same source commitment and source ID under both the narrow protocol
  test graph and the full CLI feature graph.
- Requalify the proposed repository-authority migration seam without changing
  a canonical Frontier byte or weakening repository-boundary verification.
- Supersede `v0.930.0-rc.1`, which remains an immutable failed candidate and
  must not be used for a migration ceremony.

## v0.930.0-rc.1 — 2026-07-24 — Qualify the repository-authority migration seam

- Add the proposed ADR 0020 dual-history verifier, restricted Cedar runtime,
  closed principals and capabilities, DSSE authority records, repository
  keysets, policy bundles, rotation, and terminal close.
- Add `vela authority migrate` as one temporary two-phase migration command:
  preview is key-free and write-free; apply rederives the exact plan under the
  recovery barrier, requests one final protected legacy continuity signature,
  authenticates the local operating-system principal, and records sequence 1
  through a standard OpenSSH-agent repository key.
- Bind the initial Cedar policy to one exact local administrator. Human review
  actions target proposals; an unbound human principal is denied.
- Preserve non-canonical historical event file bytes exactly while comparing
  their typed event content. Migration appends new canonical bytes and never
  normalizes or rewrites retained Era-0 history.
- Prove the composed command seam on a disposable Git Frontier, including
  cancellation before custody access, exact helper response validation,
  recoverable installation, a separate repository signature, and network-free
  clean-clone replay.
- Keep ADR 0020 Proposed. This candidate migrates no active Frontier, removes
  no Era-0 verifier, and is not the public `v0.930.0` release.
- Qualification note: live read-only replay exposed a feature-dependent source
  commitment and repository-anchor mismatch after this tag was published.
  The tag is retained as a failed candidate and superseded by `v0.930.0-rc.2`.

## v0.915.1 — 2026-07-24 — Align human and JSON strict-check verdicts

- Make the ordinary `vela check --strict` renderer use the same verified
  repository-context classification as `--json`.
- Report exact Profile v1 proposal identities retained at an independently
  pinned signed boundary as `anchored_immutable_unauthenticated` audit facts
  in both output modes.
- Continue to fail closed for every unanchored, added, altered, native
  Profile v1, legacy v0.1, or invalid-boundary proposal-ID conflict.
- Fix the real Formal Conjectures CI path without changing any event,
  proposal, reducer, scientific root, or authority rule.

## v0.915.0 — 2026-07-24 — Authenticate exact historical dependency states

- Accept ADR 0018 and allow an existing
  `vela.exact-frontier-dependency.v1` record to identify one exact retained
  ancestor of the first independently pinned repository temporalization
  boundary.
- Rederive hardened Git ancestry, the historical tree and replay, stable
  repository identity, event and proposal retention, every canonical retained
  byte, the empty dependency context, and Scientific State Root v2 before
  returning the existing dependency record.
- Continue to fail closed for forks, descendants, missing or shallow history,
  Git replacement or configuration attacks, altered retained objects,
  non-empty dependency contexts, ambiguous or invalid boundaries, wrong trust
  pins, and any expected-root mismatch.
- Pass the real read-only Erdős-to-Formal vector: the later signed Formal
  boundary authenticates Erdős's unchanged historical Formal commit, tree,
  and snapshot pin while preserving all 1,217 producer targets and the exact
  1,511-condition plus 81-actor Erdős strict debt.
- Add no event, object, signature, reducer transition, dependency schema, or
  scientific authority rule. Vela `0.914` histories replay unchanged.

## v0.914.2 — 2026-07-24 — Preserve exact legacy proposal identity debt

- Make `vela check --strict` agree with the Profile v1 repository write gate:
  a verified, externally pinned temporalization boundary may retain only the
  exact proposal logical-ID conflicts present at its Git anchor.
- Report those anchored historical IDs as
  `anchored_immutable_unauthenticated` audit facts. They confer no proposal
  authentication, acceptance, identity, or authority.
- Continue to fail closed for native Profile v1 conflicts, legacy repositories
  without a verified boundary, any new conflict, any changed conflicted
  proposal byte, an invalid boundary or trust pin, and every withdrawal
  conflict.
- Restore strict checking of the signed Formal Conjectures migration after
  ordinary regeneration of its derived `frontier.json`, `vela.lock`, and proof
  readouts. Canonical events, proposals, findings, artifacts, registries,
  accepted state, and scientific roots remain unchanged.

## v0.914.1 — 2026-07-24 — Preserve exact legacy signature debt

- Let a signed repository temporalization retain signature bytes already
  present at its exact Git/Vela anchor when the legacy display actor has no
  anchored public key. The bytes remain unauthenticated and confer no
  identity, standing, or authority.
- Continue to fail closed if such a signature is added, removed, replaced, or
  otherwise differs from the immutable anchor. Registered actors and
  repository-boundary events still require ordinary signature verification.
- Make the target-task Receipt fixture use a stable logical base path so the
  same release source passes from any clean checkout directory.
- Restore exact replay of the released Erdős Profile v1 repository without
  changing any event, proposal, Receipt, artifact, registry, policy, reducer,
  accepted-state, or scientific-state byte.

## v0.914.0 — 2026-07-23 — Bind portable frontier repositories

- Replace the permissive v0.1 repository manifest with closed Profile v1 and
  Settings v1 contracts. Minimal initialization now records the real scope,
  rejects unknown state-carrying fields, and separates human profile data,
  operational preferences, derived views, and canonical protocol bytes.
- Add one signed, non-scientific `frontier.repository_bound` event with exact
  repository identity, dependency, Git, event-prefix, registry, retained-byte,
  and Scientific State Root v2 commitments. The boundary governs repository
  administration and continuity; it grants no scientific acceptance.
- Add protected native binding and legacy temporalization ceremonies with
  proof of actor possession, out-of-band full-root trust pins, exact
  destination binding, recoverable transactions, and byte-preserving replay.
  Invalid, missing, forked, stale, or substituted context fails closed.
- Add Target Index v2 as a sealed, derived producer-work projection with exact
  candidate inputs, canonical rank, availability state, repair commands, and
  strict suppression when repository trust or roots do not verify.
- Make compact status, checks, policy administration, CI, publishing, work,
  and landing share the complete hardened repository/Git read boundary. Add a
  closed JSON command envelope and cross-implementation fixture 19.
- Harden Settings and Target Index writes with descriptor-relative no-follow
  paths, exact preimages, cooperative repository locking, atomic replacement,
  readback, durability barriers, rollback, mode repair, and hostile race/error
  regressions. These files remain non-authoritative.
- Keep native Windows read, check, reproduce, key-free preview, and protected
  signing support. The two standalone exact-preimage maintenance writes fail
  closed on native Windows until a handle-relative exchange/rollback primitive
  is proven; WSL2 with a Linux-filesystem checkout is the supported path.

## v0.913.0 — 2026-07-22 — Keep producer work and review legible

- Restore the retained assertion text in compact `finding.add` review rows so
  a reviewer can choose one proposal before opening its exact Decision Brief.
- Make `vela next` return configured producer work only. Human review remains
  in `vela review`, and advice-only structural opportunities remain in
  `vela frontier rank`; neither can masquerade as a packet-bound work offer.
- Fail closed if the derived producer offer list contains duplicate target IDs
  or disagrees with its available-work count. Change no event, Receipt,
  proposal, policy, signature, reducer, standing, or accepted-state schema.
- Remove the unshipped standalone Hub projection-indexer experiment. Canonical
  Git frontiers remain the source of truth, and hosted read models stay outside
  the Vela authority and product boundary.

## v0.912.0 — 2026-07-20 — Retain proposal-scoped verification

- Add `vela verify attach` for signed, retry-safe verifier evidence bound to
  one pending proposal, its exact claim root, execution evidence, probe
  evidence, implementation, and declared lineage.
- Add `vela reproduce --proposal` with explicit `pending_proposal` scope and no
  authority effect, plus deterministic `review show` next actions.
- Preserve all legacy attachment IDs by omitting the new optional evidence
  fields from historical records. Verification evidence remains orthogonal to
  human acceptance and cannot change accepted state by itself.

## v0.911.1 — 2026-07-20 — Make work claims retry-safe

- Return the exact active private session when the same actor repeats
  `vela work` for the same still-leased target. The retry appends no lease
  event, changes no root, and reports `idempotent: true`.
- Fail closed when a matching active lease exists but its private session is
  unavailable, instead of silently refreshing the lease and masking an
  in-flight or interrupted claim.
- Change no event, Receipt, proposal, policy, signature, reducer, or accepted
  state schema. Existing frontier history replays unchanged.

## v0.911.0 — 2026-07-20 — Explain work availability

- Preserve the compact `vela.offer.v1` target array while adding configured,
  available, and leased producer-work counts plus bounded lease summaries.
  A live lease no longer makes `vela next` claim that a frontier needs seeds.
- Add `available_work` and `leased_work` to `vela.status.v1` while preserving
  `open_work` as the configured-open compatibility count.
- Mark `vela frontier rank` as advice-only structural opportunity ranking and
  direct producers to `vela next`; graph leverage is not a work queue,
  verifier verdict, or authority decision.
- Change no event, Receipt, proposal, policy, signature, reducer, or accepted
  state schema. Existing canonical frontier history replays unchanged.

## v0.910.0 — 2026-07-19 — Cross-platform public beta

- Make the registered Ed25519 actor the only Vela identity shown in ordinary
  product language. Platform credential stores are hidden custody adapters;
  `vela id protect` uses safe one-command defaults, `vela id show` reports the
  bounded approval session, and `vela id lock` closes only that local session.
- Keep routine agent work prompt-free under exact signed policy. Ordinary help
  now teaches protected `review decide` and `policy decide` flows while hiding
  legacy batch signing, raw key import, key generation, and binary pinning.
- Publish a provenance-first six-crate package graph and prove exact
  `cargo install --locked vela-cli --version 0.910.0` on macOS, Linux, and
  Windows. GitHub bundles carry checksums, SPDX SBOMs, build attestations, and
  explicit portable/native trust metadata rather than pretending an Apple or
  Windows platform signature exists.
- Separate optional native distribution credentials from scientific identity.
  Developer ID/notarization and managed Windows Artifact Signing may add
  no-warning native tiers later; they do not block the universal source and
  package-manager beta.

- Add a Proposed AcceptancePolicy v0.3 candidate for an exact Permit lane
  scoped to one full producer credential root already retained in Receipt v1.
  V0.1/v0.2 registry-backed replay is unchanged; v0.3 requires both the four
  execution roots and an explicit producer root even for globally registered
  actors. Missing, malformed, duplicate, or substituted roots fail closed.
- Add `policy draft search-witness --from-proposal <vpr_id>` so one retained
  pending Receipt supplies all five full roots. Protected previews rederive
  the complete identity binding and display its actor, key fingerprint, and
  root. Draft output now makes the required policy-only Git commit explicit
  before the clean-checkout Decision Plan request.

- Give the unreleased public-beta train a distinct SemVer identity so candidate
  binaries cannot be mistaken for the published `v0.901.0` artifacts.
- Preserve all historical protocol bytes and released tags. This identifier is
  provenance metadata only; it does not accept ADR 0012 or satisfy platform
  signing, notarization, attestation, or fresh-install release gates.
- Make the release workflow reject any Git tag that differs from the one exact
  version shared by all Vela workspace packages, before building artifacts.
- Treat `.vela/proof-state.json` as derived export bookkeeping in new frontier
  transactions, and let completed historical journals survive a legitimate
  proof re-export. Active recovery and transaction completion still require
  exact postimages, while authority, proposals, Receipts, artifacts, and other
  canonical evidence remain fail-closed.
- Make `vela id show` use the same protected-readiness rule as ceremony and
  doctor checks: both the signer helper and the running Vela binary must match
  their authenticated pins. A rebuilt binary is now reported as stale with
  the exact rebind action instead of incorrectly appearing ready.
- Make `vela migrate --to 0.900` add the current `/.vela/work/` ignore rule as
  well as the operation-journal rule. Private work-session coordination no
  longer appears as publishable source dirt on migrated frontiers.

## v0.901.0 — 2026-07-17 — Protected decisions and producer withdrawal

- Replace ordinary batch signing with `vela review decide`: a key-free exact
  preview followed by one root- and time-bound decision card. The protected
  path accepts no key path, batch state, wildcard, `--yes`, or blanket approval.
- Add identity v2 and a one-shot `vela-signer` for macOS, Windows, and Linux.
  It uses the platform credential store, a signed 15-minute-idle/one-hour-total
  authentication session by default, and an optional per-signature mode. It
  verifies key possession and binary identity, removes the plaintext source
  after protected readback, and zeroizes transient seed buffers. Identity v1
  remains readable.
- Add an enforced `VELA_NO_USER_INTERACTION=1` latch for conformance and other
  automated contexts. Any accidental path to platform authentication or a
  decision card now fails before opening desktop UI or touching protected
  custody.
- Prepare checksum-verified paired bundles for macOS, Windows, and Linux so the
  CLI and exact helper cannot drift. The Linux bundle includes a non-caching
  polkit action; no release installs a daemon, listener, or reusable signing
  socket.
- Add the signed, Receipt-bound, accepted-state-neutral
  `proposal.withdrawn` event. Only the exact producer key embedded in Receipt
  v1 can withdraw its own pending proposal; all proposal, Receipt, artifact,
  record, and event evidence remains immutable.
- Move historical batch and detached-file `vela sign` compatibility to
  advanced help and add focused transaction, tamper, duplicate, conflict,
  custody, CLI-surface, and cross-implementation reducer vectors.
- Order the compact review queue newest-first, expose each proposal's exact
  `created_at`, and make the retired `proposals` hint name the replacement
  list/show commands. This closes the first-party cold-review discovery defect
  without changing canonical proposal bytes.
- Accept ADR 0011 after the exact release pair passes the protected Erdős
  terminal test, current-platform rebind and semantic-card review, unchanged
  root audit, and clean-clone replay.

## v0.900.2 — 2026-07-17 — Truly read-only product projections

- Make compact status and exact Decision Brief reads verify operation recovery
  before and after projection without creating frontier lock files. A
  read-only checkout now remains byte- and mode-clean while incomplete,
  corrupt, or overlapping transactions still fail closed.
- Update the shipped frontier skill to teach
  `vela finding show <dir> <vf_id> --view standing` instead of the retired
  `vela state` surface.
- Add a focused filesystem-permission regression proving the compact read path
  needs no checkout write access and creates no operational state.
- This compatible patch changes no event, reducer, Receipt, proposal,
  signature, policy, accepted-state, migration, or key-custody contract.

## v0.900.1 — 2026-07-17 — Clean legacy migration locks

- Extend the 0.900 repository migration to add the missing
  `/.vela/operation-journals/` safety rule when a legacy `.gitignore` lacks it.
  Ordinary `status`, `next`, and review reads may create an empty frontier-lock
  file; it is operational coordination, not canonical state or publishable
  evidence.
- Keep the migration preview's own recovery barrier in Git-private storage so
  checking a legacy frontier cannot dirty the checkout before the ignore rule
  is applied.
- Reproduce the defect on the migrated Sidon and formal-conjectures frontiers,
  preserve all canonical roots and scientific debt classifications, and add a
  focused regression for clean preview and exact touched-file reporting.
- This compatible patch changes no event, reducer, Receipt, proposal,
  signature, policy, accepted-state, or key-custody contract.

## v0.900.0 — 2026-07-16 — Everyday product contract

- Cut the default CLI to the twelve-command daily path and moved setup nouns
  and advanced verification into separate help sections.
- Added compact, root-bound status, producer offer, work-session, and review
  projections with explicit output budgets.
- Added minimal bounded initialization and a compact doctor response with one
  next action. Optional MCP, CI, proof, and editor scaffolding no longer ships
  in a fresh frontier.
- Added an isolated `migrate --to 0.900` preview and apply path. Migration
  removes retired manifest fields, regenerates derived views, preserves
  canonical roots and bytes, and reports stale proof pointers and artifact
  debt.
- Retired the core Atlas wrapper, Foundry and external-reproduction paths,
  legacy proposal/state/credit/publication aliases, and the old Hub wrapper.
  `vela-hub` remains a separate binary.
- Added ADR 0010 and focused regressions for help, compact contracts, minimal
  initialization, migration refusal, and old-frontier replay. This release
  adds no protocol primitive or authority surface.

## v0.800.23 — 2026-07-16 — Durable journal boundaries

- Fixed the completed-transaction recovery barrier so a later legitimate
  materialization may replace a historical derived-view postimage without
  making the frontier unavailable.
- Retained exact completed-journal verification for markers, staged blobs,
  event membership and roots, authority, public-review, canonical-evidence,
  and private-coordination postimages. Active installation and completion
  continue to verify every write class exactly.
- Added regressions proving derived re-materialization preserves event bytes
  and roots while authority drift, missing durable postimages, corrupt journal
  material, and incomplete recovery still fail closed.
- Reproduced the defect and repair against the completed Erdős governance
  journal. This patch changes no event, reducer, signature, policy, Receipt,
  scientific authority, or human key-custody contract.

## v0.800.22 — 2026-07-16 — Immutable event transactions

- Fixed repository rendering so an existing event file is retained
  byte-for-byte whenever its decoded event is unchanged. This preserves legacy
  JSON encodings across policy, decision, maintenance, and ordinary derived
  view writes.
- Extended the append-only transaction guard from actor activation to `work`
  and `land`: those commands now fail closed if their candidate removes or
  semantically changes any preexisting event. Deliberate valid signature
  acquisition remains representable through its explicit migration path.
- Added protocol and real CLI regressions proving `work` and deferred `land`
  preserve all preexisting event bytes. The defect was found by the first valid
  direct-Codex cold-use producer cell, which stopped safely after observing
  that `v0.800.21` had reserialized three fixture events.
- This patch changes no event identity, signature algorithm, reducer result,
  actor-registration semantics, policy route, accepted-state authority, or
  human key-custody boundary.

## v0.800.21 — 2026-07-16 — Byte-preserving actor activation

- Fixed the temporal actor-registration ceremony so it preserves every
  preexisting event file byte-for-byte. The recoverable transaction now rejects
  any removal or semantic mutation of an existing event and installs only the
  new signed activation event plus its derived projections.
- Added a CLI regression with deliberately noncanonical legacy bytes, including
  an explicit `signature: null` and trailing newline, and proved those bytes
  survive the real terminal ceremony unchanged.
- The verifier semantics, activation payload, event identity, authority rule,
  and scientific state are unchanged from `v0.800.20`. This patch closes the
  immutable-history migration contract before cold-use registration.

## v0.800.20 — 2026-07-16 — Temporal actor registration

- Added the signed, audit-only `actor.registration_activated` event and closed
  `vela.actor-registration-boundary.v1` payload. A temporal boundary binds one
  actor key to an exact ancestor Git commit and tree, event-log root and count,
  and actor-registry byte root.
- Strict verification now classifies exact unsigned anchor members as legacy
  and unauthenticated without attributing them to the key holder. Matching
  events absent from the anchor still require valid signatures regardless of
  timestamp. Existing actor records without a valid activation preserve the
  timeless rule.
- Added fail-closed checks for missing and shallow history, non-ancestor forks,
  wrong roots, registry tampering, duplicate boundaries, event deletion or
  mutation, signature stripping, backdating, and activation-plus-registry
  deletion from descendant history.
- Added `vela actor activate --preview` and the human-only activation ceremony.
  Scripted use requires `--yes --confirm-root <sha256:...>` from an exact
  key-free preview. The ceremony refuses agent identities, reads the matching
  key only after confirmation and a recovery barrier, installs one signed event
  through the recoverable frontier transaction, and uses exact Git publication.
- Reproduced the Erdős migration preview at its exact registration root: 81
  anchored unsigned events, 131 anchored signed events, zero later unsigned
  events, and one later signed event. No key was read and no frontier byte was
  changed.
- Rust, Python, and TypeScript reducers agree that the activation event is
  state-neutral. This release changes no scientific acceptance rule, signature
  algorithm, object family, registry service, or model authority.

## v0.800.19 — 2026-07-16 — Reload-stable policy retirement

- Kept legacy-policy retirement replay-stable by recomputing project
  statistics without re-materializing removed source associations during the
  governance-only event. A shared-source regression now proves that a signed
  retirement remains byte-stable after reload.
- Corrected the reusable GitHub Action and installation examples to pin the
  current release. `v0.800.18` published valid binaries, but its conformance
  run exposed the stale `v0.800.17` action example after publication.
- This patch changes no scientific state, authority rule, signer boundary,
  event schema, Receipt contract, or verifier semantics.

## v0.800.17 — 2026-07-16 — Bounded composition profiles

- Completed ADR 0004's removable Phase 1B reference profiles around one exact
  canonical fact manifest: a Vela resolver, correction-aware CI projection,
  accepted-state context pack, independent Reader C, and a competing
  Git/DSSE/in-toto/`science.lock` wrapper.
- Added 54 hostile fact-manifest vectors, seven three-consumer CLI cases,
  independent reader status/root parity, offline same/descendant/stale/fork
  bundle inspection, projection-deletion checks, and 13 hostile
  standards-wrapper vectors.
- Red-team review removed an ambiguous precedence rule. One delivered manifest
  may now carry one later truth-relevant change event; combined finding,
  decision, or verifier changes fail closed and must be delivered as separate
  exact manifests.
- No dependency object, event kind, command family, authority rule, automatic
  child-truth propagation, graph, cache, or hosted service is promoted. These
  are internal research profiles and engineering evidence, not independent
  interoperability, human ceremony, ecosystem adoption, or proof that Vela is
  a scientific foundation.

## v0.800.16 — 2026-07-16 — Native atlas targets

- Added the optional `vela.target-index.v1` projection so a large scientific
  atlas can expose thousands of stable, ranked targets through the ordinary
  `next -> work -> land` loop without creating a second authority store.
- `vela next` reads only the bounded derived index, filters non-open and live
  leased targets, and carries a hash-pinned packet reference plus the live
  producer-only authority ceiling. `vela work` opens and verifies only the
  selected packet, then retains the packet root, index root, current frontier
  root, and Git commit in the private session.
- Hardened the new reader against unsafe or duplicate IDs, traversal,
  symlinks, oversize indexes and packets, frontier mismatches, digest drift,
  and schema drift. Terminal entries remain explicitly addressable but are not
  suggested as open work.
- Validated the interface against the complete 1,217-problem Erdős atlas:
  652 open targets, seven paused targets, 558 completed-status targets, and 38
  frozen witnesses reproduced. This establishes native local integration, not
  acceptance of any scientific claim or completion of the pending human policy
  retirement ceremony.

## v0.800.15 — 2026-07-16 — Replay-equivalent work snapshots

- Fixed the task-first `work` transaction so its in-memory candidate
  recomputes derived project statistics before rendering `frontier.json` and
  `vela.lock`. The signed `attempt.claimed` event, visible state, lock, proof,
  Git commit, and a fresh replay now agree immediately after the claim.
- Added a public regression that runs `work` and then
  `vela check . --strict --json` without an intervening materialization.
- This patch changes no event, Receipt, authority, signer, policy, or human
  ceremony contract.

## v0.800.14 — 2026-07-15 — Task-first closure and inspectable decisions

- Completed the ordinary producer chain through one supported Receipt v1
  surface. Scientific-chain assertions now have bounded typed authoring,
  flag/file canonical-byte parity, exact retry identity, and the same
  structured landing result across CLI and MCP, including a nullable typed
  `original_route` for exact retries.
- Made `vela work` and signed zero-TTL lease release publish their exact Git
  deltas through the existing recoverable publication path. A clean-clone
  training frontier now proves `next -> work -> reproduce -> land`, Deferred
  routing, zero accepted-state delta, and replay from retained artifacts.
- Completed the Decision Brief facet inventory and finite DecisionPlan batch
  matrix. Mixed decisions remain coherent, high-risk items remain isolated,
  and every supported read transport exposes the same bounded review facts
  without adding model judgment or authority.
- Retained each new decision's canonical seven-field DecisionPlan preimage at
  its root-keyed content-addressed evidence path in the same recoverable
  transaction as the decision. A pure named-decision inspector and registered
  adversarial vectors prove exact binding, including every matched semantic
  event's historical reviewer signature, and prove the evidence remains
  non-authoritative: deleting it changes neither replay nor signed events.
- This release adds no scientific event, Receipt family, signer, policy rule,
  dependency object, or automatic truth propagation. Its fixture and local
  engineering evidence do not establish a human ceremony, independent
  adoption, outside interoperability, or a new scientific foundation.

## v0.800.13 — 2026-07-15 — Evidence closure and composition probe

- Froze the reviewed pre-ADR 0003 canonical `.vela` Git tree, every path and
  byte in that tree, and its strict replay result. The test attributes the tree
  to the recorded historical commit and prevents a fixture or manifest
  rebaseline from retaining stale provenance.
- Completed the private frontier-transaction journal failpoint coverage for
  abort and committed-conflict transitions, including operation-ID collision
  refusal, third-party-drift preservation, and exact recovery. Added a
  post-key-read/pre-marker boundary proving one fixture-key read still leaves
  zero decision or journal delta on failure.
- Made the complete Decision Brief byte-equivalent across six read transports
  after normalizing only their independent observation time, and added five
  adversarial fixtures with concrete reviewer questions for statement
  fidelity, vacuity, partial verification, contradiction blast radius, and
  contributor credit.
- Started ADR 0004 only as a removable internal experiment frozen against
  released `v0.800.12`. Exact-checkout vectors reject structural substitutions,
  while the resolver has no success state and stops at
  `unresolvable:authority_snapshot_porcelain_missing`: a derived aggregate
  cannot prove canonical replay, historical signer scope, or which attachment
  set the reviewer consumed. No public wire object, authority rule, human
  ceremony, outside-use claim, or benchmark result is added.

## v0.800.12 — 2026-07-15 — Bounded trust-edge hardening

- Made explicit Receipt v1 files and local public artifacts descriptor-bound,
  symlink-safe, and bounded before the write edge. Receipts and individual
  artifacts are capped at 8 MiB; one landing may retain at most 64 MiB of local
  public artifact bytes. Traversal, symlink, oversize, and identity drift fail
  before scientific or Git state changes. Archive-like artifacts remain opaque
  content-addressed bytes and are never expanded by review.
- Removed duplicate raw claim rendering from proposal preview and state diff.
  Hostile claims and caveats are now regression-tested across `diff`, proposal
  preview, state diff, sign preview, and status with visible escaping, bounded
  output, no command execution, and no scientific delta.
- Consolidated exact Git publication interruption controls into one private
  durability-step harness. Focused regressions cover a nonempty caller index on
  exact retry, caller-index drift after preflight, every pre-ref journal state,
  an actual compare-and-swap loss, post-ref recovery, and successful push
  followed by completion-record failure without minting or moving another
  commit.
- Replaced the Hub's unbounded review JSON response with
  `vela.hub.review.v0.2`: snapshot-bound offset pages default to 25 and cap at
  100, each ledger reports its own exact total and continuation, and policy
  counts are explicitly page-scoped. Materialized rows are loaded with the
  snapshot hash stored in the same database read snapshot, so a concurrent
  promotion can never cache or label new rows under an old continuation hash.
  The scale proof composes the existing 10,000-row typed pending catalog with
  bounded Receipt paging, task-first landing/idempotency regressions, and a
  10,000-row Hub page collector; it does not perform 10,000 redundant Git
  writes.
- Changed no Receipt, event, proposal, signature, policy-authority, or accepted
  scientific-state contract. ADR 0004 remains accepted but inactive behind the
  open ADR 0003 human and independent-adoption gates.

## v0.800.11 — 2026-07-15 — Exact fixture-signature state

- Removed a superseded detached signature from the active fixture-manifest
  slot after `v0.800.10` changed one fixture and regenerated the manifest.
  The current manifest is now honestly unsigned; no key was read and no new
  authenticity claim was created. The prior human signature remains in Git
  history.
- Added a public Rust regression that rejects any present detached signature
  unless it verifies over the exact current manifest bytes. Fixture-change
  guidance now requires retiring the prior active signature and leaves any
  later re-signing to the canonical `vela sign <manifest>` human ceremony.
- Closed the same ceremony's clear-signing gap: detached `vela sign <path>`
  now checks the operator's binary pin before resolving a key or writing a
  signature, with a regression proving stale pins fail closed.

## v0.800.10 — 2026-07-15 — Typed readiness and bounded review

- Replaced the ambiguous open/closed policy lane with one byte-level policy
  state (`absent`, `staged_unsigned`, `active`, or `broken`) and a separate
  Permit readiness (`ready`, `human_only`, or `blocked`). Missing or lifecycle-
  limited authority now routes a Permit to a human; malformed governance blocks;
  an intentional evaluator Deny remains a Deny. Status, check, doctor, policy
  show/test/log/suggest, Decision Briefs, MCP, and landing use the same
  assessment without reading a human key.
- Made exact publication no-ops explicit as `unchanged`, allowed a requested
  push to verify an unchanged tip, and made recovery recognize when an upstream
  ref already contains an earlier publication candidate. Recovery remains
  idempotent and preserves caller index, worktree, local tip, and remote
  descendants.
- Added bounded review-pressure telemetry over durable pending-proposal facts.
  A 10,000-row queue is measured without opening Receipts; queues beyond 16,384
  keep deterministic pagination while pressure becomes typed unavailable.
  Unsupported quality, independence, priority, verifier, effort, correction,
  and downstream-use metrics remain typed missing and never gain authority.
- Bounded the default Receipt parser against excessive JSON depth and 100,000-
  artifact inputs, corrected the canonical stored dependency relation to
  `depends`, and narrowed public reproducibility and adoption claims.
- Recorded accepted ADR 0004 and its experiment plan for verifiable scientific
  composition, queued behind completion of ADR 0003. It starts with four
  Codex-only diagnostic runs; no API spend, new wire primitive, human ceremony,
  or active-goal change begins during this release.

## v0.800.9 — 2026-07-15 — Deterministic hosted regression

- Removed a probabilistic ordering assumption from the legacy-retirement
  regression exposed by hosted conformance for `v0.800.8`. The test now proves
  canonical insertion against an existing proposal on either side of the new
  proposal ID, without searching for a hash with a particular ordering.
- Runtime behavior, transaction postimages, schemas, accepted state, and
  authority boundaries are unchanged from `v0.800.8`.

## v0.800.8 — 2026-07-15 — Release lint closure

- Removed the needless test-only borrow caught by the hosted all-targets Clippy
  gate for `v0.800.7`. Runtime behavior, transaction bytes, schemas, and
  authority boundaries are unchanged.
- Retains the `v0.800.7` canonical proposal-ordering fix and its exact
  prepare-materialize-recovery regression as the release candidate used for
  frontier migration.

## v0.800.7 — 2026-07-15 — Canonical proposal transaction postimages

- Kept every in-memory pending-proposal insertion in the same proposal-ID
  order used by split-repository loading. A prepared transaction's visible
  postimages therefore remain byte-identical after the official materializer
  reloads the same proposal files.
- Added the exact regression found while migrating the Erdős frontier: prepare
  bounded legacy-policy retirement, materialize, compare `frontier.json`,
  `vela.lock`, and `proof/latest.json` byte for byte, then reacquire the
  completed-journal recovery barrier successfully.
- Added a direct protocol ordering regression. Accepted events, Receipt v1,
  scientific schemas, and authority rules are unchanged; this patch reads no
  key and performs no decision.

## v0.800.6 — 2026-07-15 — Bounded legacy-policy retirement

- Added one prepare-only recovery command for unsupported prelaunch policy
  bytes. `vela policy retire-legacy` records a closed, content-addressed
  governance proposal without reading a key, validating the legacy signature,
  or granting policy authority. The existing isolated `vela sign` Decision
  Plan remains the only acceptance path.
- Bound acceptance to the exact raw active pair, fixed internally derived
  paths, the optional byte-identical same-id snapshot pair, an intact replay,
  no current policy head, and no evidence that the legacy policy admitted
  state. Drift aborts before key access; rejection preserves every byte; an
  accepted review and the bounded deletions commit in one recoverable
  transaction.
- Narrowed strict-signal heuristics so typed, non-biomedical Erdős catalogue
  records do not inherit empirical missing-condition failures and a mathematical
  “translation property” is not treated as a missing biological translation
  condition. Empirical biomedical records retain the strict checks.
- Added the `vela.policy-legacy-retirement.v1` governance proposal shape and
  its review/audit regressions. Receipt v1, accepted event, policy-lane, and
  scientific finding schemas are unchanged.

## v0.800.5 — 2026-07-15 — One executable frontier scaffold

- Removed the unlaunched multi-template `vela init --template` branch and its
  orphaned adoption scaffold. New frontiers now have one task-first path and
  one generated command list: `agents sync`, `doctor`, `status`, `next`, and
  strict `check`.
- Replaced retired generated commands (`inbox`, `integrity`, `stats`,
  `source-inbox`, `task`, `claim diff`, and `gate .`) with commands the current
  binary actually exposes. The generated charter now teaches
  `next -> work -> land -> sign`, and a regression keeps first-run guidance on
  the release surface.
- Made a fresh frontier's MCP file byte-identical to `vela agents sync` and
  explicitly limited it to the nonfinalizing draft profile. Agent tooling can
  land a Receipt, but cannot sign or finalize a proposal.
- Made an empty frontier's first `next` useful: it offers one generic
  `seed:first` producer session, without inventing scientific content or
  restoring a template system. Init/doctor command hints now shell-quote the
  frontier path and the generated MCP adapter carries no dead environment.
- Treat a completely absent optional review-policy document set as an explicit
  conservative-default warning. Partially configured or malformed policy
  documents remain release-blocking, including explicitly declared files that
  are missing; declared paths never silently fall back. A fresh frontier no
  longer contradicts its own doctor guidance.
- Made `vela doctor` a local, offline diagnostic. It no longer probes a hosted
  hub, requires a Rust toolchain outside the substrate checkout, or treats an
  occupied optional Workbench port as failure.
- Added a tag-driven two-platform release workflow. Linux x86-64 and Apple
  Silicon binaries plus installer-compatible SHA-256 companions are now built
  from the exact tag and attached to its GitHub Release. Release jobs use fixed
  runner images, exact action commits, least-privilege job permissions, and
  repository-level immutable releases.

## v0.800.4 — 2026-07-15 — Trust-boundary parity hardening

- Consolidated policy-context derivation in the protocol. Landing, replay,
  review, policy testing, and policy suggestion now use one strict builder and
  one caller-supplied observation instant. Missing or incoherent retained
  material fails closed instead of being reconstructed optimistically by the
  CLI; legacy audit paths cannot manufacture credential or assurance facts.
- Added direct regressions for detached-HEAD refusal, publication to an
  un-checked-out branch without touching the caller index, linked-worktree
  rejection, and exact post-ref index-lock recovery. These tests exercise the
  existing Git transaction rather than adding a transport or authority layer.
- Proved that flag authoring and file import retain byte-identical canonical
  Receipt v1 bytes and roots for the same facts. Landing-time activity,
  proposal, and commit identities remain separate provenance; exact retries
  on one frontier remain fully idempotent.
- Receipt-backed finding proposals now retain one typed evidence span per
  explicitly bound artifact, pointing into the canonical Receipt. The normal
  task-first result is therefore review-ready without inventing verifier,
  independence, or acceptance claims.
- No Receipt, event, policy-lane, or Decision Brief schema changed. Existing
  accepted-event bytes are not rewritten, and no human decision or key
  ceremony is part of this release.

## v0.800.3 — 2026-07-15 — Nested-workspace test portability

- Made the frontier-repository integration tests honor the explicit
  `VELA_BIN` contract, matching the release-contract tests. Parent workspaces
  can now reuse the Vela binary they already built instead of requiring a
  duplicate binary under the submodule's private `target/` directory.
- No runtime behavior, protocol schema, Receipt, verifier, accepted event, or
  materialized-frontier bytes changed.

## v0.800.2 — 2026-07-15 — External Lean boundary consolidation

- Removed the unlaunched replay-packet compatibility mode, its packet lineage
  and sealed-environment contract machinery, and the last producer-specific
  Lakefile helper from the installed external Lean verifier.
- Kept one generic external boundary: a full GitHub repository URL, commit,
  and Lean declaration are reconstructed in a Vela-controlled project and
  produce a typed draft result without gaining acceptance authority.
- Added a prelaunch regression guard so packet flags and producer-specific
  Diderot or Krafft assumptions cannot return to the installed verifier.
- Preserved historical Diderot corpus bytes as explicitly inert provenance.
  Diderot remains an early exploratory project, not a Vela partner,
  dependency, verifier, compatibility target, or release gate.
- No protocol schema, Receipt, accepted event, or materialized-frontier bytes
  changed.

## v0.800.1 — 2026-07-15 — Portable prelaunch maintenance

- Made the one-writer-path regression guard depend only on standard Unix
  tools and Git, so a clean GitHub runner checks the same surface as a local
  checkout without requiring ripgrep.
- Made agent-adapter generation use the tracked frontier manifest instead of
  ignored local state, and refreshed the generated task-first skills.
- Made cross-workspace CLI contract tests honor the suite's explicit
  `VELA_BIN`, so a clean parent checkout reuses the binary it already built.
- Moved the broad historical Lean model build to an explicit manual workflow,
  documented its custom assumptions honestly, and removed optional external
  Lean packaging assertions from routine core CI.
- Made the active packet test fixture derive its compiler version from the
  package instead of pinning the prior release.
- No protocol, schema, verifier, Receipt, or materialized-frontier bytes
  changed. Frontiers recorded with Vela `0.800.0` remain exact historical
  artifacts.

## v0.800.0 — 2026-07-14 — Task-first protocol hard cut

Vela's prelaunch protocol candidate is organized around one contribution path:
a producer emits Receipt v1, `vela land` records it, the signed policy routes
it, and a human key holder alone can make an uncovered truth-bearing decision
through `vela sign`.

- Removed unlaunched compatibility aliases and alternate writer paths,
  including direct proposal accept/reject, submit, attempt import, auto-admit,
  legacy finding apply, redundant clients, stale schemas, and obsolete
  examples.
- Removed the unlaunched acceptance-policy compatibility subsystem. Vela now
  accepts only current content-addressed policy IDs, signatures bound to
  `signed_at`, and `vela.policy-lane.v2` replay records.
- Consolidated the portable Python emitter, installed external-verifier core,
  canonical JSON reader, and conformance commands into one crate-owned resource
  bundle. Removed the duplicate checkout-only package and made the whole-body
  `vela:receipt_body` binding mandatory in the single Receipt v1 validator.
- Reduced publication to the exact reviewed Git delta.
- Rebuilt the Hub as a disposable read-only Git index over a versioned source
  catalog. It no longer registers or deprecates sources, signs records, stores
  witness objects, or writes canonical scientific state.
- Removed Carina from live code, schemas, manifests, locks, and documentation.
  Existing immutable event payloads remain readable as opaque historical data.
- Generalized the optional external Lean verifier and removed Diderot-specific
  compatibility and release checks. Diderot is an early exploratory project,
  not a Vela partner, protocol target, or release dependency.
- Added a prelaunch-surface regression gate so retired paths cannot quietly
  return before the protocol is published.
- Removed the duplicate automatic Receipt-draft workflow. Receipt conformance
  stays in the focused core gate; optional external verifier execution remains
  explicit.
