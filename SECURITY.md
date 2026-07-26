# Security

Report vulnerabilities privately through GitHub Security Advisories for
`vela-science/vela-research-harness`.

Canopus does not accept human signing keys. Do not place a human Vela key,
provider credential, or unpublished scientific secret inside a run root.
Successful runs scrub the isolated agent-key home and retain no producer
private key or withdrawal capability. Operators remain responsible for
deleting failed diagnostic runs and benchmark output.

Agent-authored Git commits and tags must explicitly disable ambient signing
(`commit.gpgSign=false`, `tag.gpgSign=false`) and verify that the resulting Git
objects contain no signature. A repository-local configuration now disables
both for this working copy. The withdrawn `v0.1.0` tag and some earlier
pre-release implementation commits retain unintended human SSH signatures from
ambient Git configuration as transparent historical evidence. Those
signatures are not Vela authority, but agents must never create another one.

The v0 isolation backend is macOS Seatbelt. Other platforms fail closed; they
do not receive a permissive fallback.

Mission v1 uses the exact native Codex CLI with a bundled default-deny Seatbelt
profile on macOS or Bubblewrap profile on Linux and WSL2. Only the target packet
is writable and command-readable.
The runtime authentication copy, sealed source checkout, Vela home, host home,
host canary, and unrelated repositories must remain outside that boundary.
Every run performs a deterministic custody preflight; the live hostile fixture
also proves shell execution while denying credential reads, command network,
outside writes, and auth-bearing process environment.

The verifier is a separate digest- and platform-pinned Linux container with no
network and no writable persistent mount. Its manifest records Docker, the
read-only root, exact read-only binds, dropped capabilities, and denied
privilege escalation instead of mislabeling the boundary as macOS Seatbelt.
Canopus never places the producer, verifier, or model in the Vela signing path.
