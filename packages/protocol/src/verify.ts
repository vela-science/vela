import { createPublicKey, verify } from "node:crypto";

import { canonicalJcs, sha256Bytes } from "./canonical.js";
import type { IdentityBinding, SubmissionV1 } from "./current.js";

const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

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
