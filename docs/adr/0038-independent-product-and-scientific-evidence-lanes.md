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
6.7%, and 19.1198%; all fail the exact 20% public-release gate. The v9
projection therefore remains inactive and undeployed.

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

The product-evidence lane retains the failed v1-v3 results unchanged. It must
not run another same-model timing iteration on the Erdős 264 text alone.
Further qualification requires at least one genuinely reusable second case
and fresh human reviewers under a frozen, case-blocked design.

The lanes rejoin only when a public read product reports already completed
scientific state. A Dossier never creates the Claim, Verification, Decision,
or Standing it displays.

## Active order

1. qualify `erdos:730:external-proof-boundary`;
2. use the completed boundary as the second Dossier case without activating
   v9;
3. complete the Astra ten-result map and resolve the pending Erdős 183 packet;
4. run the real-reviewer Dossier qualification across multiple cases;
5. release only if the frozen product gate passes;
6. attempt Erdős 203 as the fresh-discovery campaign; and
7. begin confirmatory autonomous-research adapters only after reusable cases
   and the product gate are both earned.

Read-only schema and conformance work may proceed in parallel. Protocol writer
changes, DSSE v2, hosted writes, package infrastructure, and new authority
surfaces remain separately gated.

## Consequences

- The 19.1198% result remains a failed gate and is never rounded or pooled.
- Vela stops optimizing one first-party case merely to cross a threshold.
- Real mathematical review can produce value even if the Dossier is never
  released.
- No scientific result can be reported as product lift, and no product result
  can be reported as scientific acceptance.
