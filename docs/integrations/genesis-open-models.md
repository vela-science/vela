# Genesis: open models and the scientific-state control point

An application dossier, kept in Git so that its numbers can be checked against
the repositories they come from rather than taken on the author's word. The
repository facts and both transitions are in `vela-science/math`; the
faithfulness corpus counts are in `williamjblair/lean-proofs` at the commit
named below, and the formal statements in `google-deepmind/formal-conjectures`
at the pages commit named below. Three clones, each pinned.

- **Protocol:** Vela `v0.972.1`
- **Live authority:** `vela-science/math`, Repository UUID `8115c538-7688-40b7-ab75-3c4765bf3c19`
- **Current Repository root:** `sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc`
- **Current bounded Standing:** corrected Erdős 321 correspondence Claim `vcl_3d4fd59554ccaa2b792b08abae16a8d0fe329d4901ad798fe05c6c7769c9966b`; no resolution or optimality claim
- **Retained predecessor:** the 0.971.0 decisions discussed below remain signed continuity evidence and carry no Standing into the UUIDv4 genesis
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

At the start of this case, `vela-science/math` held zero Claims, Submissions,
Verifications, and Decisions. The retained 0.971.0 predecessor then recorded a
refusal, an admission, and a correction of that admission. Under Vela 0.972.1,
the authority made the migration boundary explicit: it rejected the overstated
321 proposal, rejected the vacuous 522 proposal, and accepted only the corrected
bounded 321 correspondence. The current repository therefore has one accepted
Claim, two rejected Proposals, three Submissions, and seven Verification
Records.

### Erdős 522 — refused, on evidence that passed

| Property | Outcome | Verifier |
|---|---|---|
| `lean_kernel_acceptance` | **pass** | `verifier:lean-kernel-attestation-reader` |
| `statement_fidelity` | **fail** | `verifier:formal-statement-fidelity-review` |

Decision: **reject**, applied as `vev_a5919cff136413e3`, authority record
`var_ff733ab6a08f6a9f`.

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

Decision: **accept**, event `vev_43eca817a4a044af`, applied as
`vev_61a6ddd9d1f8d59f`, authority record `var_29aa9a814eb87ab2`.

Inconclusive, not passing, and the reason is exact: Formal Conjectures'
`erdos_321` states `R N = answer(sorry)`, and its `isTheta`, `isBigO` and
`isLittleO` variants each state a relation against `answer(sorry)`. There is no
fixed formal statement to match or fail to match. The admitted Claim says
*candidate answer*, not *proof*, and Erdős 321 remains open.

**Read together**, the pair shows two distinct ways one green check misreports.
In 522 there is a statement to discharge, and discharging it establishes
nothing, because its hypothesis is unsatisfiable. In 321 the question's own
right-hand side is still a placeholder.

### The 321 record, corrected

An adversarial review of this repository's own records found two overstatements
in the 321 evidence, and both are now corrected through the protocol rather than
by editing a retained file.

The evidence said four defining notions corresponded exactly. Three do;
`Admissible N A` is the conjunction `A ⊆ Finset.Icc 1 N ∧ Valid A`, so pairing
it against the subset condition alone was a one-way implication counted as an
identity, and its second conjunct was already another pair. And it said, without
qualification, that no fixed formal statement exists — while `321.lean` carries
`erdos_321.variants.lower` and `.upper`, both `research solved`, both complete
statements about `R N` with no placeholder, and both the same two-sided shape
this development proves. The true statement is that none exists for the *open*
question, and that the comparison against those two has still not been made.

Decision: **accept the correction**, authority record `var_ae99ca528cae8078`.
The predecessor is retired; its bytes stay retained.

The denotational conclusion did not move. What moved is how it was argued, and
the record now says which of the two it was. That is the whole mechanism working
on its author: nothing was rewritten, a correction was submitted, verified and
ruled on, and the repository is readable afterwards — which it was not, three
days ago, for any repository that accepted a correction at all. A single `outcome` column returns `pass` for both.

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
across 18 projects: 10 `match`, 4 `match-with-note`, 3 `no-fc-statement`, and
the 522 `blocked`. Fourteen carry a `definitions_compared` list; four (320, 336,
522, 662) carry an empty one, which is itself a result — it records that the
comparison could not be made rather than that it passed. The 321 transition is
the first instance worked through the protocol. The rest are material for a
graded set where the correct answer is frequently *neither pass nor fail*.

### E — correction cascade

`vela correction impact` derives, from bytes alone, which Claims acquire repair
obligations when a correction is accepted, which support routes are lost and
which survive.

**What this environment does not yet contain, and why that is stated here.**
The derivation traverses `depends` and `supports` claim-to-claim edges, and
`vela.submission.v2` gives a producer no way to declare one. Every such edge in
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
central move is correction could not be read after making one. Fixed on
the current Vela 0.972.1 release; held shut by
`crates/vela-cli/tests/correction_impact.rs`.

**The assurance vector had no queryable home.** `scope.property` and
`scope.does_not_establish` were retained inside a JSON blob, so no surface could
count them, and `does_not_establish` was rendered only on the Proposal — a
reader arriving at a Claim saw a green check and never saw what it declined to
cover. Both are columns now.

Neither would have been found by another month of protocol work. Both were
found in the first week of using it.

## Interoperability

`docs/interop/scientific-state-profile.md` states the seven contracts an
external implementation must satisfy, each paired with the conformance check
that decides whether it does. No parallel object model: the profile names the
schemas that already exist.

Two independent clean-room emitters — `conformance/emitters/javascript.mjs` and
`conformance/emitters/python.py` — produce byte-identical DSSE-enveloped
Submissions and Verification Records without importing any Vela
implementation, and CI holds
both to the same fixtures. `conformance/readers/python/repository_root.py`
recomputes a repository's root from a clean clone with no network and no Vela
code, while `conformance/readers/javascript/canonical.mjs` independently checks
the RFC 8785 canonical-byte and SHA-256 vector corpus.

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
git -C math checkout 130fc283b99b8c55dea51b5f8f959a6c33a679f6
vela replay math --json          # ok: true, repository_root: sha256:db4d435c…
vela status math --json          # integrity.replay: verified, strict: pass
vela claims math --json          # indexed.accepted: 1
vela review list math --status all --json
```

And without trusting our tool at all — the repository root recomputed from a
clean clone by ninety lines of Python whose entire dependency set is `rfc8785`
and `hashlib`:

```bash
python conformance/readers/python/repository_root.py math \
  --expect sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc
```

That root is the Repository at commit
`130fc283b99b8c55dea51b5f8f959a6c33a679f6`. Every Decision after it moves
the root, correctly — a mismatch means the repository has advanced, not that
something is wrong. Clone at that commit, or take the current value from `vela
replay` at the commit you cloned.

That checks one thing — that the manifest bytes hash to the root they are named
by — and says so on its own output. It is the anchor the rest hangs from, and
it is the number a reader would otherwise have to take from the tool being
checked.

The mirror at `codeberg.org/vela-science/math` is a full replica on
independent infrastructure, verified ref-for-ref on every run.
