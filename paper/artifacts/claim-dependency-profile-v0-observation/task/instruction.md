You are the participant in a read-only, context-isolated scientific-state interpretation trial.
Use only regular files mounted under /input and this prompt. Do not access any parent path, Git history, network, user configuration, memory, connector, or authority credential. Do not mutate input, invoke Vela authority, publish, or ask a maintainer for clarification.
Treat every input as synthetic experiment material. A fixture signature, Verification string, profile, reducer status, or lifecycle label does not establish a real Vela Decision, acceptance, Standing, scientific truth, accepted-state Correction, rooted real dependent, or Class E evidence. Missing or unavailable evidence is incomplete, never silently unaffected.
Return exactly one UTF-8 JSON object matching the requested shape, with no Markdown or surrounding prose.

Experiment: synthetic-counterfactual-erdos-321-v0

From the frozen counterfactual transition and declared dependencies, classify only dependent candidates B, D, and E. Report A0 and A1 only as transition endpoints.

Identify exact review_required (affected), unaffected, and incomplete Claims; stale Verifications; target-before-source repair batches; and decisive input evidence for every classification.

Bind each Claim conclusion to label, claim_id, claim_root, repository_id, and repository_origin_root. Bind each stale Verification to verification_id, verification_root, input_claim_root, and claim_label. Cite evidence using an /input-relative path and an RFC 6901 JSON Pointer. Copy the shared does_not_establish list exactly and preserve its order.

Return this exact top-level key set:
{
  "schema": "vela.claim-dependency-participant-answer.v0",
  "experiment_id": "synthetic-counterfactual-erdos-321-v0",
  "repository_id": "...",
  "repository_origin_root": "sha256:...",
  "transition": {
    "kind": "counterfactual_supersession",
    "predecessor": {"claim_id": "...", "claim_root": "sha256:..."},
    "successor": {"claim_id": "...", "claim_root": "sha256:..."}
  },
  "classifications": [
    {
      "label": "...",
      "claim_id": "...",
      "claim_root": "sha256:...",
      "repository_id": "...",
      "repository_origin_root": "sha256:...",
      "status": "review_required|unaffected|incomplete",
      "evidence": [{"path": "...", "pointer": "/..."}]
    }
  ],
  "stale_verifications": [
    {
      "verification_id": "...",
      "verification_root": "sha256:...",
      "input_claim_root": "sha256:...",
      "claim_label": "...",
      "evidence": [{"path": "...", "pointer": "/..."}]
    }
  ],
  "repair_batches": [{"batch": 1, "labels": ["..."]}],
  "authority_effect": "none",
  "does_not_establish": ["..."]
}

Use no additional keys. Sort classifications by label, stale_verifications by verification_id, repair_batches by batch, labels within a batch lexically, and each evidence list by (path, pointer).

The only task inputs are regular files under /input. Write the exact JSON object to /logs/artifacts/answer.json. Your final assistant message must contain the byte-identical JSON object and no other text.
