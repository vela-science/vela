# Product-compression v1 archive

This directory preserves the frozen inputs and stop record for the first
product-compression attempt. It is historical evidence, not an active benchmark
harness or compatibility target.

The run stopped without product-lift credit. Its custom validator, scorer,
reporter, and tests were retired after Harbor became the sole execution and raw
result layer. Their exact source hashes remain recorded in `plan.v1.json` and
`plan.v2.json`; retaining executable copies would incorrectly present the
superseded framework as supported code.

Preserved records:

- `plan.v1.json`: original frozen plan, root
  `sha256:8ec3c02fce995ce8ee046844fcc40eefd37787c7e4d695ef794f0715b92ae1ef`;
- `plan.v2.json`: bounded pre-output amendment linked to the original plan;
- `stop.v1.json`: the honest terminal classification and evidence roots;
- `answer.schema.json` and `answer-key.v1.json`: exact historical contracts.

The stop remains authoritative for interpretation: this attempt does not show
product lift, independent use, adoption, or scientific acceptance.
