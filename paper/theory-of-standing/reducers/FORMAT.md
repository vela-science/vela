# Proof-history interchange format v1

`theory-of-standing.proof-history.v1` is a deliberately small interchange
format for the Phase III proof artifact. It is not a Vela Protocol schema, a
wire alias, a compatibility format, or a supported API. The canonical Vela
objects and schemas remain unchanged.

Every input is one closed JSON object:

```text
format                   "theory-of-standing.proof-history.v1"
repository               nonnegative integer
authorized_performers    sorted unique nonnegative integers
initial_versions         object from decimal resource id to version
descriptive_dependencies [{dependent, depends_on}]
records                  ordered records
```

Every integer is a nonnegative JSON safe integer, at most
`9007199254740991`, so all three language parsers preserve it exactly.

Records are `submission`, `verification`, or `decision`. A Submission carries
`claim`, `producer`, `scope`, and `authenticated`. A Verification carries
`claim`, `scope`, `property`, and `outcome` (`pass` or `fail`). A Decision
carries `id`, `repository`, `authority_label`, `performer`, `expected_root`, a
resource/version `read_set`, and one action:

```text
accept  {kind, claim}
reject  {kind, claim}
correct {kind, prior_decision, predecessor, replacement}
```

Decision ids are unique within one well-formed proof history. This is a
syntactic interchange constraint, not an additional Lean admission predicate;
a duplicate is rejected as `invalid_format` before replay.

There is no correction consequence field. Admission checks, in order, are:
local Repository, authorized performer, matching authority label, current
root, current read set, action eligibility, and an earlier admitted correction
reference. Authenticated Submissions and matching scoped Verifications advance
the abstract root but never Standing; their invalid counterparts are no-ops.
An admitted Decision appends one Event and advances the root. A rejected
Decision is a state no-op: root, Standing, Events, Submissions, and
Verifications remain unchanged, an observation is appended to `rejections`,
and reduction continues with the next record.

Each emitted Event retains the Decision id, Repository, authority label,
performer, and complete action. The compact output therefore preserves the
attributable correction record as well as the resulting Standing.

Canonical Standing uses only `accepted`, `unassessed`, `superseded`, and
`retracted`. Accepting a correction supersedes its accepted predecessor and
accepts its replacement; every unrelated Claim keeps its prior Standing.

`descriptive_dependencies` is immutable reader input outside Decision
admission. After replay, it produces a separate `reassessment` array. It never
changes root, Event history, admission, or Standing.

Every format-valid replay emits canonical compact JSON with format
`theory-of-standing.proof-result.v2`. Its `rejections` array contains zero or
more `{record_index, code}` observations in record order; `record_index` is
zero-based. The remaining fields contain the final state after the complete
history. Structural parse or validation failure cannot be replayed and instead
emits only `{code: "invalid_format", format:
"theory-of-standing.proof-invalid.v1"}`. Object keys are lexicographically
sorted, arrays have their specified order, and each output ends in one LF byte.
