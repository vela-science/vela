# VELA-RC-1 R3 CLI and first-user documentation qualification

Recorded: 2026-08-26, America/Toronto.

## Verdict

```text
PASS WITH DOCUMENTED LIMITATIONS
```

The exact candidate's release-facing documentation and CLI now support a
technically capable reader with no Vela history through the first public read
and the governed write loop without collapsing Proposal, Verification,
Decision, or Standing. The Quickstart identifies Vela as version control for
scientific state and, more precisely, as a protocol and toolchain for governed,
replayable scientific-state transitions.

No product semantic blocker or release-blocking CLI defect reproduced. The
published signed `v0.977.4` binary remains an ancestor rather than the
unpublished candidate, and a blind external user has not yet run the revised
material. Those limitations belong to the later packaging/version and R7
gates. This R3 verdict does not authorize a version change, tag, push,
publication, signing, or release.

## Exact binding

| Field | Exact value |
| --- | --- |
| Delegated supervisor commit | `431120a6995d1b24ae2d50d6889878ce43efcd97` |
| Delegated supervisor tree | `8fa55a2094569561d582629708f5588e4b5cc3ef` |
| Initial checkout | clean, detached at the exact supervisor commit |
| Vela | `0.977.4` |
| Protocol | Vela Protocol 1 release candidate |
| Scope | CLI, README, Quickstart, concepts, Protocol, conformance, examples, limitations, and first-user documentation only |

R3 read `AGENTS.md`, `README.md`, and every existing VELA-RC-1 campaign record
before acting. It then independently audited the current documentation index,
Quickstart, CLI contract, glossary, Verification guide, Protocol, roots,
conformance corpus, reference examples, repository boundaries, evidence
limitations, installer, and candidate help.

## First 30 minutes: before

The release-facing entry points already preserved the central authority rule,
but a new reader encountered these concrete failures or gaps:

1. `docs/ROOTS.md` said the Proposal root covered its record and status even
   though `vela.proposal.v1` stores no status. Status is derived from Proposal
   withdrawals and governed Decision Events.
2. The RC-1 release checklist requested a nonexistent standalone Standing
   digest instead of the accepted set, Repository root, and authority Event-log
   root that Protocol 1 actually publishes.
3. The complete semantic scenario matrix was campaign-only rather than a
   compact public implementer index.
4. The signed installer path named no compact platform, download, verifier,
   archive, network, or writable-prefix prerequisites.
5. The manual Method commit required Git `user.name` and `user.email`, but the
   Quickstart did not say so.
6. The Quickstart authored a Submission with no `--requires-verification`, then
   asked `verification record` to infer its property. The exact candidate
   refuses that shape with `--property is required because this Submission has
   no registered verification requirement`.
7. `examples/formal-math/README.md` and the current Genesis integration guide
   invoked governed Math reads without first installing the independently
   published sequence-one trust root.
8. The documentation index omitted the already-recorded R2 independent
   requalification, the read-surface fixture called its v3 contract v2, and a
   correction example described the unreleased Protocol 1 candidate as Vela
   1.0.

The first documentation-contract run retained one useful audit failure: 13 of
14 tests passed, while the complete-document index test identified the missing
`R2_REQUALIFICATION.md` link. The link was added and the same target then passed
14 of 14.

## First 30 minutes: after

The revised Quickstart gives the reader the five required answers before the
first install command:

- **What Vela is:** version control for scientific state, qualified as a
  protocol and toolchain for governed, replayable scientific-state transitions
  in ordinary Git repositories.
- **What changes Standing:** only an attributed Decision admitted through
  Repository authority.
- **What Verification is:** one scoped observation over exact retained inputs;
  pass or fail remains evidence and changes no Standing.
- **What replay means:** validation and reconstruction of retained objects,
  roots, signatures, authority history, Artifacts, Events, and derived
  Standing. It does not execute the source-owned scientific Method.
- **What Vela does not do:** it does not choose or perform scientific work,
  replace native tools or scientific judgment, or turn a check, signature, Git
  commit, or Web view into acceptance.

The public install section now names its supported platforms and actual local
prerequisites. The write path configures a Repository-local Git author identity
and gives the Submission the exact verification requirement that the later
Verification Record consumes. The formal-math and Genesis examples install the
separately published trust root before governed reads. Pinning grants no
authority and changes no Repository byte.

`conformance/README.md` now maps Submission, pass/fail/contradictory
Verification, unauthorized and authorized Decisions, rejection, correction,
supersession, retraction, withdrawal, retained history, clean replay, Artifact
failure, trust selection, Method drift, canonical bytes, and portable local
authority to executable fixtures and required state effects. The index is
informative; it does not make campaign prose normative or turn conformance into
acceptance.

No CLI command, protocol object, schema, authority path, product UI, package,
or version was added or redesigned. Two informative example/read-contract
files changed, so the exact Protocol 1 manifest was regenerated. The normative
selection remains 77 files; the informative selection remains 44 files. The
new manifest root is
`sha256:5be464c8c5968c93f2cabf2e73290894f9120963d3966482b27e970798586d97`.

## Exact verification

Passed:

```text
cargo test --locked -p vela-protocol --test cli_release_contract
```

Result: `PASS`, 14 passed and 0 failed. This binds the public Math acquisition
recipes to independent trust pinning, the Quickstart to its install/Git/
Verification prerequisites and semantic boundaries, the release checklist and
Proposal-root wording to actual Protocol commitments, the scenario index to
the required cases, the documentation index to every current document, and the
documented CLI surface to candidate help.

```text
cargo test --locked -p vela-cli verification::tests::sole_registered_requirement_is_the_default_verification_property
```

Result: `PASS`, 1 passed and 0 failed. This confirms why the corrected
Quickstart declares one exact verification requirement and may then omit
`--property` from `verification record`.

```text
cargo test --locked -p vela-cli --features test-support \
  --test wording_contract the_cli_speaks_the_vocabulary_the_protocol_fixes
```

Result: `PASS`, 1 passed and 0 failed.

```text
cargo test --locked -p vela-cli --features test-support \
  --test review_acceptance --test disposable_rejection_lifecycle --test genesis
```

Result: `PASS`, 6 passed and 0 failed across the three integration targets.
This covers authenticated Submission, passing/failing/contradictory
Verification, acceptance and rejection, independent trust selection, routine
evidence writes, Decision-only Standing changes, clean-clone replay, and
Artifact integrity.

```text
cargo test --locked -p vela-cli --test neutral_replay_fixture
```

Result: `PASS`, 1 passed and 0 failed.

```text
uv run --project conformance --locked python conformance/verify.py
```

Result: `PASS`, including 77 normative files, 44 informative files, 14 schemas,
18 positive objects, 37 negative schema cases, 179 portable patterns, the
six-record/eleven-Event authority chain, thirteen authority falsifiers, four
reference flows, Decision Inbox v3, and Protocol 1 root
`sha256:5be464c8c5968c93f2cabf2e73290894f9120963d3966482b27e970798586d97`.

```text
VELA_BIN="$PWD/target/debug/vela" examples/neutral-replay/check.sh
```

Result: `PASS`, `neutral replay fixture: ok`; the script removed the temporary
fixture trust pin it created.

The published install path was also exercised into a temporary prefix on the
current macOS Apple-silicon host with
`VELA_REQUIRE_SIGNED_MANIFEST=1`. The provider-independent manifest signature,
archive checksum, install, `--version`, and compact help passed. The installed
published binary reported `vela 0.977.4` and SHA-256
`06f912d107d29e4ce1dadd19bf7ef849ec42d7e62cbc9332c9807e6b8c9bd05e`.
That digest identifies the published ancestor only; it is not candidate or
prospective release evidence.

```text
cargo fmt --all -- --check
git diff --check
```

Result: `PASS`; Rust formatting and patch whitespace matched the repository
contracts.

## Remaining limitations

- The exact candidate is not a signed published artifact. The public installer
  still installs the already-released `v0.977.4` ancestor, whose post-install
  hint predates the candidate's mandatory trust-pin wording and omits the
  Verification requirement. R6 and the later authorized release process must
  bind any release bytes and help to the qualified candidate; R3 did not edit
  packaging, versions, tags, or publication state.
- This was an expert simulation and executable contract audit, not the blind
  R7 external-user gate. Placeholder scientific evidence, Methods, actors, and
  reasons still require the user to supply a real bounded case.
- Direct CLI retraction authoring remains unavailable; imported authenticated
  Submission v3 and the Decision kernel support it. Current CLI authoring also
  creates no `depends` or `supports` edges, so new Repositories have no
  correction cascade to project. Both are existing documented ergonomic
  limitations, not R3 release blockers.
- R3 did not rerun the complete Core union or workspace clippy because no
  product code, normative Protocol text, schema, or shared type changed. The
  focused CLI, documentation, neutral replay, and portable conformance gates
  are the owning checks; exact release-wide certification remains a later
  release-integrity action.

Subject to those explicit boundaries, R3 returns
`PASS WITH DOCUMENTED LIMITATIONS`.
