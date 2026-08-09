# Epoch-1 conformance evidence

Measured against the four repositories that predate ADR 0039 and are now
archived. Every byte here is retained exactly as recorded.

`authorization-profile-parity.json` pins those repositories' commits and
their `AuthorizationModelV1` roots, and it carries `frontier_id` and
`"resource_type": "frontier"` because that is the shape the repositories
actually have. It is not migrated: rewriting it to the current spelling would
assert a shape the measured data does not have, and the DSSE signatures over
the Cedar entity token cannot be regenerated.

`crates/vela-authority/tests/authorization_profile_parity.rs` reads this
retained corpus. It deterministically translates the retired identifier
spelling, reproduces all seven published Allows with the current closed
evaluator, and checks seven negative boundary cases for their exact reasons.
It does not claim that roots survive the vocabulary and UUID migration.
