# Independent review: Vela portable divergence reference

## Verdict

**BLOCKED** for the exact assigned review scope.

The producer commit correctly demonstrates that one authenticated Submission
can enter two repository-local authority chains and receive different local
Decisions without transporting Standing or creating global consensus. The
implementation, however, does not demonstrate the two additional evidence
properties required by this review: distinct authenticated local authority
principals, and immutable expected terminal/Event/Standing roots for both
histories.

This is an evidence-scope verdict. It does not claim that Protocol 1 requires a
different operating-system principal for every Repository. Protocol 1 makes the
Repository the authority boundary, and one principal may administer more than
one Repository. The reviewed demonstration is blocked because its assigned
acceptance criterion was stricter than that baseline contract.

## Exact binding

- Review title: `Review Vela portable divergence reference`
- Producer ref: `origin/codex/protocol1-portable-divergence-reference`
- Producer commit: `acab8a04c65d56980be8e78f2fd337570993bfff`
- Producer tree: `b4ee622e07e153fad8315440155bc9c54906368f`
- Audited base commit: `1a2e0328620b4e8c4584c3d4baf257adb11f3d45`
- Audited base tree: `1bd8ed4e11d3745f159b32f23539f5174fd44803`
- Reviewed range: `1a2e0328620b4e8c4584c3d4baf257adb11f3d45..acab8a04c65d56980be8e78f2fd337570993bfff`
- Reviewed delta: 9 paths, 669 insertions, 15 deletions
- Live `origin/main` at final compatibility check: `2b8d43ed50a9639dfc18c5f6f21677021f70a4b2`
- Live-main tree: `2a15e11af6aab2fc4574df940ec78de4ba29fdd8`
- Clean merge-tree result: `9a146c97ec8106925a84fce320fe34e3fdf8a164`
- Review time: `2026-08-21T15:49:09Z`

The producer commit was fetched from GitHub and reconstructed in a fresh full
clone. Its commit, tree, parent, changed paths, and diff size matched the
handoff. The producer branch was not modified or merged.

## Blocking findings

### PD-1: the two authority records authenticate the same local principal

The test creates two SSH agents and two repository service keys, but both
`vela init` and both Decisions execute on the same host under the same effective
UID. Current Vela constructs the authenticated authority principal only from
the runtime device identifier and effective UID:

```text
local:device-sha256:<sha256(device_identifier)>|uid:<geteuid>
```

Changing `HOME`, Repository path, Repository ID, or `SSH_AUTH_SOCK` does not
change that principal. The separate SSH agents establish distinct repository
service identities; they do not establish distinct authenticated principals.
The different `--as` values establish distinct Decision performers, but
performer attribution explicitly grants no authority.

The reviewed test asserts distinct Repository IDs, service key IDs,
sequence-one record roots, keyset roots, authorization-model roots, trust-pin
paths, Decision-record roots, and terminal Repository roots. It does not read
or compare the authority records' `principal_id`, and under the current
construction those two values are equal.

This distinction is part of Protocol 1 section 4.2: the repository service
identity, authenticated principal, and retained authorization decision are
separate facts. The producer's same-principal histories remain valid
repository-local histories, but they do not satisfy the assigned requirement
for **genuinely distinct local authority principals and local Decisions**.

Required resolution: retain or generate two synthetic histories under genuinely
distinct authenticated device/UID contexts (or an equivalent current
Protocol-1-valid human/agent/workload principal path), then assert the full
authority-record principal IDs are distinct in addition to the already distinct
Repository resources, service keys, Decisions, and roots. This does not require
a panel, extra approval ceremony, generic policy language, or a Core schema
change.

### PD-2: terminal histories have no immutable expected root binding

The portable Submission, Artifact, derived Claim, example files, and Protocol 1
manifest have fixed roots. The two complete Repository histories do not.
Fresh UUIDs, keys, and timestamps make the terminal Git commits, trees,
sequence-one roots, Decision roots, Event-log roots, Repository roots, and
Standing receipts run-specific.

The test correctly checks that the accept and reject roots differ and that a
clean clone reproduces the root, commit, tree, and accepted/pending counts of
its source run. That is useful deterministic replay coverage. It is not an
immutable expected-root vector for either history: `flow.json` records boolean
relationships, not the exact terminal roots, and the test deletes the generated
histories when it exits.

Required resolution: freeze two synthetic, non-authoritative complete-history
fixtures or durable machine-readable replay receipts. Bind for each history at
least the Git commit/tree, Repository origin and sequence-one authority root,
authority head/Decision record, Event-log root, terminal Repository root, and
Standing counts. Independently replay those exact bytes and compare against the
bound values. Keep the material informative unless a separate protocol reason
earns normative status.

## Passing evidence

- The exact Submission file independently rehashed to
  `sha256:f1669cdfa498ff85c162bce6173f04b39cdf7620fb198a19b45f6d932302204a`.
- The exact Artifact independently rehashed to
  `sha256:084c799cd551dd1d8d5c5f9a5d593b2e931f5e36122ee5c793c1d08a19839cc0`.
- Rust, Python, and JavaScript current-object readers/emitters confirm the
  producer signature, identity `agent:independent-js`, canonical payload, full
  root, and derived handle.
- Both repositories retain byte-identical authenticated Submission bytes and
  derive Claim
  `vcl_cea6cdb3e9fd02fae86886a0edbe51e5c2fe2d5e00dc7f264d4c3de0f9f2c422`
  at root
  `sha256:e865c5a2aafd459d52d9b1c8a7734104b1e2d8d1c047c5400684f01505f83632`.
- The accept history imports one scoped independent pass and replays with one
  accepted Claim and zero pending Claims.
- The reject history imports no local Verification and replays with zero
  accepted Claims and zero pending Claims.
- Both histories have distinct Repository resources, service keys, keysets,
  authorization-model roots, sequence-one roots, Decision-record roots, and
  terminal Repository roots.
- Each clean clone reproduces its source run's Git commit, Git tree, Repository
  root, and accepted/pending Standing counts.
- The example is synthetic, non-authoritative, and explicit that it does not
  establish scientific truth, external adoption, global consensus, authority
  transfer, Standing transport, or a Protocol 1.0 release.
- No runtime object, schema, generic policy language, or Core semantic surface
  changed.
- The producer diff applies cleanly over the independently refreshed live main
  by `git merge-tree`; this is compatibility evidence only, not merge approval.

## Independent checks

All commands ran against the reconstructed producer commit and passed:

```text
cargo fmt --all -- --check
uv run --project conformance --locked ruff check conformance/verify_reference_flows.py
cargo test --locked -p vela-cli --test portable_divergence
  1 passed; 0 failed
cargo test --locked -p vela-protocol --test object_interop
  4 passed; 0 failed
cargo clippy --locked -p vela-cli --all-targets -- -D warnings
PYTHONDONTWRITEBYTECODE=1 uv run --project conformance --locked python conformance/verify.py
  PASS; 77 normative files; 36 informative files
  protocol manifest root sha256:08db975c7d8ee797f0a8898e73d51c021306869b3c891c05ded68ab508e3aaac
git diff --check 1a2e0328620b4e8c4584c3d4baf257adb11f3d45..acab8a04c65d56980be8e78f2fd337570993bfff
```

Passing checks establish implementation consistency for the exercised bytes.
They do not cure the two missing review-evidence properties above. The focused
test deliberately creates disposable synthetic authority records and divergent
Standing, then deletes those temporary Repositories; this review artifact has
no authority or Standing effect, and no production or scientific Repository was
modified. Nothing here constitutes a release or merge approval.
