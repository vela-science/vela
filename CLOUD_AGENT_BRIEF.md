# Brief: consolidate, remove legacy, and modernize `vela-science/vela`

Read this whole brief before touching anything. It states what is true as of
2026-08-08, what you must not do, and how the work is judged. Where it says
*verify first*, verify — a change made on a stale premise is worse than no
change, and parts of this brief will go stale.

**Scope: this repository only.** `vela-science/vela-web` (the projection and the
Observatory) and `vela-science/math` (the live authority) are out of scope. Do
not clone them, do not edit them, and do not make a change here whose only
purpose is to serve them. Where a change here would *break* them, say so.

---

## 1. What this repository is

A Rust workspace holding the Vela protocol, its CLI, and its conformance suite.
Currently `v0.969.0`, published and signed.

```
crates/vela-protocol     objects, roots, canonical bytes, replay
crates/vela-authority    signatures, policy, the Decision boundary
crates/vela-verify       verification records
crates/vela-cli          the `vela` binary — 16 verbs
crates/vela-edge         analysis: correction impact, target index. Read-only,
                         never required for replay
conformance/             fixtures, vectors, and two independent emitters
docs/                    6,020 lines across 27 files
```

The layering rule, from `docs/ECOSYSTEM.md` §8: the kernel depends on nothing
above it, `vela-edge` is never required for replay, and the readers and emitters
are independent implementations of the same bytes. Do not violate it.

---

## 2. What you must not do

**Never write a hash, root, commit, digest, or id you did not compute locally or
read from a real remote.** Every `sha256:` here is a hash preimage or a binding
to one. An invented value passes every syntactic check and corrupts the record
silently. If you need a digest, compute it.

**Never edit retained paths.** `conformance/fixtures/**`, `paper/**`,
`docs/adr/**`, `docs/history/**`. These are frozen history. An ADR that is wrong
is superseded by a new ADR, never edited.

**Never change canonical bytes without intending to.** Anything that alters what
`to_canonical_bytes` produces for an existing object — a field name, a field
order dependency, a serialization attribute — moves every root derived from it
and breaks every published repository. `conformance/verify_canonical_hashing.py`
is the check; if it goes red, stop.

**You cannot sign or publish a release.** That needs the distribution key in a
local ssh-agent. You may prepare a version bump; the tag, the signing and the
publish are the operator's. `crates/vela-protocol/tests/cli_release_contract.rs`
will fail if `README.md` names a version that is not a real tag — that guard is
correct, do not weaken it.

---

## 3. House standards

The codebase has a voice. Match it.

- **Comments explain *why*, and cite evidence.** The good ones here name a
  measurement, a failed CI run, a commit, or a specific defect. Comments that
  restate the code are noise. When you delete code, the comment explaining why
  it was wrong is often the part worth keeping.
- **State limits plainly.** A check that overstates itself is worse than none.
  Several recent defects were docs claiming a check that did not exist.
- **No inert surfaces.** A flag nothing reads, an export nothing imports, a
  vocabulary term nothing emits, a doc describing a deleted thing — each is a
  claim the codebase cannot cash. This is the dominant defect class here.
- **Prose:** restraint, concrete before abstract, trust the reader. No em-dash
  pile-ups, no staccato fragments, no grandiosity.
- Read `git log` for the register. Commit messages explain reasoning and name
  evidence.

---

## 4. The work

### 4a. Documentation is the largest legacy surface

6,020 lines across 27 files, and measurably drifted. These are verified, not
recalled:

1. **`docs/ECOSYSTEM.md` §8 says the CLI has "15 verbs" and lists them.** There
   are 16 — `correction` is missing from the list. The same block names
   `conformance/emitters/javascript.mjs` as the emitter layer; there are two
   emitters now, `javascript.mjs` and `python.py`.

2. **Retired vocabulary is still in the docs.** `docs/ECOSYSTEM.md` §7 keeps an
   explicit list of names that are "already retired and not to be reintroduced"
   and a longer list of "invented and unbuilt — delete rather than re-base".
   Grep across `docs/*.md` finds live occurrences anyway: `Finding` ×4,
   `Frontier Commit` ×2, `Attempt` ×2, `Lens` ×2, `Review Packet`,
   `Registration Record`, `Capsule`, `FrontierBench`. Each needs reading in
   context — some will be legitimate historical references, some are drift.
   Distinguish them; do not blanket-replace.

3. **Campaign documents that may be spent.** `docs/CAMPAIGN.md` (58 lines),
   `docs/PORTABLE_WAIST_CAMPAIGN.md` (73), `docs/BREAKTHROUGH_BENCHMARK.md`
   (323, whose own status line reads "historical fixture entry gate failed").
   Each has inbound references (4, 1, and 5 respectively), so none can simply be
   deleted. **Verify what each still governs.** A campaign that concluded should
   say so and stop reading as active direction; one still live should stay.

4. Sweep the rest for the same: a documented flag that no longer exists, a
   worked example whose output has changed, a file path that moved.

### 4b. Code consolidation

- `#[allow(dead_code)]`, `todo!()`, `unimplemented!()` and `FIXME` appear in
  `crates/vela-edge/src/analysis/repository_write.rs`, `crates/vela-cli/src/ui.rs`
  and `crates/vela-cli/src/cli/records.rs`. Read each. Some allowances are load
  bearing (a `pub` item a test uses); some are the residue of something removed.
- Two `#[command(hide = true)]` verbs in `crates/vela-cli/src/command_spec.rs`.
  A hidden verb is either an operator tool that should be documented as one, or
  a surface nobody reaches. Decide which, per verb.
- Look for parallel implementations of one concept, dead `pub` exports, and
  constants duplicated across crates. `cargo clippy` is already clean at zero
  warnings, so the remaining duplication is the kind a linter cannot see.

### 4c. Modernization

Bring the workspace to current standards **without changing behaviour**. Edition
and lint level are set in the workspace root; keep them. Prefer: removing a
workaround whose cause is gone, replacing a hand-rolled helper with a standard
library one, tightening a type that was loose because something upstream used to
be. Do not run a dependency-bump sweep — Dependabot owns that, and a large
unreviewable `cargo update` diff is worse than the drift it fixes.

### 4d. Two decisions to write up, not to make

Both need an ADR proposing options and evidence. Write the ADR; do not
implement. The ruling is the operator's.

1. **`vela.submission.v1` has no way to declare a dependency.**
   `crates/vela-edge/src/analysis/correction_impact.rs` traverses `depends` and
   `supports` claim-to-claim edges, and the write path authors neither — every
   such edge in the retained corpus came from the epoch-1 ingest. A repository
   built with today's CLI can therefore record a correction and cannot record a
   cascade, and `vela correction impact` correctly reports an empty one. That
   absence is ADR 0004's standing position (*Falsify the need for a scientific
   dependency primitive*). Driving the first real correction through the
   protocol is the first evidence in that lane. Closing it means a field on a
   signed object, which moves canonical bytes.

2. **Contract 4 of the interop profile has no language-independent conformance
   vector.** `docs/interop/scientific-state-profile-v1.md` says so itself:
   contracts 1, 3 and 7 have fixtures a foreign implementation can be held to,
   and authority does not, so an implementation claiming to verify an authority
   chain currently takes its own word for it. The ADR should weigh what such a
   vector would have to contain — a signed record chain and a trust-root pin,
   without shipping a usable private key.

### 4e. A known limitation worth documenting properly

The four archived repositories (`erdos-frontier`, `sidon-frontier`,
`quantum-codes-frontier`, `formal-conjectures-frontier`) are epoch-1: their
profiles declare `frontier_id`, not `repository_id`, so the current CLI cannot
read them at all. 2,844 accepted Claims are retained and unreadable by the tool
that wrote them. Whether that is acceptable is a real question — the answer may
well be yes, since ADR 0039 made the break deliberately and §10.5 re-admission
is the sanctioned path. Either way it should be stated where a reader would look
rather than discovered.

---

## 5. How to verify

Run these. Do not assume.

```bash
cargo test --workspace                    # 38 suites, 0 failed
cargo clippy --workspace --all-targets    # 0 warnings
cargo fmt --all
uv run --project conformance python conformance/verify.py
uv run --project conformance ruff check conformance/
python3 scripts/ecosystem-status.py --check
```

`ecosystem-status.py --check` holds `ecosystem-status.json` to the checkout it
describes — it is the thing that catches a doc claiming a state the repository
is not in. If you change something it observes, regenerate with
`python3 scripts/ecosystem-status.py` and commit the result.

`conformance/verify.py` runs the canonical-hashing vectors, the current-object
waist, the wire schemas, the correction-impact fixtures, and both independent
emitters. It is the check that a foreign implementation would still agree with
you.

---

## 6. What good looks like

- Every deletion is justified by a measurement you state, not a hunch.
- Canonical bytes do not move. If they must, that is an ADR, not a cleanup.
- The inert-surface count goes down and does not go up. If you add an export, a
  flag, or a vocabulary term, something uses it in the same change.
- Where you found a defect, a test fails without your fix.
- Where you could not fix something, the repository says so plainly, in the
  place a reader would look.
- Commits are readable in a year.

Work in branches and open pull requests. If a change would move a published
root, alter retained history, or need a key you do not have — stop and report
it instead of working around it.
