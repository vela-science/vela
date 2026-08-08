# Genesis: open models and the scientific-state control point

An application dossier, kept in Git so that every number in it can be checked
against the repository it describes rather than taken on the author's word.

- **Protocol:** Vela `v0.968.1` released; the correction work below is on `main` and unreleased at the time of writing
- **Live authority:** `vela-science/math`, repository `vrepo_56d3fdfcd34ff5c3`
- **Repository root at time of writing:** `sha256:b35b335aa76cfeaa871bb6e90c2c70e93a466e4ceac1ed2393c6edbd1c12505c`
- **Target submission date:** 24 August 2026

Every claim below is reproducible with:

```bash
git clone https://github.com/vela-science/math && vela replay math --json
```

## The problem, in one case

An AI system produces a Lean proof of Erdős problem 522. The Lean kernel
accepts it. Continuous integration is green. Every automated gate reports
success.

The proof establishes nothing. Formal Conjectures' `erdos_522` quantifies over
`KacCoefficients` whose `h_unif` uses the defaulted `volume` on a two-point
subset of ℂ, and nothing measurable satisfies that hypothesis. The statement is
vacuously true. Discharging it is sound and uninformative.

This is not a hypothetical failure mode of automated mathematics. It is a
recorded one, filed upstream as `google-deepmind/formal-conjectures#4386`, and
the reason it matters is that **no scalar verdict can express it**. `pass`
hides that nothing was proved about the problem. `fail` hides that the
development is correct. Every benchmark, leaderboard and CI badge in use today
reduces to one of those two words.

As open models get better at producing proofs, the binding constraint stops
being proof generation and becomes the ability to say precisely what a proof
established — and to change that answer later when something upstream turns out
to be wrong. That is a state problem, not a model problem.

## What Vela is

A Git-native protocol for governed, replayable scientific state. Four
boundaries, and the whole argument is in the separation:

1. A **producer** submits authenticated evidence. Producing is unprivileged.
2. A **verifier** records a scoped observation: one named property, its
   method, its environment, its outcome, and an explicit list of what it *does
   not* establish. Verifying is unprivileged, and **verification is not
   acceptance**.
3. A **repository authority** makes a signed Decision. This is the only thing
   that moves Standing, and it is the only privileged act.
4. A **projection** derives read surfaces, every row bound to the root it came
   from.

The state is bytes in a Git repository. There is no server to trust, and any
reader can recompute the whole thing.

## Evidence: two transitions, driven end to end

`vela-science/math` held zero claims, submissions, verifications and decisions
until this month. It now holds two complete transitions, which are deliberately
the two halves of the same argument.

### Erdős 522 — refused, on evidence that passed

| Property | Outcome | Verifier |
|---|---|---|
| `lean_kernel_acceptance` | **pass** | `verifier:lean-kernel-attestation-reader` |
| `statement_fidelity` | **fail** | `verifier:formal-statement-fidelity-review` |

Decision: **reject**, `vev_a5919cff136413e3`.

The protocol blocked acceptance on its own before any human ruled:

```
failing_verification                       vvr_83eb7b206c58d33a reports fail for statement_fidelity
missing_independent_passing_verification   statement_fidelity
```

The recorded reason says, in full, that the refusal is on statement grounds and
not on the proof; that the Lean development is sound; and that this authority
takes no position on the mathematics of Erdős 522.

### Erdős 321 — admitted, on three checks that disagree

| Property | Outcome | Role |
|---|---|---|
| `lean_kernel_acceptance` | **pass** | requirement-satisfying |
| `definition_correspondence` | **pass** | requirement-satisfying |
| `statement_fidelity` | **inconclusive** | complementary |

Decision: **accept**, authority record `var_29aa9a814eb87ab2`.

Inconclusive, not passing, and the reason is exact: Formal Conjectures'
`erdos_321` states `R N = answer(sorry)`, and its `isTheta`, `isBigO` and
`isLittleO` variants each state a relation against `answer(sorry)`. There is no
fixed formal statement to match or fail to match. The admitted Claim says
*candidate answer*, not *proof*, and Erdős 321 remains open.

**Read together**, the pair shows the two distinct ways one green check
misreports. In 522 the statement exists and is vacuous. In 321 the statement
does not exist. A single `outcome` column returns `pass` for both.

## The three environments

### D — benchmark-bug diagnosis

The 522 case, packaged: exact affected upstream commit
(`59f30aa314ba225fcd9268723ce8291616df1ab0`), minimal reproduction of the
defect, two scoped Verification Records reaching opposite conclusions, an
authorized rejection, and an upstream filing. An evaluation environment for
*"can the system tell you the benchmark item is broken?"* — which is the
question a proof-generating model cannot answer about itself.

### A — statement-fidelity audit

Star Fleet's `faithfulness.json` separates the machine gate from a human read
across 18 projects, with `definitions_compared` lists per entry: five
`match-with-note`, three `no-fc-statement`, and the 522 `blocked`. The 321
transition is the first instance worked through the protocol. The rest are
material for a graded set where the correct answer is frequently *neither pass
nor fail*.

### E — correction cascade

`vela correction impact` derives, from bytes alone, which Claims acquire repair
obligations when a correction is accepted, which support routes are lost and
which survive.

**What this environment does not yet contain, and why that is stated here.**
The derivation traverses `depends` and `supports` claim-to-claim edges, and
`vela.submission.v1` gives a producer no way to declare one. Every such edge in
the retained corpus was written by an earlier bulk ingest. A repository built
with today's CLI therefore records corrections and cannot record a cascade, and
the verb reports the empty cascade truthfully rather than inferring one.

That absence is a standing decision — ADR 0004, *Falsify the need for a
scientific dependency primitive* — not an oversight, and driving the first
correction end to end is the first real evidence in that lane.

## What driving this found

Two defects, both surfaced only by making a real decision rather than by
building more surface.

**Accepting a correction made the repository unreadable.** Acceptance retires
the predecessor, so it leaves the accepted index while its own Proposal stays
retained saying `accepted`. The loader read those two facts as a contradiction:
`status`, `claims`, `replay`, `why` and `review list` all failed on a
repository that had done nothing but accept a correction. A protocol whose
central move is correction could not be read after making one. Fixed in
`v0.969.0`; held shut by
`crates/vela-cli/tests/correction_impact.rs`.

**The assurance vector had no queryable home.** `scope.property` and
`scope.does_not_establish` were retained inside a JSON blob, so no surface could
count them, and `does_not_establish` was rendered only on the Proposal — a
reader arriving at a Claim saw a green check and never saw what it declined to
cover. Both are columns now.

Neither would have been found by another month of protocol work. Both were
found in the first week of using it.

## Interoperability

`docs/interop/scientific-state-profile-v1.md` states the seven contracts an
external implementation must satisfy, each paired with the conformance check
that decides whether it does. No parallel object model: the profile names the
schemas that already exist.

Two independent clean-room emitters — `conformance/emitters/javascript.mjs` and
`conformance/emitters/python.py` — produce byte-identical signed Submissions
and Verification Records without importing any Vela implementation, and CI holds
both to the same fixtures. `conformance/readers/python/canonical.py` reproduces
repository state from a clean clone with no network.

An open-model lab can therefore emit into this protocol from Python or
JavaScript today, with no dependency on Rust, on this repository, or on any
service we operate.

## What we are not building

No workbench runtime, model router, task scheduler, package registry, universal
ontology, or hosted write authority. No first-party execution surface — one was
built, was found to hold zero rows across every release, and was deleted.

Federation between two repository authorities is out of scope until one
authority has been driven through enough real decisions to know what federating
would need to preserve. That started this month.

## Verification

Everything above is checkable without contacting us.

```bash
git clone https://github.com/vela-science/math
vela replay math --json          # integrity.replay: verified, strict: pass
vela claims math --json          # 1 accepted
vela review list math --status all --json
```

The mirror at `codeberg.org/vela-science/math` is a full replica on
independent infrastructure, verified ref-for-ref on every run.
