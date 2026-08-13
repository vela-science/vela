# Read-surface conformance fixtures

`decision-inbox-v3.json` freezes the exact JSON envelope emitted by
`vela review inbox --json` for one deterministic, protocol-ready fixture. The
checked-in bytes are generated from the live Rust types and compared on every
`vela-cli` test run.

The fixture qualifies `vela.decision-inbox.v3` as a stable read contract. It is
not a protocol object, a Decision, a recommendation, or retained Standing. Its
`entry_root` and `projection_root` use the domains implemented in
`crates/vela-cli/src/decision_inbox.rs`; the outer `ok` and `command` fields are
CLI envelope fields and are not part of `projection_root`.

Consumers must require the exact supported schema, keep entry, Repository, and
projection roots distinct, and treat `if_accept` and `if_reject` as
hypothetical. Adding a field to this v2 read surface is compatible for readers
that ignore unknown fields. Removing, renaming, or changing the type or meaning
of a field requires a new schema version and a parallel fixture.
