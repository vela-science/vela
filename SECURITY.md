# Security policy

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/vela-science/vela/security/advisories/new);
do not open a public issue. Include reproduction steps and the affected
component. The repository has private reporting enabled, so the report stays
inside a draft security advisory until disclosure is coordinated.

## What to read first

[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) is the substrate's honest
read on its own attack surface: what is defended, what is not, and what
is deferred. If you think you have found a hole, check whether it is
already named there — several real gaps are documented as accepted or
deferred rather than defended.

## Scope notes

- The trust story never depends on a hosted reader: a consumer's
  `git clone` + `vela replay` verifies scientific-state history locally;
  source-owned pinned methods verify their declared properties. Hub compromise is availability
  and discovery, not integrity (THREAT_MODEL.md, "Reader compromise").
- A human or agent may perform a Decision. The actor identity records
  attribution and grants no authority. Repository policy, the current roots
  and read set, strict replay, and the repository-authority signature govern
  admission. A route that lets actor kind bypass those checks is a
  vulnerability.
- Private keys never belong in a repository. Vela uses the standard OpenSSH
  agent for the repository service identity and does not read private-key
  files. `vela init` scaffolds `/.vela/keys/` in `.gitignore` as defense in
  depth; operators must also keep keys out of every other repository path. A
  reference repository carrying a private key is a vulnerability even if the
  key looks disposable.
