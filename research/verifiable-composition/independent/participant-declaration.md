# Independent participant declaration

- packet_schema: vela.independent-handoff-participant.v1
- registration_root: <sha256:...>
- participant_id: <opaque public id>
- role: <producer_a | verifier_v1 | verifier_v2 | human_steward | producer_b | reader_c | red_team | baseline_team>
- organization: <name>
- repository: <public URL or retained bundle root>
- date: <YYYY-MM-DD>
- human_or_model: <human | model-assisted human | autonomous model>
- toolchain: <versions and hashes>
- implementation_lineage: <repositories, libraries, or prior code reused>
- prior_vela_contributions: <none | list>
- prior_canopus_contributions: <none | list>
- prior_access_to_hidden_fixture_answers: <none | explain>
- relationship_to_protocol_team: <none | explain>
- relationship_to_other_participants: <none | explain>
- financial_or_employment_conflict: <none | explain>
- private_key_custody: <not_applicable | human_local_only>
- declaration_signature_or_attestation: <method and root>

## Eligibility

- outside the Vela project: <true | false>
- controls a separate repository or implementation: <true | false>
- received only the registered packet and allowed support: <true | false>
- can publish commands, roots, failures, and interventions: <true | false>
- eligible for independent credit: <true | false>
- ineligibility_reason: <none | text>

The protocol team grades eligibility before the participant starts. A fresh
model session run by the protocol team remains first-party even when it uses a
new conversation or worktree.
