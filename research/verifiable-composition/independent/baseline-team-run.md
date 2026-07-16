# Standards baseline team run

- run_schema: vela.independent-handoff-baseline-team.v1
- registration_root: <sha256:...>
- participant_declaration_root: <sha256:...>
- participant_id: <opaque id>
- implementation_repository: <URL or bundle root>
- implementation_commit: <full Git commit>
- started_at: <RFC3339 UTC>
- ended_at: <RFC3339 UTC>
- intervention_log_root: <sha256:...>

The baseline team receives the same graph bytes, scientific tasks, authority
facts, later-root cases, time limits, and support policy as the Vela arm.

## Required profile

The implementation uses:

- ordinary Git commits, trees, bundles, and ancestry;
- one DSSE-wrapped in-toto Statement carrying the canonical fact manifest;
- one signed exact `science.lock`;
- a documented scoped authority rule; and
- a deterministic dependency-standing reducer.

The baseline may add OCI descriptors or TUF metadata only for a registered
threat case that the smaller profile cannot handle. Record the reason and added
implementation cost.

## Scientific and custody results

- producer_a_outputs_equal: <true | false>
- producer_b_task_equal: <true | false>
- human_custody_rule_equal: <true | false>
- correction_cases_equal: <true | false>
- offline_completion: <true | false>
- hidden_vela_semantics: <none | explain and stop>
- unsafe_authority_attempts: <integer; passing requires 0>
- historical_rewrites: <integer; passing requires 0>
- false_strict_or_status_passes: <integer; passing requires 0>

## Measurements

- implementation_lines: <integer>
- direct_dependencies: <integer and list>
- configuration_bytes: <integer>
- packet_bytes: <integer>
- producer_a_minutes: <number>
- producer_b_minutes: <number>
- reader_minutes: <number>
- steward_minutes: <number>
- repairs: <integer>
- maintainer_semantic_interventions: <integer; passing requires 0>
- user_steps: <integer>
- recorded_errors: <integer>
- result: <pass | fail | stopped>
- stop_reason: <completed | safety | semantic_gap | intervention | participant | infrastructure>

The final comparison uses equal semantics. A smaller packet that omits scoped
authority, correction handling, full dependency identity, or human custody
does not qualify as a baseline completion.
