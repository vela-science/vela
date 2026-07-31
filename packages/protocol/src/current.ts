export interface IdentityBinding {
  schema: "vela.identity_binding.v0.1";
  binding_id: string;
  actor_id: string;
  actor_class: "human" | "agent" | "org";
  public_key_hex: string;
  created_at: string;
  signature: string;
}

export interface ExecutionBindingV1 {
  schema: "vela.execution-binding.v1";
  packet_root: string;
  profile_root: string;
  verifier_capsule_root: string;
  result_contract_root: string;
}

export type RequestedChangeV1 =
  | { kind: "add_claim" }
  | {
      kind: "correct_claim" | "supersede_claim" | "retract_claim";
      target: { claim_id: string; claim_root: string };
    };

export interface SubmissionV1 {
  schema: "vela.submission.v1";
  submission_id: string;
  claim: {
    assertion: string;
    type: "computational" | "theoretical" | "empirical" | "negative" | "contradiction";
    conditions: string[];
  };
  artifacts: Array<{ kind: string; path: string; digest: string }>;
  caveats: string[];
  replayability: "exact" | "bounded" | "approximate" | "unavailable" | "unknown";
  producer_checks: Array<{
    method: string;
    outcome: "pass" | "fail" | "error" | "skipped" | "unknown";
    authority: "producer_reported";
  }>;
  verification_requirements: string[];
  requested_change: RequestedChangeV1;
  provenance: {
    producer: string;
    source_system: string;
    source_attempt?: string;
    source_run?: string;
    emitted_at: string;
  };
  execution_binding?: ExecutionBindingV1;
  authentication: {
    algorithm: "ed25519";
    identity_binding: IdentityBinding;
    signature: string;
  };
}

export interface VerificationRecordV1 {
  schema: "vela.verification-record.v1";
  verification_record_id: string;
  subject: {
    claim_id: string;
    artifact_ids: string[];
    submission_id: string;
    submission_root: string;
    proposal_id: string;
  };
  method: {
    profile: string;
    implementation: string;
    environment_root: string;
  };
  scope: {
    property: string;
    does_not_establish: string[];
  };
  outcome: "pass" | "fail" | "error" | "inconclusive";
  verifier: string;
  independence: {
    declared_independent_of: string[];
    shared_dependencies: string[];
  };
  output_artifact_ids: string[];
  started_at: string;
  completed_at: string;
  authentication: {
    algorithm: "ed25519";
    identity_binding: IdentityBinding;
    signature: string;
  };
}
