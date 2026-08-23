# Final independent corrective review: Anthropic open diagnostic pilot

## Verdict

**BLOCKED** at final producer commit
`6f2bdc6007c1cc1bde60b433da947dbcb7935368`, tree
`c666e41452cddcc9c3124a6f603d46396ee3eaa9`.

The four findings from review `5743c4f58f7a91ec3bb67ccaf28c5e3701c1d96a`
are individually repaired in their direct implementations. The new evidence
trees and compiled descriptor-rooted tool bridge are real, the zero-provider
terminal case stays in the denominator, conservative uncertainty is safe, and
the scorer and bridge block their deterministic pathname-replacement tests.

The complete execution-to-score path is not yet closed. A successful run that
uses the registered file tool necessarily performs at least two provider HTTP
calls, but the scorer permits only zero or one and does not consume the raw
provider/tool lifecycle records at all. Separately, the new evidence
materializer and the generic maintained qualifier still have the same pre-open
pathname-substitution gap that was fixed in the scorer and Go bridge. The
changed maintained/artifact Python also fails the repository's locked Ruff
gate.

## Exact producer and artifact binding

- final producer: `6f2bdc6007c1cc1bde60b433da947dbcb7935368`
- final tree: `c666e41452cddcc9c3124a6f603d46396ee3eaa9`
- final parents:
  `b24acf82e9597424d75ca20ca5f5873c6fdece77` and
  `66e33872e3901d2a24ff1730c0273b2dc7e19e0e`
- corrective merge tree: `2d548124e03ef87385ef561b6679686b059f4d86`
- corrective merge parents:
  blocked producer `2d3b53575bd9465a0331b0e9fbf99510b05001f9`
  and prior main `cc3b88d8bfcfd7b4f720a023f049d5c365be9423`
- final reconciliation's main-only delta is confined to the separately merged
  negative flagship paper.
- diagnostic artifact root:
  `sha256:dbee02128aad44b896dca57a80bb262c45eaa3b8a7fd7716181b128b0ab385ae`
- assignment root:
  `sha256:99df881e189024c6c134ea3e521e5748cc901f56a1a5e25a8b49042b27e6bb79`
- permit-set root:
  `sha256:2af4b9ac91254429e313a7f03ec61da4f718574964a55908409eef4bd8766d07`
- registration root:
  `sha256:56ee9300334cc85d6dc85e5edacad249b2627d48f9a9ddb155b01b3cd8ebcf40`
- custody root:
  `sha256:347bc20241ba104d5d9fb4ef4947b4f0d31eaf7c32641b3a1a6048d3e38045bb`
- evidence-source catalog root:
  `sha256:0ac4cc728cda49da0e12a1407737d4731aad6b13a7ef53be482d289ad6dc1fa1`
- runtime artifact root:
  `sha256:6151d3a546514d5255f2792495e5da86bb77b4f81ad6c23e784052e0b9a70bca`
- runtime offline record root:
  `sha256:51df9fe89d649e5fbb6519d2f02eefaaf5dc672c350de1fcc58fab5047944e3f`
- runtime registration root:
  `sha256:f84bd9dcd6f9de6f8765c1ad25361f6579d7721cdc8d57937ad55c4205988ed4`
- Anthropic configuration root:
  `sha256:fe8c8a8f320f8179f343202403d3cf37b50f77c2b38495d2dc9ef739ab34f4fa`
- tool-policy root:
  `sha256:c98932d0d5bcd5956b8c578f0f42bd3c94e827d5c5ca5b8034ca9bfd1799ffd2`
- maintained offline-qualification root:
  `sha256:74e07177b119edf0e9fcf18940cce9fa06757526092bc38f18595471debb623e`
- image digest:
  `sha256:315fd2ae42a140f3be8dd05d34031f83aca6fa29e421f86ca335a4dfafd6b2f6`
- OCI archive root:
  `sha256:5f168fb3cef351ee983fa7fe12926acc3e6de3aa79bc194c61721bc9503b7799`
- runtime-source root:
  `sha256:4a44868aaf4a5d00dd7c21aa9e95ced13c9674b659b3e24aca6bac90d15ad460`

## Reproduced passes

The review used a hosted-remote clone detached at the exact final producer.
Remote branch equality, commit/tree/parent topology, `git fsck --full --strict`,
and clean status passed. Deterministic regeneration reproduced the artifact
root with zero diff. The normal verifier and the exact maintained-qualifier
mode both passed. All six v2 execution bundles independently returned their
frozen `qualified_hold` receipts.

Focused gates passed:

- 38/38 diagnostic package and scorer tests;
- 4/4 evidence-tree tests, including two-location byte identity;
- 58/58 generic maintained evidence-qualification tests;
- all Go runtime/bridge packages, including deterministic symlink, external
  hardlink, and pathname-replacement rejection;
- seven destructive held-bundle adversaries: missing evidence, substituted
  evidence, symlink, external hardlink, removed mount, stale workspace
  preflight, and forged tool-result bytes;
- all 365 JSON files parsed.

Every cell now has a content-addressed workspace and a closed assignment
manifest. Each packet-referenced path is present at its exact size and digest;
the compiled tool can read/list/stat/literal-search it. The qualifier binds the
real read-only `/workspace` mount, per-cell evidence manifest, workspace
content, workspace preflight, and boundary roots into the exact permit and
fixture launch/terminal/teardown receipts. The policy root is stable across all
six cells while the per-cell workspace/boundary roots remain distinct.

The raw and assisted trees have the registered information difference only.
Within every case they have byte-identical base atoms and supplemental source
evidence; raw has no derived atoms; assisted adds exactly the registered
derived set: 3 for the invalid fixture, 2 for Erdős 730, and 3 for
FC-to-LeanEval. The original Stage A prompts, packets, case selection, response
schema, assignment schedule, hold state, and 12 participant permits are
byte-identical to their frozen producer.

The direct scorer repairs pass. A fully root-resealed zero-provider terminal
failure is retained as a fixed-denominator non-result. The closed per-case
safety maps treat `unprovable`/`not_established` conservatism as safe and reject
claims above the evidence ceiling. Its descriptor-relative reader compares
pre-open, opened, post-read, and named-path identities and rejects the prior
deterministic substitution attack.

State remains exactly 0/6: all six diagnostic and all twelve original Stage A
participant permits are held, none released or consumed, and credential
content accesses, provider calls, responses, terminal captures, and score
attempts are zero. The exact independently reviewed 36-cell negative result
remains bound: Git/documents 12/12, neutral wrapper 12/12, Vela 11/12 with one
authority error, all positive gates false, `positive_gate=not_supported`, and
`authority_effect=none`. No Stage B selection or authority action exists.

## Blocking findings

### AD-F1: tool-using runtime captures cannot satisfy the scorer

The compiled bridge implements the registered sequential tool lifecycle by
issuing one provider request, executing at most one tool call from that
response, then issuing a continuation request. It increments `providerCalls`
on every endpoint attempt and allows up to 64 turns. Therefore a successful
response with one tool call has two provider calls; `N` tool calls have
`N + 1` provider calls.

`scorer.py` accepts only `provider_calls in {0, 1}` and requires exactly one for
every successful response. Any actual tool-using response is thus unscorable.
This conflicts with the registered read/list/stat/search condition and with
the qualifier's sequential tool lifecycle.

The scorer also requires only six synthetic roles: launch, custody, raw
response, teardown, terminal, and usage. It consumes no raw provider-event
stream, normalized lifecycle, request frame/request bytes, tool receipts,
tool stdout/stderr, or consumed-permit bytes. Its closed launch, custody,
terminal, and teardown objects omit the workspace, evidence-manifest,
tool-boundary, tool-policy, workspace-preflight, raw-event, and tool-receipt
roots that the maintained runtime captures. No committed runtime-capture to
diagnostic-score compiler exists. The test helper can return
`diagnostic_gate_pass=true` with `tool_call_count=1` while containing no tool
receipt or provider-event role at all.

Smallest correct repair: add one maintained, deterministic capture compiler
that consumes the complete exact runtime evidence set rather than accepting a
parallel synthetic receipt vocabulary. It must validate the consumed permit;
launch/request/transport bytes; all raw and normalized provider events; every
sequential endpoint attempt; every tool call/result receipt and exact
stdout/stderr; terminal, usage, teardown, workspace/evidence/policy roots; and
the final raw response. Bind the compiled capture root before the single score.
Permit provider-call counts from zero through the registered bound, with
status-specific rules and exact `provider_calls = tool_call_count + 1` for a
successful sequential tool-using response. Add positive 0-tool, 1-tool, and
multi-tool score fixtures plus missing/reordered/forged tool and continuation
negatives.

### AD-F2: two newly trusted Python readers retain the pathname race

The scorer and Go bridge now close the prior race, but
`evidence_tree.regular_bytes` and the generic maintained qualifier's
`_read_bundle_regular` do not. Both validate or assume a pathname, call
`os.open`, and compare only the opened descriptor to itself after reading.
Neither proves that the opened descriptor is the device/inode observed at the
name before open, nor that the named path still resolves to that inode after
the read.

An independent deterministic `os.open` interposition made each function
validate `victim` but open a separate single-link `forged` file. Both returned
`b"FORGED"` successfully. Downstream hashes catch ordinary byte drift in many
current callers, but that does not satisfy the registered all-bound-read
descriptor custody claim and leaves generic maintained code with a known race.

Smallest correct repair: implement one maintained descriptor-relative,
no-follow reader that binds every directory component and final file across
pre-open named stat, opened `fstat`, post-read `fstat`, and post-read named stat;
requires exact device/inode/type/size/link-count equality; holds descriptors
through validation; and returns bytes only after the postcondition. Reuse it in
the qualifier and evidence materializer instead of maintaining separate partial
copies. Add the exact deterministic replacement regression to both call paths.

### SQ-F1: the locked source-quality gate fails

The exact locked `ruff check tools/evidence_qualification` command fails on two
new `RUF017` findings in `qualification.py`. A focused Ruff run over the ten
authored diagnostic Python files finds ten additional issues: import
modernization/order, stale `noqa` directives, and one wrong exception class.
The wider artifact-directory run additionally scans frozen source evidence and
is not used as a gate, but these twelve authored-source findings are direct.

Smallest repair: make the maintained and focused artifact source pass the
locked Ruff check and format gates without altering evidence-source bytes; then
rerun deterministic generation, all qualifier/runtime/scorer tests, and exact
independent review.

## Claim boundary

The preserved prompts include an inherited Stage A sentence saying no
correspondence record or derived answer is supplied while the assisted arm, by
the frozen reviewed design, supplies its registered correspondence derivatives.
This review does not reinterpret or silently edit that inherited scientific
design. The diagnostic's ceiling remains only Anthropic reviewer-agent
feasibility on these exact open cases; it cannot establish a mechanism lift,
the two-provider Stage A, G3, Phase 0, Stage B, cross-provider or human benefit,
scientific acceptance, a breakthrough, Frontier expansion, Protocol/Core,
Repository authority, Decision, or Standing.

No producer byte was changed. No permit was released or consumed, no credential
was opened, no provider was called, no participant response was generated, and
no scoring, merge, Stage B, Protocol/Core, authority, Decision, or Standing
action occurred. This review authorizes none of those actions.
