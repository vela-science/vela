# Dependency notes

## batch-concentration -> method-m3

Kind: `requires_exact`. The concentration calculation used method-m3 exactly.

## release-risk -> batch-concentration

Kind: `requires_result`. The risk method consumes batch-concentration; the new conclusion is unknown until rerun.

## analyzer-service -> method-m3

Kind: `discovery_only`. The method file helped locate the service ticket but supplies no premise for its date.

## blended-release -> batch-concentration

Kind: `requires_result`. The blend consumes batch-concentration.

## blended-release -> batch-c-volume

Kind: `requires_unavailable`. The exact Batch C volume and source binding are unavailable.
