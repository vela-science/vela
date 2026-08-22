# Held-out inherited-correction benchmark

This non-authoritative paper artifact is a fresh prospective replacement for
the stopped 36-cell launch whose sole run was retained as a provider-schema
non-result. The stopped evidence and the sealed earlier 16-session study with
`positive_gate=not_supported` remain unchanged. No replacement participant call
has been made.

The follow-up contains three new synthetic families: provenance revocation,
taxonomy remap, and method-version correction. Each family has four sessions
in each of three conditions, for 36 fixed cells and a 12/12/12 aggregate
balance. The conditions are Git/documents, a neutral structured-state wrapper,
and Vela. Every packet is generated from one atomic family source and shares
the exact source and evidence bytes within its family. Conditions may organize
those facts differently but may not add facts.

The neutral wrapper exposes predecessor/successor, dependencies,
active/superseded/needs-recheck/current views, and neutral scope and acceptance
facts. It contains no Repository, Decision, Event, Standing, authority-scoped
replay, or Vela vocabulary. This third arm separates structure lift
(wrapper minus Git/documents), governance/inheritance lift (Vela minus
wrapper), and total lift (Vela minus Git/documents). Presentation length is
prospectively bounded to a maximum 1.20 ratio within each family.

The families prospectively exercise three authority regimes: no acceptance
action, an independently authorized acceptance action, and a referenced action
whose authorization is presently unprovable.

The response contract uses closed generic authority-effect,
authority-action, consequence-classification, and safe-action codes. Evidence
bindings are structured packet-member `{path, sha256}` pairs. Explanatory prose
cannot enter or alter exact code scoring.

The registered Draft 2020-12 response schema is byte-exact. A provider-only
derivative deletes exactly `/properties/evidence_bindings/uniqueItems`, the
single keyword rejected by the provider. The locked local Ajv validator still
validates every response against the full registered schema, and the benchmark
separately requires four distinct complete bindings. Custody binds both schema
roots.

## Held state and protected answers

All 36 permits are held and unconsumed. The producer artifact contains no
held-out adjudication bytes or answer mapping. An independent evaluator froze
the protected adjudication outside producer custody and disclosed only its
canonical root and public custody metadata. The transparent prospective launch
amendment binds that commitment and the exact user authorization. Independent
review of the amended held bytes is still required before any permit may be
released.

A distinct neutral schema calibration identity and single-use permit are also
held. It contains no study packet or family facts, has no denominator credit,
and cannot be released until the replacement prelaunch review passes. No study
permit may be released until that one calibration is terminal and independently
confirmed.

The pinned model, one-turn container runtime, image, trust bundle, 600-second
timeout, zero-tool boundary, attempt 1, and zero-retry policy are unchanged
from the qualified runtime. There is no scheduler or multi-run launch command.
`custody.py ingest` accepts one exact assigned run and requires one consumed
permit, one provider request, one turn, zero tools and compactions, terminal
receipt, event stream, stderr, and response bytes. `freeze` requires all 36
ingested records before scoring can reach the protected key.

The first replacement prelaunch was independently blocked because its image
identity was recoverable from a shared BuildKit cache but did not reproduce in
a clean build. The transparent runtime-reproducibility amendment records that
F08 finding. The repaired build fixes `SOURCE_DATE_EPOCH`, removes npm and Node
compile caches from their creating layers, disables build cache and provenance,
and uses the OCI exporter's timestamp rewrite. Two independent `docker-container`
builders with empty caches produced byte-identical OCI layouts. This repair
changes only the runtime image and identities derived from it; participant
packets, prompts, schemas, facts, assignments, gates, and the stopped evidence
are unchanged.

A second prelaunch reproduction, performed after the neutral calibration PASS,
found one remaining UTC-day dependency before any participant permit was
released. File-level comparison isolated exactly one differing byte: `useradd`
wrote the current day into the participant entry in `/etc/shadow`. The second
prospective repair validates the complete participant shadow record and fixes
its last-change field to `SOURCE_DATE_EPOCH / 86400`. Cross-day fixtures now
converge byte-for-byte, malformed account records fail closed, and two fresh
empty-cache builders again produce byte-identical complete OCI archives. The
study remains held at 0/36 pending fresh independent F08/G08 review; the neutral
calibration is not rerun.

## Deterministic checks

From the repository root:

```bash
runtime=paper/artifacts/inherited-correction-benchmark-execution/container-runtime-provider-schema-v2
first_oci=/ABSOLUTE/TEMP/PATH/schemafix-a.oci.tar
second_oci=/ABSOLUTE/TEMP/PATH/schemafix-b.oci.tar
"$runtime/build-reproducible-oci.sh" INDEPENDENT_EMPTY_BUILDER_A "$first_oci"
"$runtime/build-reproducible-oci.sh" INDEPENDENT_EMPTY_BUILDER_B "$second_oci"
"$runtime/verify-reproducible-oci.sh" \
  "$first_oci" "$second_oci" \
  sha256:71bceb9885958619b129d7567b56277422f4c1d17c85a7076fb0d60c07633dea
docker load --input "$first_oci"
paper/artifacts/inherited-correction-benchmark-execution/container-runtime-provider-schema-v2/preflight-provider-schema.sh \
  sha256:71bceb9885958619b129d7567b56277422f4c1d17c85a7076fb0d60c07633dea \
  "$PWD/paper/artifacts/inherited-correction-held-out-replacement/calibration/input" \
  EMPTY_OUTPUT_DIRECTORY
python3 paper/artifacts/inherited-correction-held-out-replacement/benchmark.py verify
python3 paper/artifacts/inherited-correction-held-out-replacement/custody.py verify-prelaunch
python3 paper/artifacts/inherited-correction-held-out-replacement/test_benchmark.py
python3 paper/artifacts/inherited-correction-held-out-replacement/test_provider_schema_runtime.py
git diff --check
```

Each named builder must be an independent `docker-container` builder with an
empty BuildKit cache. The wrapper fixes Linux arm64, disables cache, provenance,
and pull, supplies the frozen source epoch, and exports an OCI layout with
timestamp rewriting. The verifier requires the two complete OCI tar files to
be byte-identical and to bind the pinned manifest digest. The provider-schema
preflight runs with container network disabled and must leave
`provider-events.jsonl` empty. Its output directory must exist and be empty
before invocation.

The tests cover generic closed authority codes, exact path/digest bindings,
atomic-information and prompt-length equivalence, neutral-wrapper forbidden
vocabulary, all three authority regimes, answer-key and cross-family leakage,
root-bound permit identity, missing terminal receipts, the held adjudication
gate, fixed family/arm balance, pairwise decomposition, governance equality
rejection, and canonical decimal result bytes. Cross-version qualification is
required under supported CPython 3.10 through 3.14 before prelaunch review.

## Claim ceiling

The family-balanced additive estimands and gates were fixed before external
adjudication and before any session. They separately bind exact success,
correction-impact completeness, authority safety, and restricted completion
time. The preserved total Vela-versus-Git gate is unchanged. Structure uses a
proportionate parallel gate. Governance requires family noninferiority plus a
strict aggregate increment; equality alone cannot support a governance claim.
A future pass would be bounded descriptive evidence for these three synthetic
families only, not a broad significance result.
It would not establish scientific truth, general productivity, adoption,
acceptance, or any Vela authority or Standing effect.
