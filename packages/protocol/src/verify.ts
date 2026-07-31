import { createPublicKey, verify } from "node:crypto";

import { canonicalJcs, sha256Bytes } from "./canonical.js";
import type { IdentityBinding, RequestedChangeV1, SubmissionV1 } from "./current.js";
import { enumAt, exactKeys, objectAt, sha256At, stringAt } from "./validation.js";

const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const CLAIM_ID_RE = /^(?:vcl_[0-9a-f]{64}|vf_[0-9a-f]{16})$/u;

export function validateRequestedChange(value: unknown): asserts value is RequestedChangeV1 {
  const change = objectAt(value, "requested_change");
  const kind = enumAt(change.kind, "requested_change.kind", [
    "add_claim",
    "correct_claim",
    "supersede_claim",
    "retract_claim",
  ] as const);

  if (kind === "add_claim") {
    exactKeys(change, ["kind"], [], "requested_change");
    return;
  }

  exactKeys(change, ["kind", "target"], [], "requested_change");
  const target = objectAt(change.target, "requested_change.target");
  exactKeys(target, ["claim_id", "claim_root"], [], "requested_change.target");
  stringAt(target.claim_id, "requested_change.target.claim_id", {
    min: 19,
    max: 68,
    pattern: CLAIM_ID_RE,
  });
  sha256At(target.claim_root, "requested_change.target.claim_root");
}

export function identityBindingPreimage(binding: IdentityBinding): string {
  return canonicalJcs({ ...binding, binding_id: "", signature: "" });
}

export function submissionPreimage(submission: SubmissionV1): string {
  return canonicalJcs({
    ...submission,
    submission_id: "",
    authentication: { ...submission.authentication, signature: "" },
  });
}

function publicKeyFor(binding: IdentityBinding) {
  const publicBytes = Buffer.from(binding.public_key_hex, "hex");
  if (publicBytes.length !== 32) {
    throw new Error("identity binding public key is not Ed25519");
  }
  return createPublicKey({
    key: Buffer.concat([ED25519_SPKI_PREFIX, publicBytes]),
    format: "der",
    type: "spki",
  });
}

export function verifyIdentityBinding(binding: IdentityBinding): void {
  if (binding.schema !== "vela.identity_binding.v0.1") {
    throw new Error("identity binding schema must be vela.identity_binding.v0.1");
  }
  const publicKey = publicKeyFor(binding);
  const preimage = identityBindingPreimage(binding);
  const expected = `vib_${sha256Bytes(preimage).slice(7, 23)}`;
  if (binding.binding_id !== expected) {
    throw new Error("identity binding id mismatch");
  }
  if (!verify(null, Buffer.from(preimage), publicKey, Buffer.from(binding.signature, "hex"))) {
    throw new Error("identity binding signature does not verify");
  }
}

export function verifySubmission(submission: SubmissionV1): void {
  if (submission.schema !== "vela.submission.v1") {
    throw new Error("Submission schema must be vela.submission.v1");
  }
  validateRequestedChange(submission.requested_change);
  const binding = submission.authentication.identity_binding;
  verifyIdentityBinding(binding);
  if (
    binding.actor_class !== "agent" ||
    binding.actor_id !== submission.provenance.producer
  ) {
    throw new Error("Submission producer does not match its agent identity binding");
  }
  const preimage = submissionPreimage(submission);
  const expected = `vsb_${sha256Bytes(preimage).slice(7, 23)}`;
  if (submission.submission_id !== expected) {
    throw new Error("Submission id mismatch");
  }
  if (
    !verify(
      null,
      Buffer.from(preimage),
      publicKeyFor(binding),
      Buffer.from(submission.authentication.signature, "hex"),
    )
  ) {
    throw new Error("Submission signature does not verify");
  }
}
