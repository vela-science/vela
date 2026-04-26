# Agent Inbox (v0.22)

The first end-to-end loop where an AI agent's output becomes
reviewable scientific state.

```
folder of PDFs
  → Literature Scout proposes FindingIntents
  → local Workbench Inbox shows proposals with evidence
  → user accepts / rejects
  → vela queue sign --all  (CLI signs)
  → frontier diff shows what changed
```

## What's in v0.22

- **`vela scout <folder> --frontier <path>`** — runs Literature Scout
  on every `*.pdf` at the top of `<folder>`, extracts candidate
  findings using your local `claude` CLI's OAuth session (no API key
  needed on a Pro/Max subscription), and writes one `finding.add`
  `StateProposal` per candidate to `<frontier>.proposals[]`. Each
  proposal carries an `agent_run` block recording the agent name,
  model id, run id, and start time.

- **Workbench Inbox tab** at `/frontiers/view?vfr=…` — groups
  proposals by `agent_run.run_id`, renders each with claim (serif),
  rationale (italic), source filename + flags as colored chips, and
  REJECT / ACCEPT buttons. Pre-existing visual layer, no new chrome.

- **Workbench Diff tab** — newest-first list of every signed
  `StateEvent` in the frontier, color-coded by kind (`finding.add`
  moss · `finding.review` signal · `finding.revise/caveat` brass ·
  `finding.retract` madder · `finding.note` stale).

- **Local-first signing.** The browser only stages decisions into
  `~/.vela/queue.json`. The Ed25519 key never enters the browser.
  `vela queue sign --all` is the only path that produces signed
  canonical state.

## Doctrine

The substrate stays dumb. `vela-protocol` does not know whether a
proposal came from a human, a Claude run, a GPT run, a lab pipeline,
or a future agent we haven't named. The agent layer lives in
`vela-scientist`, depends on the substrate one-way, and writes
`StateProposal`s through the existing protocol. Removing
`vela-scientist` from the workspace would leave every accepted
finding intact.

The CLI binary lives in `vela-cli`, which is the only crate that
imports both. The substrate library has zero agent dependencies.

## End-to-end walkthrough

Setup once:

```bash
# Build all four crates.
cargo build --release --workspace

# Make sure the `claude` CLI is on PATH and signed in.
claude --version  # 2.x
```

Then for any folder of papers:

```bash
# 1. Scaffold a fresh frontier.
./target/release/vela frontier new ./my-frontier.json \
  --name "My bounded question"

# 2. Run Literature Scout. One model call per PDF.
#    Default model is whatever your Claude session prefers.
#    Override with --backend sonnet / opus.
./target/release/vela scout ./papers --frontier ./my-frontier.json

# 3. Serve the frontier locally so the Workbench can read it.
./target/release/vela serve ./my-frontier.json --workbench --http 3848
```

Open the Workbench in a browser:

```
https://vela-site.fly.dev/frontiers/view?vfr=<your-vfr>&api=http://localhost:3848
```

Click the **Inbox** tab. For each proposal:

- Read the claim, rationale, and evidence chip.
- Click **REJECT** to drop it from consideration, or **ACCEPT** to
  stage it.
- A banner appears at the top: *"N actions staged in your local queue
  (X accept, Y reject). Sign and apply with the CLI:* `vela queue
  sign --all`*"* with a one-click Copy.

Sign in your terminal:

```bash
./target/release/vela queue sign --all \
  --actor reviewer:you \
  --key ~/.vela/keys/your-private.key \
  --yes-to-all
```

Each accepted proposal becomes a signed `StateEvent` in
`my-frontier.json`'s `events[]`, and (for `finding.add` kinds) the
underlying finding lands in `findings[]`. Refresh the Workbench;
the **Diff** tab now shows the events with color-coded kind chips.

## Dogfood result

One PDF (focused-ultrasound BBB review, ~1.9 KB) → 2 candidate
findings extracted by Claude, both with verbatim evidence snippets
and short rationales:

1. *"Focused ultrasound transiently opens the blood-brain barrier in
   Alzheimer disease models and may improve delivery of therapeutic
   agents."* — kind: `therapeutic`, evidence span verbatim from
   abstract.
2. *"Safety, reversibility, and dosing schedules of focused
   ultrasound BBB opening are unresolved questions for clinical
   translation in Alzheimer disease."* — kind: `methodological`,
   evidence span verbatim from review section.

End-to-end latency: about 12 seconds (PDF text extraction + one
`claude -p` call + frontier write).

## What's not in v0.22

Held to keep scope tight. Each lands as its own slice when the
loop holds:

- **v0.23: Notes Compiler** for Markdown / Obsidian vaults.
- **v0.24: Code/Notebook Analyst** for Jupyter + scripts +
  AnalysisRun objects.
- **v0.25: Dataset support** for CSV / Parquet inputs and dataset
  versioning.
- **v0.26: VelaBench** for agent state-update scoring.

Other deliberate non-goals for v0.22:

- Browser-side WebCrypto signing. Stays CLI-only.
- Auto-merge of any kind. Humans review.
- Edit-in-Inbox. Reviewers reject and re-propose for now.
- Multi-frontier ingestion. One frontier per scout run.

## Architecture

```
crates/vela-protocol     # substrate library — schemas, validation,
                         # canonical-JSON, signing, registry, hub
                         # client. Zero LLM/agent dependencies.

crates/vela-scientist    # agent layer — Literature Scout (v0.22),
                         # Notes Compiler (v0.23), etc. Depends on
                         # vela-protocol; emits StateProposals.

crates/vela-cli          # the `vela` binary. Depends on both. Wires
                         # vela-scientist's scout into the substrate's
                         # CLI dispatch via register_scout_handler.

crates/vela-hub          # public hub HTTP API. Substrate-only; does
                         # not know about agents.
```

The crate split is the doctrinal claim made enforceable: the
substrate compiles and runs without the agent layer present.
