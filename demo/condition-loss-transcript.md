# Condition loss transcript

This is the BBB/TfR proof pattern Vela should preserve.

## Naive summary

> TfR-targeted delivery improves brain delivery for Alzheimer therapeutics.

## Vela frontier answer

Vela should not collapse exposure, delivery, efficacy, species, assay, payload,
endpoint, and human translation into one conclusion.

Current review shape:

- `exposure`: candidate support when a source reports increased apparent CNS
  exposure under a declared model, payload, affinity, route, assay, and endpoint.
- `therapeutic efficacy`: separate finding class requiring functional or disease
  endpoint evidence, not exposure alone.
- `mouse evidence`: condition-bounded source evidence; it does not silently
  become a human clinical claim.
- `missing comparator`: warning surface when an evidence atom cannot identify
  the control or baseline that makes the measurement meaningful.
- `synthetic/agent source`: source artifact requiring review, not primary
  evidence by default.
- `stale proof`: any new review/correction event should force proof packets to
  be regenerated or marked stale by check/proof validation.

## Expected tool path

```bash
vela check frontiers/bbb-alzheimer.json --strict --json
vela search "TfR BBB exposure efficacy mouse human" --source frontiers/bbb-alzheimer.json
vela proof frontiers/bbb-alzheimer.json --out /tmp/vela-bbb-proof
jq 'length' /tmp/vela-bbb-proof/sources/source-registry.json
jq '.[] | {id, source_id, finding_id, locator, caveats}' /tmp/vela-bbb-proof/evidence/evidence-atoms.json
```

## Review rule

If a claim says "delivery improves" but the evidence atom only supports mouse
exposure, Vela should preserve that mismatch as a review surface. It should not
rewrite the finding into a broader therapeutic or human translation claim.
