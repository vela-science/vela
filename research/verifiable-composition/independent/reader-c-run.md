# Reader C run

- run_schema: vela.independent-handoff-reader-c.v1
- registration_root: <sha256:...>
- participant_declaration_root: <sha256:...>
- participant_id: <opaque id>
- implementation_repository: <URL or bundle root>
- implementation_commit: <full Git commit>
- implementation_sha256: <sha256:...>
- language_and_version: <text>
- direct_dependencies: <list>
- vela_source_imports: <none required>
- reference_reader_imports: <none required>
- started_at: <RFC3339 UTC>
- ended_at: <RFC3339 UTC>
- intervention_log_root: <sha256:...>

Reader C receives the written fact-manifest and dependency-standing profile,
the registered vectors, and bounded input files. Reader C does not receive the
reference reader source.

## Required behavior

The implementation must:

- reject duplicate JSON names, unknown fields, unsafe numeric values, malformed
  roots, and oversized or non-regular inputs;
- derive the fact-manifest and dependency-observation roots;
- distinguish same, descendant, stale, and forked delivery;
- return one registered dependency status and code;
- keep `child_truth` equal to `not_assessed`; and
- perform no write, network lookup, or authority action.

## Parity record

| Vector | Reader C status/code | Reference status/code | Equal |
| --- | --- | --- | --- |
| satisfied | | | |
| correction | | | |
| supersession | | | |
| withdrawal | | | |
| decision revocation | | | |
| verifier revocation | | | |
| stale root | | | |
| fork | | | |
| missing bytes | | | |
| wrong revision root | | | |
| short-ID collision | | | |
| map-order variation | | | |

- registered_vectors_total: <integer>
- parity_count: <integer>
- unregistered_special_cases_added: <integer; expected 0 before scoring>
- network_attempts: <integer; expected 0>
- write_attempts: <integer; expected 0>
- authority_attempts: <integer; expected 0>
- wall_seconds: <integer>
- maintainer_semantic_interventions: <integer; passing requires 0>
- result: <pass | fail | stopped>
- stop_reason: <completed | parity | intervention | participant | infrastructure>
