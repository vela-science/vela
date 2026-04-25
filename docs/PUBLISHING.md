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
  --assertion "Lecanemab showed modest cognitive benefit in Clarity AD" \
  --type therapeutic \
  --evidence-type experimental \
  --source "van Dyck et al., 2023, NEJM 388:9-21 (Clarity AD)" \
  --source-type clinical_trial \
  --author "reviewer:you" \
  --confidence 0.80 \
  --doi "10.1056/NEJMoa2212948" \
  --year 2023 \
  --journal "New England Journal of Medicine" \
  --source-authors "van Dyck CH;Swanson CJ;Aisen P" \
  --conditions-text "Patients with mild cognitive impairment or mild AD dementia, ages 50-90, with confirmed amyloid PET positivity. Phase 3 RCT, 18 months, biweekly IV infusion." \
  --species "Homo sapiens" \
  --human-data --clinical-trial \
  --entities "Lecanemab:compound,amyloid-beta:protein,Alzheimers disease:disease" \
  --apply
```

The v0.11 flags above (`--doi`, `--pmid`, `--year`, `--journal`, `--url`, `--source-authors`, `--conditions-text`, `--species`, `--in-vivo`, `--in-vitro`, `--human-data`, `--clinical-trial`) populate the structured `Provenance` and `Conditions` fields of the finding bundle. They're all optional — pre-v0.11 invocations still work — but every one you supply makes the finding queryable and verifiable in ways prose alone is not.

Valid enum values are surfaced in `vela finding add --help`. Invalid values
are rejected at add-time, not deferred to strict validation.

| Flag | Allowed values |
|---|---|
| `--type` | `mechanism`, `therapeutic`, `diagnostic`, `epidemiological`, `observational`, `review`, `methodological`, `computational`, `theoretical`, `negative`, `measurement`, `exclusion` |
| `--evidence-type` | `experimental`, `observational`, `computational`, `theoretical`, `meta_analysis`, `systematic_review`, `case_report` |
| `--source-type` | `published_paper`, `preprint`, `clinical_trial`, `lab_notebook`, `model_output`, `expert_assertion`, `database_record`, `data_release` |
| Entity types | `gene`, `protein`, `compound`, `disease`, `cell_type`, `organism`, `pathway`, `assay`, `anatomical_structure`, `particle`, `instrument`, `dataset`, `quantity`, `other` |

v0.10 added the domain-neutral entries — `measurement`/`exclusion` for assertion type, `data_release` for source type, and `particle`/`instrument`/`dataset`/`quantity` for entity type. They surfaced from publishing the first non-bio frontier on the public hub (a particle-astrophysics WIMP-direct-detection frontier). Pre-v0.10 frontiers continue to validate; the additions widen expressiveness without churning content addressing.

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

When the dep republishes (BBB does so weekly via CI), your local pin
goes stale. Refresh it:

```bash
# v0.11: re-pin every cross-frontier dep to the hub's current snapshot.
# Use --dry-run first to see what would change.
vela frontier refresh-deps ./frontier.json --dry-run
vela frontier refresh-deps ./frontier.json
```

The command reports per-dep `unchanged`, `refreshed` (with old → new
snapshot), `missing` (dep no longer on the hub), or `unreachable`
(network failure). After a successful refresh, republish the frontier
to the hub so your manifest reflects the new pin.

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
