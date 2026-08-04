# Exact Lean replay contract candidate

Status: source-local Level 0 experiment; not a released Vela package.

This directory tests the smallest reusable boundary shared by exact Lean
replays. It provides file/root checks, pinned Git and Lean environment checks,
fail-closed axiom parsing, network-denied command construction on macOS, and
closed request/result schemas. It does not contain a theorem, Claim,
Verification Record, Decision, package resolver, registry client, or authority
credential.

The candidate is intentionally source-local. Its coordinate is a usability
label only:

```text
vela-science/lean-replay-contract@0.0.0-source-local
```

Exact identity is the `sha256:` package root emitted by `build_root.py`. A
consumer reference retains the JCS descriptor bytes as well as the root and
verifies every file before importing the implementation. The descriptor uses
the maintained `rfc8785` implementation pinned in `requirements.lock.txt`.

Run the focused checks from the Vela repository root:

```bash
uv run --project conformance \
  python research/lean-replay-contract/test_contract.py
uv run --project conformance \
  python research/lean-replay-contract/build_root.py
```

The experiment may be promoted only when the same root is used by the retained
Formal replay and one real Erdős replay, two independent root readers agree,
clean macOS and Linux reconstructions pass with network disabled, and the
consumer changes delete more maintained replay-contract logic than they add.
Until then there is no package release, package CLI, package lock, Web index,
OCI artifact, or hosted registry.

## Boundary

- Lean, Elan, Lake, and Mathlib remain native dependencies.
- Each consumer owns its theorem, source splice, scientific records, timeout,
  and scoped interpretation.
- The contract reports operational evidence only.
- `authority_effect` is always `none`.
- A pass cannot create or alter scientific Standing.
- Removing this directory may break optional replay convenience, but cannot
  change any retained Claim, Verification, Decision, Event, or Standing.

