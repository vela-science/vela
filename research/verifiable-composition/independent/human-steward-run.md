# Human steward run

- run_schema: vela.independent-handoff-human-steward.v1
- registration_root: <sha256:...>
- participant_declaration_root: <sha256:...>
- steward_id: <reviewer:...>
- frontier_commit_before: <full Git commit>
- event_log_root_before: <sha256:...>
- proposal_id: <vpr_...>
- proposal_root: <sha256:...>
- decision_facts_root: <sha256:...>
- verifier_v1_root: <sha256:...>
- verifier_v2_root: <sha256:...>
- started_at: <RFC3339 UTC>
- ended_at: <RFC3339 UTC>
- model_or_runner_received_key_bytes: <false required>
- key_access_observed: <none | one_human_ceremony>

The human steward works in a local terminal outside model, runner, MCP, and
browser processes. The worksheet does not choose a verdict.

## Review

Record the steward's own answers before the ceremony:

1. What exact graph claim would the proposal add?

<answer>

2. Which artifacts establish triangle-freeness and chromatic number four?

<answer>

3. Did two verifier implementations reproduce the exact registered graph and
certificate?

<answer>

4. What will acceptance change, and what will it leave unchanged?

<answer>

5. How can a later correction change dependency standing without rewriting the
child?

<answer>

## Decision record

- complete_brief_rendered: <true | false>
- critical_warnings_reviewed: <true | false>
- verdict: <accept | reject | skip | no_decision>
- reason: <human words>
- decision_plan_root: <sha256:... | none>
- decision_event_id: <vev_... | none>
- decision_event_content_root: <sha256:... | none>
- frontier_commit_after: <full Git commit | unchanged>
- event_log_root_after: <sha256:... | unchanged>
- publication_state: <published | local_only | no_decision>
- authority_attempts_by_nonhuman_processes: <integer; required 0>

The steward stops if any process other than the human terminal requests or
receives the key. A preview, Git commit, verifier result, or pending proposal
does not count as acceptance.
