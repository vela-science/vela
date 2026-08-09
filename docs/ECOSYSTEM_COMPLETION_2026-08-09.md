# Vela ecosystem completion ledger, 2026-08-09

Status: **the current Vela 0.972.1 implementation cut is complete; the external
validation program is not complete**.

This ledger reconciles three planning inputs against exercised state:

- *Vela Ideal Ecosystem and Architecture*, 2026-08-08;
- *Ink & Switch, Universal Version Control, and Vela*; and
- *Genesis Open Models and the Scientific-State Control Point*, 2026-08-07.

The newest architecture memo governs when the inputs differ. A memo proposal is
not a protocol requirement merely because it was written down. Promotion still
requires a real consumer, exact evidence, and net deletion.

## Completed implementation

| Requirement | Exercised result |
| --- | --- |
| One final standards cut | Vela 0.972.1 uses UUIDv4 Repository identity, genesis-only origin, full canonical roots, one DSSE implementation, generated JSON Schema, and one closed authorization evaluator. Retired writers and compatibility paths are absent. |
| Independent wire reading | Rust, Python, and JavaScript reproduce the current portable objects and canonical roots. The hosted conformance union passes. |
| Retained authorization parity | The closed evaluator reproduces all seven retained Cedar Allows and denies seven boundary mutations for their exact reasons. The test does not claim root parity across the vocabulary migration. |
| Complete authority loop | `vela-science/math` completed genesis, three Submissions, seven Verification Records, two human rejections, one human acceptance, strict replay, and an empty review inbox. |
| Corrected projection ownership | `vela-web` keys canonical projection data by Repository UUID, keeps `math` only as a route slug, and uses `@vela/observatory-data`. The reader has no scientific write authority. |
| Science-translation experiment | The Erdős 321 Dossier carries exact Reference annotations, deterministic semantic facts, explicit loss, provenance, RO-Crate material, and five nonclaims. None is a new Vela protocol object. |
| Provider-loss continuation | Signed Vela assets and Math state were restored from Codeberg, replayed without GitHub, rebuilt twice into fresh PostgreSQL clusters, compared by roots, and exported as a usable Dossier without Neon or Vercel. See [the qualification record](PROVIDER_LOSS_QUALIFICATION_2026-08-09.md). |
| Live read surface | The production manifest binds Vela 0.972.1, Math commit `130fc283b99b8c55dea51b5f8f959a6c33a679f6`, Repository root `sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc`, and release root `sha256:eba70ba9604105bbc4fbdd229d806877d467a86e25bea4c071ee78066bc5ca78`. |

The current implementation therefore satisfies the architecture memo's Phases
0 through 3, delivers the concrete translation artifacts in Phase 4, and closes
the provider-loss part of Phase 5. Phase 4's comparative cold-reader measurement
remains open. The work also closes the Ink & Switch memo's immediate continuity
defect and delivers its first Reference Map, Semantic Diff, Derivation Trace,
and Dossier experiment.

## Deliberately unpromoted experiments

The translation structures remain local to the Observatory. They have one
maintained consumer. No shared package, registry, universal pointer service,
graph authority, or new protocol object was created.

The Ink & Switch promotion gate requires a materially different second
maintained consumer, independent root agreement, measured continuation benefit,
and more duplication deleted than abstraction added. Those facts do not exist
yet. The correct current result is to retain the experiment locally, not to
promote it.

## External gates still open

| Gate | Why it remains open | Completion evidence required |
| --- | --- | --- |
| External workbench producer | The current Submissions were prepared inside the Vela-owned workflow. | A conformant Submission emitted by OpenScience or another workbench Vela does not control. |
| Independent scientific reader or consumer | Clean-room conformance readers exist, but no separately maintained scientific consumer has adopted the Dossier or translation profile. | A maintained external reader that agrees on the applicable roots and uses the output. |
| Genuinely independent authority | A second repository operated by the same person would be fake federation. | Separate governance, independent key custody and trust root, a real mandate, and capacity to disagree. |
| Cross-Repository transfer | Only one live authority exists. | One provenance-preserving candidate evaluated through a local Decision in a separately governed Repository. |
| Correction cascade | The current accepted Claim has no producer-declared Claim dependencies, so the exact cascade is empty. | A real accepted dependency, a correction or withdrawal, the derived affected and unaffected state, and human comparison with the correct repair. |
| Cold successor measurement | Browser and command-line checks were performed by the implementing operator. | A person or agent without private context identifies Standing, evidence, limits, correction state, and next work from public materials, with time and errors recorded. |
| Genesis application | The repository contains a current application brief but has not filed an institutional application. | User-approved submission to Genesis by August 24, 2026, plus any requested rights and reviewer commitments. |
| Three-to-five Genesis cases | One bounded case is complete; rejected variants are Decisions within that case, not independent environments. | Two to four additional complete environments, including a consequential correction case and external-workbench output. |
| Reviewer and continuation benefit | The run demonstrated correctness, not comparative human benefit. | Predeclared baseline and measured review time, missed caveats, continuation time, and correction accuracy. |

These are not software defects that this repository can close unilaterally.
They require external operators, workbenches, reviewers, or institutional
authorization. The ecosystem must not manufacture them to make a checklist
green.

## Current operating boundary

Until the external gates close:

- keep Vela core at `init -> submit -> verify -> decide -> replay`;
- keep Decision local, attributed, human-authorized, and fail closed;
- keep the Observatory reconstructable and read-only;
- keep translation profiles source-local;
- do not create a synthetic second authority or a Genesis-specific fork; and
- do not claim federation, external adoption, reviewer leverage, or scientific
  productivity.
