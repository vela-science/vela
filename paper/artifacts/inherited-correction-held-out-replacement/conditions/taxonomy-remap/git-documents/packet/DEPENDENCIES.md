# Dependency notes

## cohort-count -> taxonomy-t7

Kind: `requires_exact`. The cohort count used the predecessor category map exactly.

## trend-model -> cohort-count

Kind: `requires_result`. The trend model consumes cohort-count and must be rerun after a corrected count.

## freezer-commissioning -> taxonomy-t7

Kind: `discovery_only`. The taxonomy map helped locate the commissioning record but is not a premise for the date.

## combined-incidence -> cohort-count

Kind: `requires_result`. Combined incidence consumes cohort-count.

## combined-incidence -> cohort-d-denominator

Kind: `requires_unavailable`. The exact cohort D denominator and binding are absent.
