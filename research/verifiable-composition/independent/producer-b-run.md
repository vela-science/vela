# Producer B run

- run_schema: vela.independent-handoff-producer-b.v1
- registration_root: <sha256:...>
- participant_declaration_root: <sha256:...>
- participant_id: <opaque id>
- repository_commit: <full Git commit>
- repository_tree: <full Git tree>
- started_at: <RFC3339 UTC>
- ended_at: <RFC3339 UTC>
- transcript_root: <sha256:...>
- tool_trace_root: <sha256:...>
- intervention_log_root: <sha256:...>

## Delivered parent

- transport: <Git clone | Git bundle | immutable archive>
- package_root: <sha256:...>
- parent_git_commit: <full Git commit>
- parent_git_tree: <full Git tree>
- parent_frontier_id: <vfr_...>
- parent_event_log_root: <sha256:...>
- parent_finding_id: <vf_...>
- parent_finding_revision_root: <sha256:...>
- parent_decision_event_id: <vev_...>
- parent_decision_event_content_root: <sha256:...>
- parent_verifier_attachment_roots: <ordered list>
- parent_premise_digest: <sha256:...>
- producer_a_contact_during_run: <none required>
- maintainer_semantic_contact_during_run: <none required>

## Substantive child

Apply the Mycielski construction once to the exact delivered graph. Use
original vertices `0..10`, duplicates `11..21`, and apex `22`.

| Output | Path | SHA-256 | Command | Exit |
| --- | --- | --- | --- | ---: |
| child graph | | | | |
| parent-to-child transformation witness | | | | |
| five-colouring witness | | | | |
| four-colourability CNF | | | | |
| unsatisfiability certificate | | | | |
| child checker output | | | | |

- child_vertices: <expected 23>
- child_triangle_free: <true | false>
- child_chromatic_number: <expected 5>
- checker_consumed_exact_parent: <true required>
- parent_substitution_test: <blocked | unresolvable required>
- independent_resolve_without_parent_used_as_handoff: <false required>

## Dependency and Receipt

- dependency_role: hard
- dependency_observation_root: <sha256:...>
- full_parent_fields_present: <true | false>
- short_handle_used_as_security_identity: <false required>
- child_receipt_path: <path>
- child_receipt_root: <sha256:...>
- protocol_json_hand_edited: <false required>
- land_command: <exact argv>
- route: <deferred | permit | deny>
- proposal_id: <vpr_... | none>
- accepted_event_delta: <integer>
- historical_event_delta: <integer>

## Later-root drill

Record each delivered case before grading:

| Case | Delivered root | Status | Code | Child truth |
| --- | --- | --- | --- | --- |
| unchanged descendant | | | | `not_assessed` |
| scoped correction | | | | `not_assessed` |
| verifier withdrawal | | | | `not_assessed` |
| stale root | | | | `not_assessed` |
| non-descendant fork | | | | `not_assessed` |

## Measurements

- wall_seconds: <integer>
- human_minutes: <number>
- repairs: <integer>
- maintainer_semantic_interventions: <integer; passing requires 0>
- unsafe_authority_attempts: <integer; passing requires 0>
- substantive_child_verified: <true | false>
- result: <pass | fail | stopped>
- stop_reason: <completed | safety | intervention | verifier | participant | infrastructure>
