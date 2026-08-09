# Epoch-1 conformance evidence

Measured against the four repositories that predate ADR 0039 and are now
archived. Every byte here is retained exactly as recorded.

`authorization-profile-parity.json` pins those repositories' commits and
their `AuthorizationModelV1` roots, and it carries `frontier_id` and
`"resource_type": "frontier"` because that is the shape the repositories
actually have. It is not migrated: rewriting it to the current spelling would
assert a shape the measured data does not have, and the DSSE signatures over
the Cedar entity token cannot be regenerated.

Its test was removed with the epoch-1 reader. A replacement parity fixture is
measured against `vela-science/math` once that repository has a genesis.
