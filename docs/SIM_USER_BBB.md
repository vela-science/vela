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

---

# Pass #2 (after v0.29 shipped) — 2026-04-26

The point of pass #2 was to actually complete the loop the
v0.29 local-mode loader was supposed to enable: open the
Workbench in a real browser against a local `vela serve`,
accept/reject through the actual Inbox UI, sign with the CLI,
bench. Trimmed workspace (1 paper, 2 notes, 1 .py, 1 .csv) so
the click-through could finish in a single sitting.

## What worked

- All 4 ingestion agents ran clean. 27 pending proposals, mix
  of types: 2 from scout, 15 from notes (with the new
  `--max-items-per-category 3` cap holding), 5 from code, 5
  from data.
- The v0.28.1 reviewer streaming progress *was* the right
  call. Notes Compiler is fast enough to not need it; the
  reviewer/experiment-planner pace is what made progress
  visible.
- The v0.29 local-mode loader worked first try against
  `http://localhost:4321/frontiers/view?api=http://localhost:3860`.
  The page loaded the frontier from the local API, synthesized
  the HubEntry, rendered all 27 cards in the Inbox. No hub
  touched.
- `vela queue sign` produced 21 signed accepts → events + 6
  signed rejects. The substrate held; events appended cleanly;
  the diff tab showed everything.

## New friction surfaced

### #9 (P1, fixable) — Deployed Astro site can't reach localhost over HTTPS→HTTP

Browsers block mixed-content. Opening
`https://vela-site.fly.dev/frontiers/view?api=http://localhost:3860`
hangs the fetch silently — no error, just a stalled page with
"…" placeholders. The `?api=` parameter is parsed correctly
(title becomes "local frontier · Vela") but the request never
completes.

**Fix:** detect mixed-content scheme mismatch in the loader and
show an explicit message: *"This deployed site is HTTPS but the
local API is HTTP. Run the Astro site locally
(`cd site && bun run dev` → http://localhost:4321) or expose
your serve via a TLS tunnel."* Half-day to add a clear error +
a doc page explaining the workaround.

### #10 (P0, fixable in 1 line) — `flyctl deploy` ships stale `dist/` if you forget `bun run build`

The Dockerfile is `FROM nginx:alpine; COPY dist /…`. There's no
build step in the container. So `flyctl deploy` after a code
change with no `bun run build` ships the *previous* compiled
JS. Hit this immediately in pass #2: my v0.29 deploy "succeeded"
but served the v0.28 Astro behavior.

**Fix:** wrap `flyctl deploy` in a `Makefile`/`bun run` script
that invokes `bun run build` first, then `flyctl deploy`. One
file, no excuses.

### #11 — false alarm (closed)

Initial pass-#2 diagnosis: clicking accept/reject re-renders
the Inbox and breaks subsequent clicks. **Wrong.** Re-reading
the loader code: `renderInbox()` is only called once on initial
load; the click handler patches the card in place (disables
buttons, swaps the pending pill for a "staged · accept" pill).
What actually killed my batch was the **45-second CDP eval
timeout** — the Chrome MCP connection dropped while my JS loop
was still running, so I lost visibility into the remaining
clicks (and possibly Chrome killed the eval context). Honest
mistake from misreading the queue.json count under time
pressure.

No code change required. Lesson: when scripting batch UI
operations through CDP, run them as a fire-and-forget Promise
and immediately return a marker ID, then poll the resulting
state from a fresh eval rather than awaiting in-place.

### #12 (P2, doc bug) — `vela queue sign --all` is documented but doesn't exist

The flag is `--yes-to-all`. The first sim-user pass's friction
report referenced `--all`; my pass-#2 muscle memory hit the
same wall. Trivial: either accept `--all` as an alias, or grep
the docs for `--all` and replace.

### #13 (P2, bench-design issue) — Composite is the same 0.312 whether you sign 4 or 27 findings

The bench's claim_match_rate uses jaccard ≥ 0.4 on raw text. A
candidate frontier built from 1 paper + a few notes will *never*
share enough 4-grams with a curated 48-finding gold to score >0
on matches, no matter how good its proposals. So composite
collapses to 0.25 + a sliver from contradiction_recall (defaults
to 1.0 when there are 0 gold contradictions to recall) and
1-duplicate_rate. 0.312 is a floor, not a measure.

**Fix:** lower the jaccard threshold (0.2?), or add an embedding-
similarity backstop, or give the composite a "no overlap →
explicit zero" mode so 0.312 doesn't masquerade as a passing
grade. This is real bench-design work — defer to a follow-up
v0.30 *if* we want bench to become a real CI gate.

## What I'm fixing tonight

The two with the highest leverage and lowest risk:

1. Friction #10 — `bun run` script that does
   `bun run build && flyctl deploy`. Stops me from ever
   shipping a stale dist/ again. 5 minutes.
2. Friction #12 — make `--all` an alias for `--yes-to-all` on
   `vela queue sign`. 10 minutes, no behavior change.

## v0.29.2 follow-up — finishing the rest

After v0.29.1 shipped, picked up the deferred items:

- **Friction #9** — mixed-content guard now fires before any
  fetch attempt. The page detects `https://` document +
  `http://` API and shows a clear "Mixed-content blocked"
  banner with two specific workarounds (run Astro locally;
  expose serve via cloudflared/ngrok/tailscale-funnel). Verified
  in browser against the deployed site.
- **Friction #11** — closed as false alarm; see updated entry
  above.
- **Friction #13** — bench learned the difference between a
  metric that scored 1.0 and one that's vacuous. New
  `MetricResult.vacuous` field marks "no contradictions in
  gold" / "no novel candidates" cases; `compute_composite`
  excludes them from both numerator and denominator. Pretty
  output tags vacuous metrics as `n/a`. A "no overlap detected"
  banner now appears when `matched_pairs == 0` against
  non-empty inputs. BBB-vs-BBB still scores 1.000; the pass-#2
  candidate, which used to score a misleading 0.312, now scores
  honestly closer to 0 because the vacuous 1.0s are no longer
  inflating it.

## Updated ledger

Pass #2 closed the loop. v0.29 delivered the local Workbench
flow it promised — *when run against the local Astro dev
server*. The deployed site is blocked by browser mixed-content
policy, which is honest browser behavior, not a Vela bug. Pass
#2 surfaced 5 new frictions (1 P0, 2 P1, 2 P2); the two cheapest
ship in v0.29.x.

The substrate doctrine still holds: 21 accepts + 6 rejects
signed cleanly, every event content-addressed, no panics, BBB
strict-check unchanged.
