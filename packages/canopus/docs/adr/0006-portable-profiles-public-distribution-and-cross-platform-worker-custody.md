# ADR 0006: Portable profiles, public distribution, and cross-platform worker custody

- Status: Accepted
- Release gate: satisfied 2026-07-19 by the quantum Deferred landing and
  clean-clone replay, macOS/Ubuntu/WSL2 hostile custody fixtures,
  multi-architecture verifier and installed-package checks, and the exact
  GitHub Actions OIDC publisher bound to the public npm package
- Target: Canopus `v0.4.3` (`v0.4.0` and `v0.4.1` stopped before publication;
  `v0.4.2` was the first provenance-backed public release)
- Later simplification: accepted ADR 0009 removes `withdraw` and retained
  producer keys from Canopus `v0.7.0`; this ADR's released custody evidence
  remains historical

## Context

Canopus 0.3 completed one real authority-free Erdős loop but remains a tagged
source repository rather than an installable product. Its profile reader is
hard-coded to two Erdős ranges, its capsule is Linux arm64 only, and its native
worker custody profile is macOS-only.

Canopus must stay outside the Vela protocol repository. Its job is to compile a
bounded research plan into projected context, a constrained worker run, frozen
artifacts, and a Receipt proposal. Vela alone decides what the result may
change.

## Decision

1. Publish `@vela-science/canopus` through npm trusted publishing with
   provenance and a matching GitHub release.
2. Introduce closed `canopus.profile.v2`, binding the target/packet match,
   objective and stop condition, allowed artifacts, budgets, per-platform
   capsule roots, verifier image index digest, replay argv, and landing ceiling.
   Profile v1 remains read-only replay input.
3. Add advanced `profile list`, `profile show`, `profile validate`, and
   `profile pack`. The released ordinary interface was `doctor`, `run`,
   `inspect`, `replay`, and `withdraw`; ADR 0009 removes the last command from
   new versions.
4. Keep the worker native: macOS uses the proven Seatbelt profile; Ubuntu and
   WSL2 use Codex Bubblewrap with read-only host defaults and explicit writable,
   user, PID, and network namespaces. Native Windows supports install,
   diagnosis, inspection, and replay; tool-using runs use WSL2 until an
   equivalent native custody fixture passes.
5. Keep verification in a distinct multi-architecture, network-denied,
   read-only container. The producer cannot invoke it as an oracle.
6. Bind the exact Linux verifier platform as well as the image digest. Older
   Mission v1 records without that optional field replay unchanged; every new
   profile resolves it explicitly. A verifier manifest for Mission v1 reports
   the actual Docker, read-only-root, exact-bind, dropped-capability boundary
   rather than reusing the historical Mission v0 macOS-Seatbelt description.

No platform may fall back to an unsandboxed worker. The same hostile fixture
must prove authentication, human keys, host canaries, unrelated repositories,
outside writes, command network, and verifier access unavailable before a real
mission.

Hosted Linux CI additionally runs the real Codex Bubblewrap sandbox without a
model call or credential. This deterministic preflight proves that the pinned
binary and permission profile hide source and runtime authentication canaries,
sealed inputs, unrelated files, and host canaries; deny outside writes and
command network; and scrub authentication-shaped environment state. It does
not replace or earn credit for the model-mediated hostile custody fixture.
The live fixture must run the same deterministic boundary before reading the
staged Codex authentication file or making a model call. A nested container or
VM that cannot install Codex's seccomp filter is unsupported and must stop with
an exact native-Ubuntu/WSL2 recovery action; outer privilege is not a permitted
substitute for the missing inner boundary. The fixture independently verifies
an exact generated workspace sentinel and records only structural event counts,
boolean verdicts, and content hashes on failure.

The first-party model-mediated Ubuntu x86-64 fixture passed on a disposable
full-system QEMU guest at candidate commit
`ab132389bdf0522fc257ee81a9d1cc527052b31a`; its bounded evidence is recorded
in
`benchmarks/results/hostile-native-custody-ubuntu-x86_64-2026-07-19/result.json`.
The first-party model-mediated WSL2 x86-64 fixture subsequently passed on a
disposable Windows 11 24H2 guest at candidate commit
`cef2bf740d57aef31238ec112dbe48d98c33cee2`; its bounded evidence is recorded
in
`benchmarks/results/hostile-native-custody-wsl2-x86_64-2026-07-19/result.json`.
The hostile unrelated-data probe was placed on the Windows host mount rather
than only inside the Linux guest. Both platform fixtures ran no research or
authority action and claim no independent credit. They satisfy the Ubuntu and
WSL2 custody portions of the gate. The `v0.4.3` release workflow supplies npm
trusted-publishing provenance and final release evidence. The one-time
namespace bootstrap is retained as an explicitly unprovenanced prerelease and
earns no release credit.

Historical benchmark sources and immutable registrations stay in Git and
source archives but leave ordinary help and the installed npm payload.

## Release gate

Canopus 0.4 requires pack/install/provenance checks, macOS and Linux/WSL2
custody fixtures, multi-architecture verifier replay, released Vela
composition, and one real quantum-code Receipt routed Deferred with accepted
event delta zero and clean-clone reproduction.
