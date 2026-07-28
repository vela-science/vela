# Third-party components

Canopus is licensed under Apache-2.0 OR MIT. The corresponding license texts
are in `LICENSE-APACHE` and `LICENSE-MIT`.

## Shipped npm package

`@vela-science/canopus@0.6.4` has no runtime npm dependencies. Its packaged
JavaScript uses only Node.js built-ins. The package also contains frozen
mission data and verifier capsules produced for the bounded profiles described
in `docs/MISSIONS.md`; their source and executable identities are bound by the
profile and release evidence.

The native Vela verifier capsules are distributed under Vela's Apache-2.0 OR
MIT licensing. The Python and shell verifier capsules use only their host
language or declared container toolchain and do not add a vendored application
dependency.

## Development-only dependencies

These packages are used to type-check and build Canopus but are not runtime npm
dependencies of the published CLI:

| Component | Role | Upstream license |
| --- | --- | --- |
| `typescript` | TypeScript compiler | Apache-2.0 |
| `@types/node` | Node.js type declarations | MIT |

Exact resolved versions and integrity hashes are recorded in `bun.lock`.

## Separately supplied host tools

The full tool-using workflow composes separately installed Vela, Codex CLI,
Git, Docker, and operating-system sandbox facilities. They are not bundled as
npm runtime dependencies and remain governed by their respective upstream
licenses and terms. Canopus records or checks their exact identities where the
mission contract requires it.

Third-party presence does not imply authority. The Codex worker, verifier,
Canopus process, and host tools cannot sign or perform a protected Vela
scientific acceptance decision.
