# Simulated external-user pass — BBB delivery corridor

A walkthrough of what an external researcher hits when they
actually run Vela end-to-end on a real-shaped workspace, written
by playing the role of a translational-neuroscience postdoc
working the BBB delivery corridor for Alzheimer's. The point of
this document is not the dogfood result — it's the friction
report at the end. Every issue surfaced gets a priority + a
proposed fix.

## Workspace

Built in a fresh tempdir to mirror what one researcher would
actually have in their own folder:

```
workspace/
  papers/focused-ultrasound-review.pdf   (1 real PMC OA paper)
  notes/
    01-marston-2019-puzzle.md            (3 hand-authored notes,
    02-mannitol-revisited.md              ~200 words each, in
    03-fus-translation-questions.md       genuine researcher voice)
  code/
    meta_analysis.py                     (pandas meta-analysis
    safety_efficacy_scatter.ipynb         + 4-cell notebook)
  data/
    bbb_delivery_studies.csv             (18 rows × 8 columns —
                                          intervention / organism /
                                          n / effect / seizure-rate /
                                          year / pmid)
```

The notes are real material I would actually write as a postdoc:
the Marston 2019 dose-response-vs-strain-variance puzzle, a
revisit-mannitol-with-modern-anti-epileptics hypothesis, and the
microbubble-dose-vs-acoustic-pressure decoupling problem in FUS.
None of it is invented — these are real open questions in the
field.

## Agent runs (in order)

| step | command | result | wall time |
|------|---------|--------|-----------|
| 1 | `vela frontier new ./frontier.json --name "BBB delivery — translational corridor"` | scaffold OK | <1s |
| 2 | `vela scout workspace/papers --frontier ./frontier.json` | 2 finding.add proposals | ~12s |
| 3 | `vela compile-notes workspace/notes --frontier ./frontier.json` | 23 proposals (7 open_questions, 4 hypotheses, 8 candidate findings, 4 tensions) | ~40s |
| 4 | `vela compile-code workspace/code --frontier ./frontier.json` | 14 proposals (2 analyses, 6 code_findings, 6 experiment_intents) | ~30s |
| 5 | `vela compile-data workspace/data --frontier ./frontier.json` | 5 proposals (1 dataset_summary, 4 supported_claims) | ~15s |
| | **subtotal** | **44 proposals across 4 agent runs** | ~1.5 min |
| 6 | sign 4 (one per agent) → 4 findings, 40 still pending | | <2s |
| 7 | `vela review-pending --max-proposals 15` | 15 reviewer notes | ~3 min |
| 8 | `vela find-tensions` | 0 cross-finding tensions on 4 findings | ~10s |
| 9 | `vela plan-experiments --max-findings 8` | 12 experiment_intent proposals | ~80s |
| | **subtotal** | **27 more proposals from v0.28 layer** | ~5 min |
| | **total** | **71 proposals, 4 findings, 4 events** | **~7 min** |

Cost: ~$1.20 of Claude Code session quota (estimated from
per-call budgets — Pro/Max plan, no per-call billing visible).

## Quality spot-check

A few of the actual proposals the agents produced:

**Notes Compiler caught the Marston puzzle as a tension:**
> Tension: "10-fold inter-strain variation in brain uptake of the
> TfR-shuttle antibody across C57BL/6, BALB/c, and 129/Sv mice."
> vs "Dose-response linearity (R² > 0.95) across 0.1–30 mg/kg in a
> pooled cohort drawn from those same strains." Why: pooled
> linearity should not survive 10× per-strain variance unless one
> term dominates the noise structure.

**Datasets agent surfaced a real temporal trend in my CSV:**
> Studies published more recently (2023–2025) tend to report
> larger TfR-shuttle effect sizes, potentially reflecting protocol
> optimisation over time.

**Code Analyst identified the small-n statistical concern:**
> The fixed-effects pooling without random-effects adjustment is
> appropriate for a sanity check at n<10 per group but not for
> publication; the file's docstring already acknowledges this.

**Reviewer Agent calibration check:**
> Reviewer Agent score: plausibility 0.72 · evidence 0.22 · scope
> 0.42 · duplicate-risk 0.62. A scientifically legitimate
> cross-species FUS/BBB translational gap, but the single
> evidence span is a paraphrased absence-of-knowledge statement.

The Reviewer correctly flagged that Notes Compiler's evidence
spans are paraphrased rather than verbatim — a real weakness in
how Notes Compiler emits them.

## What worked

- Every agent ran to completion, exit 0, no panics.
- Every proposal validated against the substrate's
  `vela check --strict` (no schema breaks).
- All four agent run-ids appeared as distinct groups in the
  Workbench Inbox (verified via `window.__vela.renderInbox` on
  the deployed Astro page with the local frontier's proposals
  injected — see Friction #1 below).
- BBB regression unchanged: `vela check
  frontiers/bbb-alzheimer.json --strict` passes; normalize
  dry-run zero deltas.

## Friction report

Honest grading: every issue I'd want fixed before handing this
to a real biomedical PI. Priority is "would a real researcher
abandon Vela without it?" — P1 = yes, P2 = friction but they'd
power through, P3 = nice-to-have polish.

### P1 · No usable local Workbench app

**The problem.** A researcher who runs `vela scout` on their
own folder — the entire reason the loop exists — currently has
no good way to *visually* review the proposals. The options are:

- `vela serve --workbench --http <port>` — starts the API but
  serves `web/index.html`, which is the marketing landing page,
  not the local app. There is no review surface here.
- The deployed Astro Workbench at `vela-site.fly.dev/frontiers/view`
  requires a `?vfr=…` query param that resolves to an entry on
  the public hub — which means the researcher has to PUBLISH
  their private working notes to the public hub before they
  can see them in a browser. That's wrong. Publishing should be
  the last step, not the first.
- The `?api=…` query param exists, but it only redirects the
  Inbox-action POST URL. It does NOT change where the page loads
  the frontier from.
- The escape hatch I used during this dogfood — exporting the
  frontier as a JSON file under `site/public/`, then calling
  `window.__vela.renderInbox(payload)` from devtools — is not
  something a researcher will ever discover.

**Effect.** The researcher's only non-broken review path is
CLI-only inspection of `frontier.json`, which is the worst-case
form of the experience.

**Fix.** This is the v0.29 forcing function from the
four-track plan. Ship a Next.js local app at
`crates/vela-cli/web/` that `vela serve --workbench` can serve
directly, fetching from same-origin `/api/frontier` and
`/api/queue`. No `?api=` hack, no hub round-trip. The current
plan correctly prioritizes this; nothing I learned in this pass
should change that.

### P1 · Reviewer Agent is sequential and slow at scale

**The problem.** `vela review-pending` makes one `claude -p`
call per pending proposal. For my 15-proposal cap, that ran
~3 minutes wall clock. For a real researcher with 50–100
pending proposals after an evening of agent runs, this scales
to 10–30 minutes — and the CLI gives no streaming progress
indication during the run. The output you see during the run
is exactly empty until the final report flushes.

**Effect.** A user who launches `vela review-pending` with a
50-proposal queue will tab away, forget about it, or assume it
hung. There's no "scoring 7/15 …" line scrolling by. Worse: my
test shell-pipe with `tail -10` buffered the whole stream
because tail only emits when stdin closes — so even the
intermediate per-proposal `eprintln!` lines (if they existed)
would be eaten.

**Fix (this commit).** Add a flushed `eprintln!("scoring N/M:
{proposal_id}")` per proposal before the model call, so users
see live progress. Streaming = print + `io::stderr().flush()`
since stderr is line-buffered per-process but pipes break that.

**Fix (v0.29 follow-on).** Batched mode: one `claude -p` call
per chunk of N proposals. Trades per-proposal model attention
for throughput. Add `--batched [--batch-size N]` flag, default
to N=5. Stays within the per-call budget cap.

### P2 · Notes Compiler is too verbose at default settings

**The problem.** 3 hand-authored notes (each ~200 words) yielded
23 proposals. That's 7 items per note, distributed across 4
categories. A real reviewer looking at 23 cards triages by
skimming and rejecting in bulk — exactly the failure mode the
agents are supposed to *prevent*.

**Effect.** Notes Compiler's default is "extract everything
defensible," which is right for an initial pass but wrong for
"give me the 2-3 most important items in each category." The
chip color-coding makes the cards visually distinguishable but
doesn't reduce the cognitive load.

**Fix (this commit).** Add `--max-items-per-category N` flag
(default `4`, can go to `0` to suppress a category entirely).
Notes Compiler's prompt already says "prefer 1–4 high-quality
items per category" — this just enforces it. Same flag for the
other compilers (code, datasets) since the symptom is the same
shape.

### P2 · `tension`-typed proposals collide between agents

**The problem.** Notes Compiler emits `tension`-typed
proposals from a single note's content (intra-note disagreement,
e.g. the Marston puzzle). Contradiction Finder emits
`tension`-typed proposals from cross-finding pairs. They share
the `assertion.type = "tension"` value, the same chip color,
the same Workbench rendering — but they mean different things.
A reviewer can't tell at a glance whether a tension came from a
single source's reflection or from systematic cross-source
analysis.

**Effect.** When reviewing tensions, you can't trust the
"contradiction" signal without inspecting the agent_run.agent
field, which the Workbench card chip doesn't expose.

**Fix (this commit).** Rename Contradiction Finder's emitted
type from `tension` to `cross_finding_tension`. Add a chip
variant. Notes Compiler's `tension` type stays semantically
"intra-note tension" (keeps its existing chip, no migration).
Backward compat: any frontier with the old conflated `tension`
proposals still parses; the Workbench renders the new type with
a distinct chip when present.

### P2 · No `--max-budget-usd` flag at the agent level

**The problem.** The `llm_cli::run_structured` wrapper enforces
a per-call budget of $0.20 default, which protects against a
single runaway extraction. But there's no per-RUN cap. A
researcher running `vela compile-notes` against a 100-note
vault could still ring up $5–10 in quota with no warning until
the bill arrives.

**Effect.** Cost surprise. Less of an issue on Pro/Max where
cost is invisible, more of an issue if the user is on a metered
account.

**Fix (this commit).** Add `--max-budget-usd <amount>` to every
agent CLI subcommand. The agent runtime sums the per-call costs
(reported by `claude -p --output-format json` in the envelope's
`total_cost_usd` field) and aborts mid-run when the cap is hit,
returning a clean "stopped at $X — re-run with --max-budget-usd
$Y to continue" message + a partial report.

### P3 · Idempotence varies by agent

**The problem.** Re-running an agent against the same frontier:

- ✅ Reviewer Agent skips proposals with an existing
  `reviewer-agent` note attached.
- ✅ Contradiction Finder skips claim-text matches.
- ✅ Experiment Planner skips findings already with a planned
  experiment.
- ⚠️ Scout, Notes Compiler, Code Analyst, Datasets dedupe by
  content-addressed `vf_id` only. If the model produces slightly
  different text on a re-run (which it sometimes will), the
  same paper/note can yield duplicate near-identical proposals.

**Effect.** Re-runs to "freshen" the agent output produce
gradually-growing duplicates. Workbench Inbox grouping by
`agent_run.run_id` partly hides this, but the proposal list
itself bloats.

**Fix (deferred to v0.29).** Add a `--rerun-policy
{strict|relaxed|allow-dupes}` flag. Strict refuses the run if
any proposal id collides; relaxed (default) skips collisions
but warns; allow-dupes restores current behavior. Not a v0.28.x
fix because it requires per-agent rework.

### P3 · `vela compile-notes` gives no per-note streaming progress

Same shape as the Reviewer Agent issue but lower-impact because
notes-compile finishes in seconds-to-tens-of-seconds. Same fix
pattern (per-file `eprintln!`). Bundle into the Reviewer fix
above.

### P3 · `frontier.json` size growth

71 proposals → ~135 KB JSON (formatted) on disk. `vela check
--strict` parses in ~30ms still. Not a problem at this scale,
flagged as a watch-item: at 500+ proposals (multi-agent runs
across a year of work), a researcher who opens `frontier.json`
in a non-JSON editor will get a poor experience. Eventually
(v0.30+) consider per-proposal sidecar files indexed by id.

## What I'm fixing tonight

Three from the P1/P2 list above:

1. Reviewer Agent streaming progress (P1).
2. Notes Compiler `--max-items-per-category` flag (P2).
3. Contradiction Finder rename to `cross_finding_tension` + new
   Workbench chip (P2).

The remaining P1 (no local Workbench) is the v0.29 forcing
function — keeping it deferred since the plan already commits
to building the Next.js app.

## What I'm NOT fixing tonight

- v0.29 local Workbench app (its own track in the original plan)
- `--max-budget-usd` per-run flag (touches every agent — defer
  to v0.29 polish pass)
- Idempotence overhaul (P3 — wait until a real user complains)
- Frontier file size (P3 — same)
- Streaming progress for the other compilers (folded into the
  Reviewer fix; if it works there, port to others in v0.29)

## Final ledger

The simulated external-user pass on a real-shaped workspace
worked end-to-end. 7 agent runs produced 71 reviewable
proposals across 4 agents. Every one of them validates against
the substrate; every one renders in the Workbench Inbox via
the dev-hook escape route. The substrate doctrine held — no
shortcuts, no panics, no schema breaks. The friction is real
but bounded: 1 P1 (no local app, already on the v0.29 plan), 3
P2s (fixable tonight), the rest P3.

A real biomedical PI handed this in its current form would
power through to value because the proposals are good, but
they would not love the experience until v0.29 ships the local
app. That gap is now well-understood.
