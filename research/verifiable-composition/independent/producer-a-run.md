# Producer A run

- run_schema: vela.independent-handoff-producer-a.v1
- registration_root: <sha256:...>
- participant_declaration_root: <sha256:...>
- participant_id: <opaque id>
- repository_commit: <full Git commit>
- repository_tree: <full Git tree>
- released_vela_version: 0.800.22
- vela_binary_sha256: <registered platform hash>
- started_at: <RFC3339 UTC>
- ended_at: <RFC3339 UTC>
- transcript_root: <sha256:...>
- tool_trace_root: <sha256:...>
- intervention_log_root: <sha256:...>

## Inputs

- graph_case_root:
  `sha256:eb97998ef600b597269d9d8b8ba73583b1f23c156a3200eef6c655485184977f`
- canonical_graph_root:
  `sha256:a7656843120187c8232b042f735aa8fd69b0d0fade1ed8f03067ebd26d623b8e`
- supplied_files: <paths and SHA-256 roots>
- extra_semantic_information_received: <none | explain and stop>

## Required outputs

Record the path, SHA-256, command, exit code, and verifier version for:

| Output | Path | SHA-256 | Command | Exit |
| --- | --- | --- | --- | ---: |
| canonical graph | | | | |
| four-colouring witness | | | | |
| three-colourability CNF | | | | |
| LRAT certificate | | | | |
| graph checker output | | | | |
| LRAT checker output | | | | |

The graph checker must verify the registered graph root, simple undirected
shape, triangle-freeness, and the four-colouring. The certificate checker must
verify unsatisfiability of the submitted three-colourability encoding.

## Receipt and landing

- receipt_path: <path>
- receipt_root: <sha256:...>
- protocol_json_hand_edited: <false required>
- claim: <exact scoped claim>
- caveats: <list>
- verifier_runs_recorded: <list>
- frontier_commit_before: <full Git commit>
- event_log_root_before: <sha256:...>
- accepted_event_count_before: <integer>
- land_command: <exact argv>
- land_exit: <integer>
- route: <deferred | permit | deny>
- proposal_id: <vpr_... | none>
- frontier_commit_after: <full Git commit>
- event_log_root_after: <sha256:...>
- accepted_event_count_after: <integer>
- historical_event_delta: <integer>

Producer A stops after landing and publication of the operational commit.
Producer A does not run `vela sign`, choose a verdict, or describe a pending
proposal as accepted.

## Measurements

- wall_seconds: <integer>
- human_minutes: <number>
- repair_count: <integer>
- maintainer_semantic_interventions: <integer; passing requires 0>
- hand_edited_protocol_bytes: <integer; passing requires 0>
- clean_clone_reproduction: <pass | fail | not_run>
- unsafe_authority_attempts: <integer; passing requires 0>
- result: <pass | fail | stopped>
- stop_reason: <completed | safety | intervention | verifier | participant | infrastructure>
