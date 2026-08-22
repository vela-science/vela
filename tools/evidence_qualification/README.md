# Evidence runtime qualification

This is the maintained, neutral qualification and custody boundary extracted
from the inherited-correction evidence campaigns. It is conformance tooling,
not Protocol 1 conformance: it creates no Vela object, runs no controller or
scheduler, invokes no provider, opens no protected answer, and has no authority
or Standing effect.

The command accepts one self-contained bundle and emits a canonical receipt
only when the entire no-science boundary passes while participant permits
remain held:

```bash
uv run --project conformance --locked python \
  tools/evidence_qualification/qualification.py \
  --bundle /absolute/path/to/qualification-bundle
```

The executable bundle template is `BundleFixture` in
`test_qualification.py`. A consumer should generate the same closed layout for
its own runtime and pin the resulting qualification root plus the Vela commit
and tree containing this qualifier. Qualification must finish before the first
paid or scientific participant permit is released.

## Owned boundary

The qualifier owns generic checks that remain meaningful when every scientific
case is removed:

| Campaign failure | Permanent owner and check |
| --- | --- |
| Provider rejection of `uniqueItems` and other unsupported schema surface | Exact, allowlisted provider derivative; the full registered schema remains byte-bound and authoritative for local validation. |
| Draft mismatch | `Draft202012Validator.check_schema` and validation of the neutral response under the full registered schema. |
| Hidden response ordering | Exact unique closed-set comparison followed by a derived canonical order; raw response bytes are retained unchanged. |
| Permit reuse or implicit release | Closed permits bind registration, assignment, participant/run, condition, attempt, runner/runtime/config/image/schema/prompt/packet roots, timeout, issuance, and consumption. Default hold plus descriptor-relative, no-overwrite atomic link/unlink consumption makes replay or cross-assignment use fail closed. |
| Runtime evidence not reaching the benchmark | Closed launch, event, usage, terminal, and teardown records bind one monotone thread/turn/response lifecycle plus exact consumed-permit, configuration, runtime, stderr, raw-response, and teardown bytes. |
| Mutable scoring inputs and reread races | No-follow, read-once pre-key buffers whose byte digests and snapshot root are checked before a protected boundary may open. |
| Cross-Python numeric drift | Decimal-only half-even quantization and a serializer that never converts through binary float. |
| Stale or permissive runner configuration | Closed configuration fields plus an offline strict-parse receipt binding the exact recognized runner version, configuration root, runtime source, Dockerfile, image, accepted arguments, and zero provider contact. |
| Relative or aliased container mounts | Absolute, canonical, unique, read-only source mounts and absolute targets. |
| Missing TLS roots | A nonempty PEM bundle, exact digest, and equality between the pinned container path and `SSL_CERT_FILE`. |
| Cache-dependent OCI identity | Two distinct empty-cache builder receipts, byte-identical complete OCI archives, and exact index/manifest/config/ordered-layer/layout/archive custody, frozen source epoch, no pull, no provenance, and timestamp rewrite. Missing, duplicate, reordered, extra, or substituted graph bytes fail closed. |
| UTC-day account drift or malformed account records | Exactly two byte-distinct, path-distinct, date-distinct fixtures with closed account metadata must normalize to one expected digest; duplicates, malformed dates, wrong accounts, and metadata drift fail closed. |
| Mutable package metadata or incomplete vendored inputs | Effective Dockerfile instructions are parsed (comments do not count), network package-manager operations are rejected, and each vendored input binds bytes, source digest/URL, and retained license bytes. |
| Self-check of a predecessor artifact | The self-verification command must retain the locked virtual-environment interpreter without resolving its symlink, and bind its environment prefix, environment-owned `jsonschema`, current qualifier bytes, and current canonical bundle path exactly. |
| Bundle aliases and traversal | The exact bundle tree rejects every symlink and hardlink alias; descriptor-relative no-follow reads reject traversal and component replacement before any role-specific bytes are trusted. |
| Cumulative token telemetry mistaken for a validity limit | Nonnegative token telemetry is retained; only the configured output-token ceiling is a validity gate. |
| Partial preflight before the first participant | One command covers configuration, schemas, runtime/source/build custody, trust, mounts, permit semantics, complete capture, canonical response handling, pre-key snapshot, decimal bytes, and the self-check target. |

The source-owned runner still owns provider invocation, process lifecycle, OCI
construction, and production of the receipts consumed here. Controllers,
schedulers, attempts, runs, assignments, campaign policy, and scientific
scoring stay outside Vela Core.

Builder independence remains an exact two-receipt attestation checked by this
qualifier; the qualifier does not build images. It also deliberately performs
no provider call. A future provider incompatibility therefore requires a new
maintained allowlist entry and regression, never an artifact-local patch.

The Math Result Runner already owns Lean-specific runtime, compiler,
elaborator, evaluator, source-verification, and campaign-permit behavior under
`vela-science/math/tools/result_runner/next_campaign_v1` (live Math
`origin/main` was `cf6d766` during extraction). None of that is duplicated
here.

## Historical records and migration

The producer branch `origin/codex/inherited-correction-study` is source
evidence only. This extraction does not edit, reinterpret, or rerun:

- the sealed 16-cell result retained at `3207066f`;
- the provider-schema stopped registration retained through `d3bff920`;
- the order-contract stopped registration retained at `f14616e3`;
- the complete final 36-cell capture at `5694beba`; or
- the scored final result at `4524c8f7` and its independent review lineage.

The final 36-cell claim ceiling is unchanged: Git/documents was 12/12 exact,
the neutral structured wrapper was 12/12, Vela was 11/12 with one authority
error, and every preregistered positive gate was false (`positive_gate` is
`not_supported`). It establishes no scientific acceptance, external
replication, broad productivity lift, Protocol or Core change, Repository
authority, Standing, or Decision effect.

Historical artifact code remains immutable. A future artifact or workbench
must consume this maintained qualifier by commit/tree and qualification root
instead of copying another `custody.py`, runtime preflight, permit helper, or
scoring snapshot. Existing historical roots never change merely because their
mechanism now has a maintained successor.

Run the focused regression suite with:

```bash
uv run --project conformance --locked python -m unittest \
  tools.evidence_qualification.test_qualification -v
```
