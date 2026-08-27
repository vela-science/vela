# Vela organization: current architecture

Status: canonical observed-state map after VELA-ORG-2
Observation cutoff: 2026-08-27

This document records the active topology. Historical campaign receipts retain
their original names, versions, and provider identities; they are evidence,
not alternate current architecture.

[Vela Protocol 1](../PROTOCOL.md) remains normative for scientific objects,
replay, and authority. [Vela architecture](../ARCHITECTURE.md) defines the
component model, and [repository ownership boundaries](../REPOSITORY_BOUNDARIES.md)
define placement.

## Canonical system

```text
vela.space -> www

scientific Repository Git
        +
signed Vela 0.977.6
        |
        v
public Problems projection -> problems.science
        |
        v
native Work / Workbench -> unsigned candidate handoff
        |
        v
Repository Verification -> authorized Decision -> replayed Standing
```

Git supplies byte and ancestry custody. Vela supplies canonical objects,
authority evaluation, and replay. Scientific repositories own their own source
locks, Decisions, and Standing. Problems owns discovery, hosted Work, and a
read projection. Workbench owns local execution and evidence preparation.
`www` is only the public front door.

## Immutable release boundary

Vela `0.977.6` is the current signed Protocol 1 release.

- release commit: `9ac8e7730bfb63a3b8eb1d2e1d91081c3e703c59`
- release tree: `1332713f627ac73c235e4f9a7afe206499717154`
- Protocol 1 manifest root:
  `sha256:bf1ef68165bccbc4d2e8a854f78c70448cc7de771bac23329f7a8ca115303f56`
- macOS runtime SHA-256:
  `5b21415c98503b20518c0e68714b0b4f4b3c371525ea110563b89a53a0d3dbb3`

Historical releases and receipts remain immutable at their original versions.

## Active repositories

| Repository | Visibility | Sole responsibility | Qualified main at cutoff |
| --- | --- | --- | --- |
| `vela` | public | Protocol 1, Rust CLI, schemas, conformance, replay, signed releases | descendant of `3fe64a523aa25b39d3c5a0d90a02b1187901dfd2` |
| `problems` | public | Problems, hosted Work, public read projection, authority-neutral WebMCP | `4d3561fcefab471be400b2d25da4b89312731642` |
| `workbench` | public | Local execution, evidence preparation, explicit Submission handoff | `7449aebf79afa2ae497a84887a1c8ba5e11c400d` |
| `math` | public | Reference mathematics authority Repository | `36c6d0fef71a3fec84d9dcd5eeca4e22e378f7cb` |
| `www` | public | Static public front door | `1d455ca61204d3e8adb0699c76ad9eb53d268a6a` |
| `.github` | public | Organization profile and shared GitHub configuration | `b2d3f87e2b6a468b3736924889deacb51a5c47f2` |
| `lean-correspondence` | public | Optional non-authoritative exact relationship receipts | `01d0b3253227bc41d2edc13e5cb318bdae53fc88` |

The active product budget is Core, Problems, Workbench, Math, and www, plus the
organization profile. `lean-correspondence` is the single optional active
reference and owns no scientific authority.

## Deployments and DNS

| Public name | Provider project | Source | Deployed revision | State |
| --- | --- | --- | --- | --- |
| `problems.science` | Vercel `problems` | `vela-science/problems` | `cee75db394ccca624ebb3a7f0c2566a909cd4b3a` | governed release, exact manifest |
| `vela.space` | Vercel `www` | `vela-science/www` | `1d455ca61204d3e8adb0699c76ad9eb53d268a6a` | canonical tiny hero |
| `www.vela.space` | Vercel `www` | redirect | same deployment | permanent redirect to apex |

The Vercel projects `vela-web-www` and `vela-web-problems` were deleted after
live cutover verification. Namecheap no longer contains the dangling
`app.vela.space`, `app.constellate.science`, or `erdos.constellate.science`
CNAMEs.

## Problems projection and provider boundary

The governed Problems release binds:

- public source commit `cee75db394ccca624ebb3a7f0c2566a909cd4b3a`;
- Vela `0.977.6` and runtime digest
  `5b21415c98503b20518c0e68714b0b4f4b3c371525ea110563b89a53a0d3dbb3`;
- Math source `36c6d0fef71a3fec84d9dcd5eeca4e22e378f7cb`;
- projection release root
  `sha256:a4a96441668a9f8b33afbcb3f696a2639f5fae145f36ddc063d7f67d5fb87ce8`;
- public source-adapter set root
  `sha256:0ea01e2806386d0bbf68e0e9960dbc03f9883d7b62f7deeb24171b59bbc46647`.

Public reconstruction produced byte-identical projection artifacts without a
private dependency. The production manifest identifies the exact deployment,
source, Vela digest, release root, and anonymous retrieval path.

Neon project `lingering-meadow-20929365` is named `vela-problems`. It retains
only `neondb`, `vela_activity`, and `vela_projection`, one primary compute, and
the active least-privilege role families. The stale preview branch, read
replica, `vela_observatory` database, and unused legacy roles were removed.
Neon remains operational storage, never scientific authority.

WebMCP can read projection and hosted Work, mutate attributed hosted Work, and
prepare unsigned candidates with `authority_effect:none`. It cannot sign,
verify, decide, or mutate Standing. VELA-ORG-2 did not submit the WebMCP
challenge and created no stability freeze.

## Workbench and local operator

Workbench source is qualified on public `vela-science/workbench` and binds the
signed Vela `0.977.6` runtime. The deterministic macOS arm64 artifact remains
unsigned qualification evidence because this release host has no Developer ID
identity or notarization credentials. It must not replace the installed app.

The shell-resolved local CLI is `~/.local/bin/vela` `0.977.6` with SHA-256
`5b21415c98503b20518c0e68714b0b4f4b3c371525ea110563b89a53a0d3dbb3`.
The obsolete Cargo binary is disabled and no shell alias overrides the current
path.

## Archived custody

`vela-web` is archived private historical implementation custody with no live
deployment. The frontier repositories, `vela-frontiers`, `vela-internal`,
`vela-research-harness`, and `vela-site` remain archived historical custody.
Archive state is not permission to rewrite or delete their history.

## Repository policy

The five active product repositories protect the default branch from deletion
and non-fast-forward updates while preserving organization-administrator
recovery. Their `v*` release tags are immutable where release tags apply. Core
CI remains repository-owned; the rules deliberately avoid mandatory-review
ceremony for the single-founder development model.

## Remaining external blocker

The only active-system blocker is signed and notarized Workbench distribution.
It requires a Developer ID Application certificate/private key and App Store
Connect notarization credentials. The unsigned build and historically installed
app do not invalidate the source, protocol, Problems, DNS, or www cutovers.
