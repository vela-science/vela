# Security policy

## Supported scope

Security reports for the public v0 release should focus on:

- `crates/vela-protocol/`
- proof packet export and validation
- MCP/HTTP serving
- install and release scripts
- schema and artifact validation

Roadmap-only runtime, Hub, desktop, and federation surfaces are outside the v0
release security scope unless a release explicitly includes them.

## Reporting

Please do not open public issues for vulnerabilities. Email the maintainer or
use GitHub private vulnerability reporting when enabled.

Include:

- affected command, route, or file path
- reproduction steps
- expected impact
- whether credentials or private artifacts are involved

Do not include real secrets, tokens, private papers, or unpublished research data
in reports.
