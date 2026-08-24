# Dependency notes

## aggregate-e-requires-site-q

Kind: `requires_unavailable`. Source: `aggregate-e`. Target: `site-q-yield`.

The Site Q source and content binding are absent from the bounded packet.

## aggregate-e-requires-yield-b

Kind: `requires_result`. Source: `aggregate-e`. Target: `yield-b`.

aggregate-e consumes the corrected Site P yield and an exact Site Q input.

## installation-d-discovered-with-calibration-a-v1

Kind: `discovery_only`. Source: `installation-d`. Target: `calibration-a-v1`.

The calibration file helped locate the installation record but supplies no premise for its date.

## stability-c-requires-yield-b

Kind: `requires_result`. Source: `stability-c`. Target: `yield-b`.

stability-c consumes yield-b, but its retained method does not determine the post-correction outcome without a rerun.

## yield-b-requires-calibration-a-v1

Kind: `requires_exact`. Source: `yield-b`. Target: `calibration-a-v1`.

yield-b used the exact predecessor scale factor and must be recalculated when that input is superseded.
