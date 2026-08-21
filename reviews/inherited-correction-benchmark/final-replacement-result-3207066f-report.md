# Independent inherited-correction replacement result review

## Verdict

**BLOCKED**, bound to producer result commit
`3207066f22f09b578f354b7028f55559e7b45926`, tree
`451237b4b85df33da5b8d8442fe67bd60b8d3b08`, whose sole parent is the exact
all-capture commit `0aa38e9413d7fa4c03aa799200c88b8ed867f5c9`.

Custody, the fixed denominator, frozen packet/input equivalence, the pre-key
capture gate, immutable-snapshot scoring order, and the stated claim ceiling
pass. The final artifact is nevertheless blocked by one exact reproducibility
finding: the registered and documented unpinned `python3` scorer emits
different canonical result bytes across available Python versions.

This verdict does not invalidate the 16 retained terminal records or change
their registered denominator. It does not support a positive Vela result:
under both observed serializations the preregistered gate is
`not_supported`. It authorizes no rerun, substitution, merge, scientific
acceptance, Protocol/Core change, Decision, authority action, or Standing
effect.

## F06 — exact scored-result bytes are interpreter-dependent

The committed result has SHA-256
`1f1d886c778e8fef0effce59692f761eb6d937afa9421880aed3340079004679` and
contains:

- Vela restricted mean `233.07823840475`; and
- restricted-mean ratio `0.38846373067458334`.

From a fresh detached checkout of the exact producer commit, the documented
command under the checkout's `python3` (CPython 3.11.2) recomputed the capture,
read the frozen snapshot, and produced SHA-256
`2c7d6e4a3e0cf43c07ea19003dda65887d2965dd5e79f389c5b5960384624caa`.
Only two fields differ:

- Vela restricted mean `233.07823840474998`; and
- restricted-mean ratio `0.3884637306745833`.

The exact arithmetic reproduction is:

```text
x = [15.592438382, 13.723074048, 12.586077922, 11.777694964,
     600, 10.946621922, 600, 600]
restricted_mean = sum(x) / 8
ratio = restricted_mean / 600
```

CPython 3.10, 3.11, 3.12, and 3.13 on the review host emit the longer mean and
shorter ratio. CPython 3.14 emits the committed mean and ratio. The registration
does not bind a scorer interpreter or define version-independent decimal
arithmetic/rounding, and the README instructs `python3`. The scorer uses binary
floating-point `sum` followed by ordinary JSON number serialization.

All categorical and integer outputs agree: Git/documents remains 112 points,
zero exact successes, and eight authority errors; Vela remains 130 points,
five exact successes, and three authority errors; the fixed denominator remains
16; and `positive_gate` remains `not_supported`. The defect is therefore at the
claimed exact deterministic result-byte boundary, not a changed substantive
conclusion.

Minimal closure requires a transparent post-result corrective amendment that
records this blocked review and the already observed result, then either binds
an exact scorer runtime or defines canonical decimal/rounding semantics with a
cross-version fixture. A corrected result must be regenerated only from the
same sealed capture root; no participant may be rerun or substituted.

## Custody and denominator

The immutable ref, result commit/tree/parent, all-capture commit/tree, and
prelaunch commit/tree match the handoff exactly. The history from prelaunch to
capture is a linear sequence of 16 commits. Commit `n` adds exactly the 36 files
for `replacement-run-n` and its capture, with no other path. Each commit time is
after its terminal receipt and before the next provider start. The capture
commit then adds only `capture-manifest.json`; the result commit adds only
`scored-result.json`. Final-ref Git data cannot independently prove the remote
push instant, but the committed sequence and receipt timestamps are consistent
with the disclosed commit-before-next-run procedure.

Independent fail-closed validation passed for all 16 captures and all 16
ingested runs. It established:

- 16 unique run identities, participant identities, thread IDs, consumed
  permits, and terminal receipts;
- eight Git/documents and eight Vela assignments;
- attempt one, timeout 600, status `completed`, exit code zero, no timeout, and
  no validation error for every run;
- exactly four events, one thread, one turn, one agent response, zero tools,
  zero continuations/compactions, and empty stderr for every run;
- `credential_retained=false` for every receipt;
- exact authorization, assignment, shared/condition configuration, mapping,
  prompt, packet, image, trust, runtime-registration, and runtime-source
  bindings;
- exact byte identity from each frozen permit template to its consumed permit,
  from each raw capture file to its ingested runtime file, and from each runtime
  response to its bridge-generated benchmark response; and
- source permit templates remain unconsumed and both source holds remain
  `hold`.

The independently recomputed complete-runtime-custody root is
`sha256:619512f17009dd92c651a687cbc17dd5899c0b908619d82de465b9747a7aa3f5`.
The independently recomputed capture root is
`sha256:0e5f60fa1dc78e531d44cb8fff626e73c6b2c0017bbcec52e41220cbfac686fd`,
with `adjudication_accessed=false` at capture verification.

The aggregate receipts reproduce exactly 210.053573638 seconds, 313,968 input
tokens, 28,928 cached input tokens, 8,421 output tokens, and 4,184 reasoning
output tokens. Cumulative token data remains telemetry only.

## Packet equivalence and scoring custody

Every capture input directory is byte-identical to its frozen condition input
directory. Every ingested run packet is byte-identical to its registered
condition packet. The registered packet roots remain:

- Git/documents:
  `sha256:bdda8e39a17e50607a4587993dc7fe855fae9408dad2dd0ae11dc47ee281cb6e`;
- Vela:
  `sha256:2bc904703cfd47419846e0a9771c5e9c3933dba5465ec9f48440d1850ace4c97`.

The unchanged benchmark verification confirms the same candidate-visible fact
set, sources/evidence, response schema, and protected-key exclusion for both
presentations.

The scorer implementation and custody bridge are byte-identical to the passed
prelaunch commit. The independent score was invoked only after capture
verification. It first revalidated complete custody, then opened every
capture-listed `run.json` and `response.json` through no-follow regular-file
descriptors, checked their byte digests and bound fields, reconstructed the
capture root from that buffered snapshot, and only then opened adjudication and
scored the buffered JSON. Thus the immutable-snapshot/pre-key ordering passes;
F06 concerns only the subsequent floating-point serialization.

The reviewer accessed protected adjudication after the capture gate solely for
independent final verification: once through the complete scorer and once in a
diagnostic pass that isolated the five exact-success durations. No provider,
participant, permit, auth, or authority state was touched.

## Checks

The following passed from the fresh detached producer checkout:

- locked event/schema contract tests;
- 12 container-runtime tests;
- 10 confirmatory-custody tests;
- 9 replacement-prelaunch/stopped-registration tests;
- confirmatory custody prelaunch verification;
- benchmark verification;
- 16 benchmark tests;
- all 16 real capture and ingested-run validations;
- exact input, packet, permit, capture-to-run, telemetry, and history checks;
  and
- `git diff --check` from prelaunch through the result.

The exact score-byte comparison is the sole failed gate.
