# Publishing your first frontier

This is the path from zero context to a signed frontier on
<https://vela-hub.fly.dev>. The whole loop is ~10 minutes if you have the
binary built.

The doctrine is unchanged from `docs/HUB.md`: dumb signed transport. Anyone
with an Ed25519 key publishes their own `vfr_id`; the hub stores canonical
bytes verbatim; clients verify locally on read.

## Prerequisite

A built `vela` binary on PATH. From a checkout:

```bash
cargo build --release -p vela-protocol --bin vela
export PATH="$PWD/target/release:$PATH"
vela --version    # vela 0.9.0
```

## 1. Scaffold a publishable stub

```bash
mkdir my-frontier && cd my-frontier
vela frontier new ./frontier.json \
  --name "Your bounded question" \
  --description "One paragraph describing the scope of this frontier."
```

`vela frontier new` writes a fresh `frontier.json` that passes
`vela check --strict` immediately. Use this — not `vela init` — when your
target is the hub. (`init` creates a `.vela/` repo, which is for local
development only and is not directly publishable in v0.)

## 2. Add findings

Each call appends a content-addressed finding via the standard
proposal → event → reducer flow:

```bash
vela finding add ./frontier.json \
  --assertion "Liraglutide reduces amyloid plaque burden in APP/PS1 mice" \
  --type therapeutic \
  --evidence-type experimental \
  --source "Hansen et al., 2015, J Neuroinflammation" \
  --source-type published_paper \
  --author "reviewer:you" \
  --confidence 0.55 \
  --entities "liraglutide:compound,amyloid-beta:protein,APP/PS1:organism" \
  --apply
```

Valid enum values are surfaced in `vela finding add --help`. Invalid values
are rejected at add-time, not deferred to strict validation.

| Flag | Allowed values |
|---|---|
| `--type` | `mechanism`, `therapeutic`, `diagnostic`, `epidemiological`, `observational`, `review`, `methodological`, `computational`, `theoretical`, `negative` |
| `--evidence-type` | `experimental`, `observational`, `computational`, `theoretical`, `meta_analysis`, `systematic_review`, `case_report` |
| `--source-type` | `published_paper`, `preprint`, `clinical_trial`, `lab_notebook`, `model_output`, `expert_assertion`, `database_record` |
| Entity types | `gene`, `protein`, `compound`, `disease`, `cell_type`, `organism`, `pathway`, `assay`, `anatomical_structure`, `other` |

Validate any time:

```bash
vela check ./frontier.json --strict
```

`--json` returns per-failure detail under `checks[].errors[]` and
`checks[].blockers[]`.

## 3. Optional — compose with another hub frontier

If your finding extends, contradicts, or depends on a finding in another
hub-published frontier, declare the dep first, then add the typed link.
The dep pins the remote by `vfr_id` + snapshot hash; cross-frontier
targets without a declared dep are refused by strict validation.

```bash
# Get the remote frontier's current snapshot from the hub
curl -s https://vela-hub.fly.dev/entries/vfr_093f7f15b6c79386 | jq

vela frontier add-dep ./frontier.json vfr_093f7f15b6c79386 \
  --locator https://raw.githubusercontent.com/vela-science/vela/main/frontiers/bbb-alzheimer.json \
  --snapshot f23b4aba173f3fb840c6ec2555715bdcdd7c90864019aff399a43a6ff554c6ec \
  --name "BBB Flagship"

vela link add ./frontier.json \
  --from vf_<your-finding> \
  --to vf_<remote-finding>@vfr_093f7f15b6c79386 \
  --type extends \
  --inferred-by reviewer
```

## 4. Sign and register your publisher identity

```bash
mkdir -p keys
vela sign generate-keypair --out keys

vela actor add ./frontier.json reviewer:you \
  --pubkey "$(cat keys/public.key)"
```

`reviewer:you` is your stable signing identity; the hub binds publish
rights to it via the embedded pubkey. Treat `keys/private.key` like a
production secret — don't commit it; for CI use a secret store.

## 5. Publish to the public hub

The hub stores the signed manifest; the frontier file lives wherever you
host it (the `--locator`). Typical choices: a `raw.githubusercontent.com`
URL on a public repo, an S3 object, or your own domain.

```bash
vela registry publish ./frontier.json \
  --owner reviewer:you \
  --key keys/private.key \
  --locator https://your-host.example.com/frontier.json \
  --to https://vela-hub.fly.dev \
  --json
```

The CLI prints the assigned `vfr_id`, the snapshot/event-log hashes, and
the `signed_publish_at` timestamp. Verify it landed:

```bash
curl -s https://vela-hub.fly.dev/entries/<your-vfr_id> | jq
```

Anyone can now pull and verify:

```bash
vela registry pull <your-vfr_id> \
  --from https://vela-hub.fly.dev/entries \
  --out ./pulled.json
```

`pull` fetches the frontier from your locator, verifies the signature
against your declared `owner_pubkey`, and rejects on any hash mismatch.
For frontiers with cross-frontier deps, add `--transitive` to walk the
graph.

## CI republish (the BBB pattern)

A bot is just an actor whose private key lives in a CI secret. The
substrate treats human-signs and bot-signs identically. See
[`.github/workflows/bbb-living-repo.yml`](../.github/workflows/bbb-living-repo.yml)
for a worked example: weekly cron, fresh `signed_publish_at` per run,
no recompilation in CI.

## What can go wrong

- **`invalid --type 'x'`**: pick from the table above. Strict and CLI
  enums are single-sourced from `bundle.rs`.
- **`Finding id 'vf_…' does not match content-address 'vf_…'`**: you
  hand-edited a finding's `assertion.text`/`assertion.type`/provenance
  after creation. The ID is derived; the simplest fix is to delete the
  finding and re-add via `vela finding add`.
- **`cross-frontier --to references vfr_id 'vfr_…' but no matching dep
  is declared`**: run `vela frontier add-dep` first.
- **`owner '…' is not registered in the frontier`**: run
  `vela actor add` to bind your pubkey to the actor id you pass to
  `--owner`.
- **`Project stats.links N does not match aggregated links M`**:
  before v0.9 this could happen after a hand-edit. `vela link add`
  recomputes stats; for hand-edits, re-run `vela check` to see what
  drifted.
