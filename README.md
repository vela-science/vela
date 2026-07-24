<p align="center">
  <img src="assets/brand/vela-readme-hero.jpg" width="960" alt="Vela preserves evidence through reproduction and review, while accepted state changes only at an exact authority boundary." />
</p>

<p align="center"><strong>Version control for scientific state.</strong></p>

<p align="center">
  Capture evidence. Reproduce it exactly. Keep verification and scientific authority distinct.
</p>

<p align="center">
  <a href="https://github.com/vela-science/vela/releases"><img alt="GitHub release" src="https://img.shields.io/github/v/release/vela-science/vela?style=flat-square&color=C9A664&labelColor=081224" /></a>
  <a href="https://crates.io/crates/vela-cli"><img alt="crates.io" src="https://img.shields.io/crates/v/vela-cli?style=flat-square&color=4F8F8B&labelColor=081224" /></a>
  <a href="https://github.com/vela-science/vela/actions"><img alt="Conformance status" src="https://img.shields.io/github/actions/workflow/status/vela-science/vela/conformance.yml?branch=main&style=flat-square&label=conformance&labelColor=081224" /></a>
  <a href="LICENSE-APACHE"><img alt="Apache-2.0 OR MIT" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-C9A664?style=flat-square&labelColor=081224" /></a>
</p>

<p align="center">
  <a href="https://www.vela.space">Website</a> ·
  <a href="https://app.vela.space/frontiers">Observatory</a> ·
  <a href="docs/PROTOCOL.md">Protocol</a> ·
  <a href="docs/CLI.md">CLI</a> ·
  <a href="docs/QUICKSTART.md">Quickstart</a>
</p>

Vela turns a Git repository into a **frontier**: a content-addressed,
replayable record of evidence, review, policy, and the scientific state accepted
by one named authority for one bounded scope.

The core rule is simple: **verifier success is evidence, not acceptance**.
Agents may produce work and verifiers may check it. Accepted scientific state
changes only through an exact human decision or a narrow policy that a human
already signed.

## Start with a replay

Install the checksum-verified public beta on Apple Silicon macOS or Linux
x86-64:

```sh
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.915.1/install.sh \
  | VELA_VERSION=v0.915.1 bash
vela --version
```

Windows x86-64:

```powershell
& ([scriptblock]::Create((Invoke-WebRequest https://raw.githubusercontent.com/vela-science/vela/v0.915.1/install.ps1).Content)) -Version v0.915.1
vela --version
```

The native Windows binary supports inspection, checking, reproduction,
key-free Target Index previews, and protected human signing. Profile v1
settings updates and `target-index seal --apply` remain fail-closed on native
Windows in this release: the write edge requires an exact-preimage atomic
exchange that the current Windows implementation does not claim. Run those
repository-file mutations from WSL2 with the checkout on its Linux filesystem,
or from a supported Unix host. Vela does not silently fall back to path-based
replacement.

Reproduce the included Sidon witness set from tracked bytes:

```sh
git clone https://github.com/vela-science/vela.git
cd vela
vela reproduce examples/sidon-a309370
```

Success means the retained bytes still produce the recorded scoped result. It
does not mean the result is universally true or scientifically accepted.

## Produce bounded work

```sh
vela status <frontier>
vela next <frontier> --json
vela work <target> --frontier <frontier> --as agent:<you> --json

vela land --frontier <frontier> --work <target> --claim <result> \
  --type computational --replayability exact --artifact <path>:<kind> \
  --caveat <scope-limit> --as agent:<you> --json

vela reproduce <frontier>
```

`vela land` creates a Receipt. A signed policy may `Permit` an exact,
pre-authorized result class or `Defer` it for human review. Producers can
withdraw their own pending work; they cannot accept or reject it.

## The boundary

| Evidence path | Authority path |
| --- | --- |
| Git fixes exact bytes and ancestry. | Signed policy can route only an exact allowed class. |
| Verifiers check one declared claim and artifact. | A protected human decision accepts or rejects one proposal. |
| Receipts preserve producer, packet, caveat, and verifier roots. | The Observatory and other replaceable readers remain read-only projections. |

Verification is derived from retained attachments, not a mutable status field.
See [Verification](docs/VERIFICATION.md) for the exact gate and reject vectors.

## Build from source

Vela requires a current stable Rust toolchain.

```sh
cargo build --release
./target/release/vela --help
python3 conformance/verify.py
```

To install from crates.io, run
`cargo install --locked vela-cli --version 0.915.1`. Human signing setup is
documented under [Review and authority](docs/CLI.md#review-and-authority); agents
must never receive a human key.

## Documentation

- [Start here](docs/QUICKSTART.md): inspect, produce, initialize, or migrate
- [Produce work](docs/PRODUCER_QUICKSTART.md): contribute without a human key
- [Protect authority](docs/SIGNING.md): custody, exact decisions, and repository administration
- [Understand the protocol](docs/PROTOCOL.md): objects, events, replay, and standing
- [Inspect exact contracts](docs/RECEIPTS.md): Receipts, roots, and provenance
- [Operate safely](docs/THREAT_MODEL.md): trust boundaries and governance

## License

Code is dual-licensed under Apache-2.0 OR MIT, at your option. The Vela name and
marks are trademark rights reserved; see [`assets/brand/LICENSE`](assets/brand/LICENSE).
