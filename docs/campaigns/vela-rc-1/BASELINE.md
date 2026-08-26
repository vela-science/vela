# VELA-RC-1 frozen baseline

Recorded before any RC-1 implementation change on 2026-08-26,
America/Toronto.

## Source identity

| Field | Frozen value |
| --- | --- |
| Repository | `https://github.com/vela-science/vela.git` |
| Source checkout | `/Users/williamblair/personal/vela` |
| Source branch before RC-1 | `campaign/compose1-supervisor` |
| RC-1 supervisor branch | `campaign/vela-rc-1-supervisor` |
| HEAD | `421cdc0dc9e9aee57b604e57a8bf5401ab957645` |
| Tree | `24d1f8a314db2dc728ab6a01f0c2ada29bbba0e0` |
| Git status | clean |
| Vela | `0.977.4` |
| Protocol | `Vela Protocol 1`, release candidate |
| Submission | `vela.submission.v3` |
| Verification Record | `vela.verification-record.v2` |
| Proposal | `vela.proposal.v1` |
| Proposal withdrawal | `vela.proposal-withdrawal.v2` |
| Repository index | `vela.repository.v4` |
| Status projection | `vela.status.v4` |
| Claim record | `vela.claim-record.v1` |
| Decision Inbox | `vela.decision-inbox.v3` |
| Migration | none at baseline |

The baseline is an unreleased strict descendant of public tag `v0.977.4`, which
names commit `1a2e0328620b4e8c4584c3d4baf257adb11f3d45`. Its version string therefore
does not by itself identify the RC-1 candidate.

## Toolchain and public routes

- Rust `1.97.1`, with `rustfmt` and `clippy`, is pinned by
  `rust-toolchain.toml`; the workspace uses Rust edition 2024.
- Python conformance uses the locked `conformance/uv.lock`; hosted CI selects
  Python 3.13, uv 0.11.32, and Node 24.
- Public binary installation pins signed tag `v0.977.4` through `install.sh`.
- Source installation is `git clone`, `cargo build --release`, then
  `./target/release/vela --help`.
- Published binaries support macOS Apple silicon and Linux x86-64.
- Public entry points are `README.md`, `docs/QUICKSTART.md`, `docs/CLI.md`,
  `docs/PROTOCOL.md`, and `docs/README.md`.
- The public reference example is the pinned Vela Math Repository.

The CLI product loop is `init -> submit -> verify -> decide -> replay`. The
top-level CLI exposes `init`, `status`, `claims`, `submit`, `show`, `why`,
`review`, `replay`, and `log`; advanced verification and maintenance surfaces
are documented separately.

## Existing release and CI state

The latest published tag is `v0.977.4`. The latest hosted conformance run
observed during freeze was successful at commit
`23c2eb86b0deb1b155807fae16bcd7ba5bb707c0` (run `32794177685`, completed
2026-08-25T00:39:59Z). It does not qualify the later baseline commit. The most
recent hosted release workflow observed was successful for `v0.977.4` commit
`1a2e0328620b4e8c4584c3d4baf257adb11f3d45` (run `32447842087`).

The checkout contains ignored local `dist/` residue describing `v0.977.2`; it
is not tracked candidate evidence and must not be confused with a prospective
RC-1 release bundle. The baseline local debug binary reports Vela `0.977.4`
and has SHA-256
`f6a0d6c7ccd7f406367f550dac74a17fbdb70a61bb0fac5562a53e8a6eda8a57`.
This is only a local diagnostic identity, not a release digest.

## Inherited evidence locations

- VELA-COMPOSE-1 final report:
  `docs/campaigns/vela-compose-1/FINAL_REPORT.md`.
- T7 internal qualification:
  `docs/campaigns/vela-compose-1/T7_RELEASE_QUALIFICATION.md`.
- T4 source Repository:
  `/Users/williamblair/Documents/Codex/2026-08-26/vela-compose-1-lean-vertical/work/t4-lean-repository`.
- T5 source Repository:
  `/Users/williamblair/Documents/Codex/2026-08-26/vela-compose-1-t5-alzheimer-lifecycle/outputs/vc1-e004-alzheimer-governed-lifecycle`.

The inherited Protocol 1 root is
`sha256:e7a6d288918692d6a6186cc3e612871f167ba954c4cc31de28cce182a66a0afd`.
It binds 77 normative and 39 informative files. RC-1 must independently
reproduce it and the T4/T5 roots rather than trust the prior summary.
