# Security policy

## Reporting a vulnerability

Email the maintainer (Will Blair, william.blair0708@gmail.com); do not
open a public issue. Include reproduction steps and the affected
component. You will get an acknowledgment, and a fix or an honest
assessment before any public disclosure.

## What to read first

[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) is the substrate's honest
read on its own attack surface: what is defended, what is not, and what
is deferred. If you think you have found a hole, check whether it is
already named there — several real gaps are documented as accepted or
deferred rather than defended.

## Scope notes

- The trust story never depends on a hosted reader: a consumer's
  `git clone` + `vela replay` verifies the log and `vela reproduce`
  verifies the science, both locally. Hub compromise is availability
  and discovery, not integrity (THREAT_MODEL.md, "Reader compromise").
- No AI or agent identity sits in any trust path; the engine refuses
  `agent:`/`ci:` actors on every decision verb. A bypass of that
  refusal is a vulnerability — report it.
- Private keys never belong in a repo. `vela init` scaffolds a
  `.gitignore` that covers `/.vela/keys/` and nothing else
  (`crates/vela-cli/src/current_init.rs`), so a key kept anywhere but
  that directory is yours to ignore. This line used to promise `keys/`
  and `*.key` as well; neither rule has ever been written by the
  scaffold, and a reader who trusted it would have committed the key
  (THREAT_MODEL.md, "Key compromise"). A reference repository carrying
  a private key is a vulnerability even if the key looks disposable.
