# Research-program loop

`vela integration program <INPUT>` is a read-only, local projection for one
bounded research program. It closes a continuity gap between source-owned
activity and Vela's existing scientific-state boundary without turning tasks,
hypotheses, experiments, or next actions into Protocol 1 objects.

## Why one internal input is necessary

Current Vela objects already retain the part of a result that can change
Standing: authenticated Submission, scoped Verification, attributed Decision,
and deterministic replay. The supplied longitudinal corpus exposed a different
missing field set:

- mutable task observation time and callback deadline;
- an explicit distinction between a stale task and completed or stopped work;
- proposed ledger or hypothesis-register edits that have not been accepted;
- a ranked next action over unresolved evidence and supervision gaps.

Those fields cannot be added to Submission, Verification, Decision, Standing,
the repository profile, or the repository projection without putting native
research activity and planning inside the protocol. They also cannot be
recovered reliably from prose. The command therefore reads one closed
`vela.cli.research-program-input.v1` document. This is an internal CLI input,
not a portable protocol object, published schema, authority record, workflow
engine, or promise of cross-domain ontology stability.

The input points to source-owned files by path and SHA-256. It carries only the
small task-status overlay and explicit proposed changes that the existing
sources do not contain. Raw traces, prompts, credentials, and private artifact
bytes stay in their owning systems.

## Contract

The command:

1. verifies every declared source file against its expected SHA-256;
2. reads the existing H-112 episode-denominator shape rather than recoding its
   campaign rows;
3. keeps scientific results, software qualifications, stopped work, invalid
   experiments, bounded nulls, and accepted Standing on separate axes;
4. checks available file evidence against its retained reconstruction digest
   and reports missing, changed, or unsupported pointers;
5. parses the exact-hash-bound manual hypothesis register only as current
   source-owned activity state;
6. derives stalled work and missing callbacks from explicit timestamps;
7. emits proposed ledger/register changes without applying them; and
8. emits the lowest-numbered declared next action whose required proposal IDs
   survived validation, with exact source references for its rationale.

The output has `authority_effect: "none"`, `standing_effect: "none"`, and
`accepted_standing_changes: 0`. A proposed `accepted` state is rejected unless
the proposal carries an exact Vela Decision reference. Even with that
reference, the command reports the proposal; it does not admit an Event or
change Standing. Authoritative acceptance remains `vela review accept` or
`vela review reject` followed by replay.

## Intended use

The input and output are metadata projections. A workbench or source repository
may regenerate them whenever tasks or evidence change. Consumers should bind
the output to its `input_root`, `source` digests, and `as_of` instant. A stale
projection is evidence of a missed refresh, never accepted state.

This first slice is deliberately local. It does not monitor tasks, contact
workers, edit source ledgers, schedule experiments, or publish a program view.
