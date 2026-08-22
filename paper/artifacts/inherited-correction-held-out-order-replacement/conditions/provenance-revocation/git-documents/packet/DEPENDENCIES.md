# Dependency notes

## north-estimate -> lineage-r1

Kind: `requires_exact`. The estimate used the exact predecessor multiplier and must be recomputed if that input is superseded.

## regional-sensitivity -> north-estimate

Kind: `requires_result`. The sensitivity method consumes north-estimate and must be rerun after that estimate changes.

## archive-accession -> lineage-r1

Kind: `discovery_only`. The predecessor lineage helped locate the archive record but supplies no premise for its date.

## combined-estimate -> north-estimate

Kind: `requires_result`. The combined estimate consumes north-estimate.

## combined-estimate -> south-estimate

Kind: `requires_unavailable`. The exact south-region premise and source binding are absent from the bounded packet.
