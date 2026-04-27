# Phase A — Content Expansion (v0.30)

The site's first-impression rewrite (v0.31) shipped against the
existing 48 BBB findings. The frontier reads cleanly today, but its
density skews to BBB delivery (45/48 findings). This runbook expands
it to ~80–100 findings spanning Aβ, BACE1, tau, TREM2, ApoE, and
clinical readouts so every drug-target page on the site has real
content.

This is Will-must-do work. It needs:

- A working `ANTHROPIC_API_KEY` (or an alternative backend the
  Vela agents accept) **directly addressable by the `vela`
  binary**, not via a Claude Code proxy.
- Will's signing keypair under `~/.vela/keys/` (or wherever
  `vela actor` is configured).
- A folder of PDFs for the priority sources listed below.

The plan that grounds this work: `~/.claude/plans/noble-floating-willow.md`.

---

## 1. Verify environment

```bash
./target/release/vela --version              # expect 0.29.4 (rebuild if older)
echo "${ANTHROPIC_API_KEY:0:10}…"             # should print a real prefix
ls ~/.vela/keys/private.key                   # signing key present
./target/release/vela frontier list-deps projects/bbb-flagship/.vela
```

If the binary is older than the workspace, rebuild:

```bash
cargo build --release -p vela-cli
```

## 2. Compile the Obsidian campaign into proposals

The campaign lives at:

```
~/Documents/Obsidian Vault/Research/Campaigns/Alzheimer's Drug Target Landscape.md
```

with linked notes under `Research/Notes/` and sources under
`Research/Sources/`. The campaign covers Aβ, BACE1, tau, TREM2,
ApoE, lecanemab, and BACE1-inhibitor failures — the exact targets
the site's drug-target pages are waiting for.

Run Notes Compiler against the vault, dry-run first to confirm the
extraction shape:

```bash
./target/release/vela compile-notes \
  ~/Documents/Obsidian\ Vault/Research \
  --frontier projects/bbb-flagship/.vela \
  --dry-run \
  --max-files 30
```

If the dry-run looks reasonable, run for real:

```bash
./target/release/vela compile-notes \
  ~/Documents/Obsidian\ Vault/Research \
  --frontier projects/bbb-flagship/.vela \
  --max-files 30
```

Expected: 10–20 `finding.add` proposals appended to
`projects/bbb-flagship/.vela/proposals/`.

## 3. Add the priority sources via Literature Scout

The campaign references these papers but doesn't yet have them in
Vela. Pull the PDFs into `~/scratch/alz-pdfs/` (or wherever) and
run:

| Source | Why |
|---|---|
| **Selkoe & Hardy 2016** — *The amyloid hypothesis of AD at 25 years* | Anchors the cascade-hypothesis claims |
| **Shi et al. 2017** — TREM2 mechanism review | Anchors the TREM2 claims beyond the existing ATV:TREM2 paper |
| **Gratuze et al. 2023** — TREM2 review (already partly in Obsidian) | Adds the timing/harm tension explicitly |
| **van Dyck et al. 2023** — *Lecanemab in early AD* (Clarity AD) | Lecanemab efficacy claim with real provenance |
| **Sims et al. 2023** — Donanemab TRAILBLAZER-ALZ | Donanemab efficacy claim |
| **Egan et al. 2019** — Verubecestat trial | The canonical BACE1-failure source |
| **Wessels et al. 2020** — Atabecestat | Liver-toxicity contradiction |

```bash
./target/release/vela scout \
  ~/scratch/alz-pdfs/ \
  --frontier projects/bbb-flagship/.vela \
  --max-files 12
```

Expected: 30–50 additional proposals.

## 4. Review and accept

Open the queue:

```bash
./target/release/vela queue review --frontier projects/bbb-flagship/.vela
```

Or via the workbench at `vela-workbench.fly.dev` once the proposals
are live.

For each proposal, accept (`a`), reject (`r`), or annotate (`n`).
The Reviewer agent can pre-score the queue:

```bash
./target/release/vela review-pending \
  projects/bbb-flagship/.vela \
  --batch-size 8
```

Sign accepted proposals as a batch:

```bash
./target/release/vela queue sign \
  --frontier projects/bbb-flagship/.vela \
  --all \
  --key ~/.vela/keys/private.key
```

## 5. Surface contradictions

```bash
./target/release/vela find-tensions \
  projects/bbb-flagship/.vela \
  --max-pairs 12
```

Expected: ~5–10 `tension`-type findings covering the amyloid
paradox (plaque burden vs cognitive decline), TREM2 timing
benefit/harm, BACE1 substrate-mediated toxicity, and the
combinatorial-therapy question.

Review and sign these the same way as step 4.

## 6. Re-publish to the hub

```bash
./target/release/vela registry publish \
  ./projects/bbb-flagship/frontier.json \
  --owner willblair \
  --key ~/.vela/keys/private.key \
  --locator https://raw.githubusercontent.com/vela-science/vela/main/frontiers/bbb-alzheimer.json \
  --to https://vela-hub.fly.dev
```

Note: this changes the `vfr_id` because the project name and
content both changed. Update `site/src/config.ts` `BBB.locator` if
the locator URL changes.

## 7. Renaming the canonical frontier (optional, end-of-phase)

When the frontier feels right, rename it to match the site:

```bash
# Edit projects/bbb-flagship/.vela/config.toml:
#   name = "Alzheimer's Therapeutics"
#   description = "Live frontier covering Alzheimer's drug targets, mechanisms, BBB delivery, and clinical readouts."
./target/release/vela check projects/bbb-flagship/.vela --strict
./target/release/vela registry publish ...   # republish under the new vfr_id
```

The site already calls the frontier "Alzheimer's Therapeutics" via
the `FRONTIER` constant in `site/src/config.ts`; renaming the
canonical project syncs the on-disk identity to the public face.

## 8. Run the weekly diff

The first Monday after Phase A lands, run:

```bash
scripts/weekly-diff.sh
```

This emits an unsigned `weekly_diff.unsigned` event to
`projects/bbb-flagship/.vela/events/<week>-weekly-diff.json` and
prints the rhythm summary. The site's `/frontier/<week>` page
renders it automatically on next deploy.

The unsigned-marker is a v0.31 stop-gap; the v0.32 replacement is
`vela frontier diff --since <date>` as a Rust subcommand. See
`~/.claude/plans/noble-floating-willow.md` Phase C for that work.

---

## Acceptance gate

Phase A is done when:

- `./target/release/vela stats projects/bbb-flagship/.vela` shows ≥ 80 findings
- `./target/release/vela tensions projects/bbb-flagship/.vela` lists ≥ 5 contradictions
- The site, rebuilt, shows non-zero counts on the Tau and ApoE
  drug-target chips on the homepage (currently both 0)
- A clean visitor lands on `/targets/tau` and sees real claims, not
  the empty-state.

Until those are true, Phase B's site rebuild is reading against
under-density content. The site is fine; the frontier is what's
thin.
