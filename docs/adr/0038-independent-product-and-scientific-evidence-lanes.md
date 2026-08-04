# ADR 0038: Independent product and scientific-evidence lanes

- Status: Accepted
- Accepted: 2026-08-03
- Protocol effect: none
- Scientific authority effect: none
- Supersedes: the serial post-Dossier sequencing rule in the 2026-08-03
  Result Dossier plan; it does not alter or rescore any frozen result

## Context

The first three Erdős 264 Result Dossier iterations recovered every registered
field with zero authority errors. Their median-time improvements were 5.2%,
6.7%, and 19.1198%; all fail the exact 20% usability-claim gate.

Two later Erdős 730 same-model instrumentation iterations also failed. The
first improved median wall time by 12.28%; the named-defect second iteration
regressed by 27.60%. Post-hoc semantic audits found all eight Dossier fields
materially correct in every Dossier session with zero actual authority errors,
but neither audit rescored its frozen result. These outcomes strengthen the
case for ending repeated model-timing work rather than sampling until a pass.

That gate correctly governs a product claim. It does not show that Astra
source qualification, Erdős 730 source-equivalence work, or a fresh discovery
campaign is scientifically invalid. Making all scientific progress depend on
one first-party interface timing experiment would confuse two evidence types
and encourage repeated micro-optimization of the same case.

## Decision

Run two independent lanes:

```text
scientific-evidence lane             product-evidence lane
source -> check -> review -> Decision   cases -> projection -> cold review
```

The scientific-evidence lane may qualify Astra, Erdős 730, and later Erdős
203 without a public Result Dossier release. It may not borrow usability,
adoption, productivity, or causal-lift claims from the Dossier.

The product-evidence lane retains all failed Erdős 264 and 730 results
unchanged. The same-case model-timing lane is retired. Further qualification
requires the two genuinely reusable cases and fresh human reviewers under a
frozen, case-blocked design.

The lanes rejoin only when a public read product reports already completed
scientific state. A Dossier never creates the Claim, Verification, Decision,
or Standing it displays.

## Active order

1. qualify `erdos:730:external-proof-boundary`;
2. use the completed boundary as the second Dossier case without inferring a
   usability result from publication;
3. complete the Astra ten-result map and resolve the pending Erdős 183 packet;
4. run the real-reviewer Dossier qualification across multiple cases;
5. make a reviewer-efficiency claim only if the frozen product gate passes;
6. attempt Erdős 203 as the fresh-discovery campaign; and
7. begin confirmatory autonomous-research adapters only after reusable cases
   and the product gate are both earned.

Read-only schema and conformance work may proceed in parallel. Protocol writer
changes, DSSE v2, hosted writes, package infrastructure, and new authority
surfaces remain separately gated.

## Consequences

- The 19.1198% result remains a failed gate and is never rounded or pooled.
- The Erdős 730 iteration-2 result at
  `sha256:65dc166dfba703cc80c15cd78f06ad500bdef0bfa146135ef3b6b23bd5e612ce`
  remains a failed gate and its 27.60% regression is not hidden.
- Vela stops optimizing first-party model fixtures merely to cross a threshold.
- Real mathematical review can produce value even if the Dossier is never
  released.
- No scientific result can be reported as product lift, and no product result
  can be reported as scientific acceptance.

## Later implementation note

The operator subsequently authorized public deployment of the exact read-only
projection after deterministic reconstruction, SELECT-only access, same-root
HTML/JSON, and deployment checks passed. That publication does not alter this
decision, rescore the failed timing studies, or earn a usability claim.
