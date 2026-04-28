# Learn Vela

A working scientist's path from zero to a publishable signed
frontier in under an hour. The book takes one running example
(a small Alzheimer's-adjacent campaign) all the way through:
asserting findings, attaching evidence, recording replications,
making predictions, federating with a peer hub, and publishing.

The architecture doc explains the model. This document is the
walkthrough.

## What you'll build

By the end of this book you will have:

- A local frontier called `projects/microglia-trem2` with a
  handful of signed findings about microglial TREM2 biology
- One replication record per finding-with-evidence
- One dataset (`vd_`) and one code artifact (`vc_`)
  reference attached to a finding
- One prediction (`vpred_`) with a deadline
- A cross-frontier link into the live BBB Flagship
- A federated peer registration against the public hub
- A signed registry entry published to your local hub
- A causal-typing pass that produces a clean
  `vela causal audit`

The whole walkthrough takes about an hour. The substrate is the
same shape no matter the topic; you can replace `microglia-trem2`
with whatever campaign you actually care about and the steps are
identical.

## 0. Setup

### Prerequisites

You need:

- Rust toolchain (stable channel) for building Vela from source
- Git for cloning the repo
- About 200 MB of disk for the workspace
- Optional: the `claude` CLI from Anthropic, only if you want to
  run the agent inbox (Scout, Notes Compiler, etc.)

### Clone and build

```
git clone https://github.com/vela-science/vela.git
cd vela
cargo build --release --bin vela
export PATH="$PWD/target/release:$PATH"
vela --version
```

The release build is what you want for any non-trivial work; debug
builds run roughly 10x slower on the substrate's heavier queries.

### Generate your first keypair

Every Vela actor signs canonical events with an Ed25519 keypair.
Generate yours.

```
vela sign generate-keypair --out ~/.vela/keys
```

This writes `~/.vela/keys/private.key` and `~/.vela/keys/public.key`,
both hex-encoded. Treat the private key like an SSH private key.
The public key is what you publish.

A quick sanity check.

```
cat ~/.vela/keys/public.key
```

Should be 64 hex characters. That's your actor identity from now
on.

## 1. Your first frontier

### Scaffold the directory

```
vela frontier new projects/microglia-trem2 \
    --name "Microglial TREM2 biology" \
    --description "A small worked example: TREM2-dependent microglial responses to amyloid beta."
```

This produces a publishable `frontier.json` stub. Take a look.

```
cat projects/microglia-trem2/frontier.json | head -20
```

Already valid: `vela check projects/microglia-trem2` should report
zero invalid findings (because there are zero findings yet) and a
clean event replay.

### Initialize a Vela repo layout

For real work you want the full `.vela/` directory layout (one file
per content-addressed object). Convert.

```
vela init projects/microglia-trem2
```

Now you have `projects/microglia-trem2/.vela/` with the standard
subdirectories: `findings/`, `events/`, `proposals/`,
`replications/`, `datasets/`, `code-artifacts/`, `predictions/`,
`resolutions/`. Each kernel object lives as one file in its
respective directory.

### Register yourself as an actor

The frontier needs to know your public key before it will accept
signed events from you.

```
vela actor add projects/microglia-trem2 \
    reviewer:your-name \
    --pubkey "$(cat ~/.vela/keys/public.key)"
```

Pick `reviewer:your-name` to be a stable identifier you'll
keep using. ORCID-style identifiers also work
(`reviewer:orcid:0000-0001-2345-6789`).

Verify.

```
vela actor list projects/microglia-trem2
```

You should see one registered actor.

## 2. Your first finding

### The unit

A finding is a single scientific claim with structured evidence
and conditions. The minimum useful shape is an assertion text, an
assertion type, an evidence type, and one condition flag. Add one.

```
vela finding add projects/microglia-trem2 \
    --assertion "TREM2 R47H carriers show reduced microglial clustering around amyloid plaques in late-stage Alzheimer's disease." \
    --type observational \
    --evidence-type observational \
    --source "Wang et al. 2015, Cell" \
    --doi 10.1016/j.cell.2015.01.049 \
    --year 2015 \
    --author reviewer:your-name \
    --conditions-text "Late-stage AD postmortem brain tissue, R47H heterozygotes" \
    --species "Homo sapiens" \
    --human-data \
    --confidence 0.75 \
    --apply
```

The `--apply` flag accepts the proposal immediately. Without it,
the finding lands in `proposals/` for separate review (which is
the workflow when an agent or another reviewer is proposing).

You should see output ending with `applied event vev_<...>`. The
finding now lives at `projects/microglia-trem2/.vela/findings/vf_<id>.json`.

### Inspect it

```
vela status projects/microglia-trem2
```

Reports one finding, one event, one actor. The daily-driver view
of the substrate's pulse.

```
vela log projects/microglia-trem2 --limit 5
```

Shows the recent event log. You should see one `finding.asserted`
event with your reviewer id as the actor.

### A second finding, with a link

Add a related claim that links back to the first.

```
vela finding add projects/microglia-trem2 \
    --assertion "Soluble TREM2 (sTREM2) levels in CSF correlate with reduced cognitive decline rate over 24 months in mild cognitive impairment patients." \
    --type observational \
    --evidence-type observational \
    --source "Suarez-Calvet et al. 2016, EMBO Mol Med" \
    --doi 10.15252/emmm.201506123 \
    --year 2016 \
    --author reviewer:your-name \
    --human-data \
    --conditions-text "MCI patients followed 24 months, CSF sampling" \
    --confidence 0.70 \
    --apply
```

Note both findings get content-addressed `vf_<hash>` IDs derived
from the assertion + evidence + conditions + provenance. Two
people independently asserting the same claim about the same
evidence get the same `vf_` ID. That's how the substrate
deduplicates without coordination.

## 3. Adding evidence

### Replication

Suppose lab Beta tries to reproduce the first finding (R47H +
reduced clustering) in an independent cohort.

First, list your findings to grab the id of the one being
replicated.

```
vela replications projects/microglia-trem2
```

(Empty for now.) Look at one of your findings to copy its id.

```
ls projects/microglia-trem2/.vela/findings/
```

Pick the first `vf_*.json` file. Use that id.

```
vela replicate projects/microglia-trem2 vf_<your-finding-id> \
    --outcome replicated \
    --by lab:beta-translational \
    --conditions "Independent late-stage AD cohort, n=18 R47H carriers" \
    --source-title "Lab Beta replication memo, internal" \
    --sample-size "n=18"
```

A new `vrep_<id>` record lands. The propagation runtime fires
automatically: the finding's confidence recomputes from the live
`replications` collection. Check.

```
vela status projects/microglia-trem2
```

The replications row now shows `1 records · 1 findings replicated`.
The audit page (described later) will treat this finding as
better-supported than its peers.

### Dataset reference

Datasets are first-class kernel objects. Register the cohort the
finding rests on.

```
vela dataset add projects/microglia-trem2 \
    --name "ADNI-3 R47H subcohort" \
    --version "v3.2" \
    --content-hash "sha256:placeholder-replace-with-actual" \
    --url "https://adni.loni.usc.edu/data-samples/" \
    --license "ADNI Data Use Agreement" \
    --source-title "Alzheimer's Disease Neuroimaging Initiative" \
    --doi 10.1212/WNL.0b013e3181cb3e25
```

That gives you a `vd_<id>` you can reference from any finding's
`provenance.dataset_refs`. The kernel doesn't fetch the data; it
just records the content-addressed pointer at it.

### Code artifact

If a claim rests on specific analysis code, record that too.

```
vela code add projects/microglia-trem2 \
    --language R \
    --repo "https://github.com/example/microglia-clustering-analysis" \
    --commit "a1b2c3d4e5f6" \
    --path "scripts/cluster_amyloid_plaques.R" \
    --line-range "42:118" \
    --content-hash "sha256:placeholder"
```

The result is a `vc_<id>` artifact. Same pattern as datasets:
content-addressed pointer, no fetch, the kernel can verify the
hash later if you supply the file.

## 4. Cross-frontier links

### The setup

The live BBB Flagship at `vela-hub.fly.dev` already publishes 188
findings about blood-brain-barrier translation in Alzheimer's. Many
of your microglia-trem2 findings probably depend on those. The
substrate lets you express that dependency by `vfr_id`.

### Declare the dependency

```
vela frontier add-dep projects/microglia-trem2 \
    --vfr-id vfr_a1b985823a975887 \
    --name "BBB Flagship" \
    --locator "https://raw.githubusercontent.com/vela-science/vela-frontiers/main/frontiers/bbb-alzheimer.json"
```

Now you can reference any BBB finding from your own findings using
the `vf_X@vfr_a1b985823a975887` syntax in `link.target`. The
substrate's strict-mode validator refuses cross-frontier targets
without a declared dependency, which forces explicitness.

### Add a finding that depends on a BBB claim

(Suppose BBB has `vf_8389130295d81413` — the ATV:TREM2 microglia
proliferation claim.)

```
vela finding add projects/microglia-trem2 \
    --assertion "TREM2 agonism in late-stage R47H carriers is unlikely to recover full microglial clustering function because of independent pathway losses." \
    --type hypothesis \
    --evidence-type theoretical \
    --source "Synthesis of TREM2 mechanistic literature" \
    --author reviewer:your-name \
    --confidence 0.55 \
    --apply
```

Once the finding lands, add a typed link from it back to the BBB
finding it depends on.

```
vela link add projects/microglia-trem2 \
    --source vf_<your-hypothesis-id> \
    --target vf_8389130295d81413@vfr_a1b985823a975887 \
    --type depends \
    --note "Depends on the ATV:TREM2 proliferation result for the underlying causal mechanism."
```

Now if BBB ever retracts that finding, the substrate's
propagation runtime will flag your hypothesis. Cross-frontier
retraction cascades work by the same mechanism as within-frontier
ones.

## 5. Federation

### Register a peer

The public hub at `vela-hub.fly.dev` is the canonical registry for
the BBB-adjacent corner of the substrate. Register it as a peer.

First fetch the BBB owner pubkey (every published manifest is
signed; the registry entry carries the owner key).

```
curl -sS https://vela-hub.fly.dev/entries | jq '.entries[] | select(.name | contains("BBB"))'
```

Find the `owner_pubkey` field. Register the hub as a peer.

```
vela federation peer-add projects/microglia-trem2 \
    hub:vela-primary \
    --url https://vela-hub.fly.dev \
    --pubkey <owner_pubkey from above> \
    --note "Primary public hub. BBB Flagship and Alzheimer's Therapeutics live here."
```

### Run a sync

```
vela federation sync projects/microglia-trem2 \
    hub:vela-primary \
    --via-hub --vfr-id vfr_a1b985823a975887 \
    --dry-run
```

Dry-run prints what would land. Drop `--dry-run` to actually
record the sync interaction as canonical events
(`frontier.synced_with_peer` plus one `frontier.conflict_detected`
per disagreement).

The sync is read-only with respect to your findings. It records
what differs; it never silently merges peer state into yours. The
v0.39.2+ work to do conflict resolution via signed proposals is
how you eventually accept a peer's view of a specific finding.

## 6. Causal grading

### Why

Vela models causal claims explicitly. A finding's
`causal_claim` is one of `correlation`, `mediation`, or
`intervention`. The `causal_evidence_grade` is one of
`theoretical`, `observational`, `quasi_experimental`, or `rct`.
Identifiability falls out of the matrix: an intervention claim
backed only by observational evidence is structurally
underidentified, and the substrate flags it.

### Set causal types

Walk each finding and record what kind of claim it is.

```
vela finding causal-set projects/microglia-trem2 vf_<your-finding-id> \
    --claim correlation \
    --grade observational \
    --actor reviewer:your-name \
    --reason "Postmortem cohort study; correlation only; no intervention."
```

Each call appends an `assertion.reinterpreted_causal` event. The
chain is auditable: future re-grades supersede prior ones, and the
event log preserves the history.

### Run the audit

```
vela causal audit projects/microglia-trem2
```

Reports per-finding identifiability with a one-line rationale.
Underidentified findings (intervention claims with only
observational evidence) surface first as concrete review items.

In the BBB Flagship, this audit reports 60 identified, 22
conditional (need reviewer-attested assumptions), and 4
underidentified out of 86 originally-graded findings. Those 4 are
the substrate's clearest "the claim outruns the evidence" signal.

## 7. Predictions and calibration

### Make a prediction

A prediction is a falsifiable claim about future state, scoped to
specific findings, with a deadline.

```
vela predict projects/microglia-trem2 \
    --about vf_<your-hypothesis-id> \
    --by reviewer:your-name \
    --claim "By end of 2027, at least one TREM2 agonist Phase 2 trial will report failure to recover cognitive function in R47H carriers compared to non-carriers." \
    --resolves-by 2027-12-31 \
    --confidence 0.70 \
    --resolution-criterion "ClinicalTrials.gov result posting reporting non-significant treatment effect in the R47H subgroup at p<0.05"
```

A `vpred_<id>` lands. It will sit pending until either someone
records a resolution or the deadline passes.

### Resolve a prediction

When the prediction's outcome is known, record it.

```
vela resolve <vpred_id> \
    --outcome "Phase 2 readout (Acumen, March 2027) showed null effect in R47H subgroup, p=0.31" \
    --matched-expected true \
    --by reviewer:your-name
```

Or, if the deadline passes without resolution, expire it.

```
vela predictions-expire projects/microglia-trem2
```

This walks every prediction, marks any past-deadline ones as
expired, and emits a `prediction.expired_unresolved` event. Expired
predictions count toward the predictor's calibration record but do
not move their Brier score (since the actor failed to commit either
way).

### Read your calibration

```
vela calibration projects/microglia-trem2 --actor reviewer:your-name
```

Reports your Brier score, log score, and hit rate over your
resolved predictions. With one prediction it is a degenerate
record; the substrate accumulates over time.

## 8. Publishing

### Sign your frontier

```
vela sign apply projects/microglia-trem2 \
    --private-key ~/.vela/keys/private.key
```

Walks every finding and produces an Ed25519 signature over the
canonical bytes. Signatures land in `Project.signatures`. Verify.

```
vela sign verify projects/microglia-trem2
```

Reports total / signed / unsigned / valid counts. With one actor
and a clean run, valid should equal total.

### Publish a registry entry to a hub

If you have a hub running locally (or access to a public one), you
can publish a signed registry entry pointing at your frontier's
network locator.

```
vela registry publish projects/microglia-trem2 \
    --owner reviewer:your-name \
    --key ~/.vela/keys/private.key \
    --locator "https://example.org/microglia-trem2/manifest.json" \
    --to https://my-hub.example/entries
```

The hub stores the signed entry. Anyone can fetch it, verify the
signature against your public key, follow the locator to fetch the
manifest, and check the manifest hash against the entry's
`latest_snapshot_hash`. End-to-end content-addressed
verification, no trust in any single hub.

## 9. Recipes

A short list of common workflows you'll reach for.

### Show me what's pending

```
vela status projects/microglia-trem2
vela inbox projects/microglia-trem2 --limit 20
```

The inbox sorts by reviewer-agent composite score (when present),
problems first.

### Ask the substrate

```
vela ask projects/microglia-trem2
```

Drops into a REPL. Try `what's pending`, `what's underidentified`,
`how many findings`, `what changed recently`, `peers`,
`calibration`. Codex-flavored, deterministic answers from
canonical state.

### Recompute confidence after editing

If you adjust an evidence field by hand and want the confidence
score to refresh:

```
vela normalize projects/microglia-trem2 --write
```

Runs the canonical recompute against every finding, writing back
new confidence scores derived from the v0.40.1 formula
(replication-aware, causal-grade-aware).

### Find contradictions

```
vela tensions projects/microglia-trem2
```

Walks the link graph for `contradicts` edges and reports the
unresolved ones.

### Diff two frontiers

```
vela diff projects/microglia-trem2 projects/microglia-trem2-fork
```

Reports adds, removes, and confidence-divergent findings between
the two. Useful for reviewing a colleague's branch before merging.

### Replay verification

```
vela check projects/microglia-trem2
```

Validates every finding, replays the event log, verifies that the
event chain is unbroken and the materialized state matches the
final replayed hash. The substrate's correctness check.

### Run the agent inbox (optional)

If you have the `claude` CLI installed:

```
vela compile-notes projects/microglia-trem2 \
    /path/to/your/markdown/notes \
    --max-files 10
```

Runs the Notes Compiler agent. Each candidate finding lands as a
pending proposal in `proposals/` for review. Accept or reject via
`vela proposals accept` / `reject`, signed under your reviewer id.

## 10. Where to next

You now have:

- A working signed frontier with structured evidence
- A federated peer registered against a real hub
- A signed registry entry the network can verify
- A causal-typing pass that surfaces the questionable claims
- A prediction with a deadline that will resolve into your
  calibration record
- An auditable trail of every change

For continued depth:

- Read `docs/ARCHITECTURE.md` for the strategic frame
- Read `docs/PROTOCOL.md` for the v0 language kernel reference
- Read `docs/CORE_DOCTRINE.md` for the rules the substrate refuses
  to break
- Read `docs/MATH.md` for the confidence formula and aggregation
  semantics
- Look at `projects/bbb-flagship/` in the workspace for a real
  188-finding campaign you can pattern-match against

The substrate is the same shape no matter the topic. The patterns
you learned in this hour generalize directly to whatever campaign
you actually care about.
