# Final Anthropic diagnostic corrective review

## Verdict

**BLOCKED** at producer commit `5dddab01f1555ab4ef6f44176f06cad03b89fa25`, tree `3aeabaf39f48a44821381448115812c7b898315d`.

The runtime-capture and lifecycle correction passes, and the authored Python surface passes locked formatting and lint. One residual file-custody gate remains blocked.

## Immutable binding

- Producer parent: `6f2bdc6007c1cc1bde60b433da947dbcb7935368`
- Current-main ancestor: `66e33872`
- Diagnostic artifact root: `sha256:648cb5c2707e3e59921b27f29b9d91de585ef31dbd1fa680def514d99a1b0ee3`
- Runtime artifact root: `sha256:ff310cd5c3bcefa5ece6589df441b1c0c2f115333013b1c42fa92629853f4a64`
- Offline record root: `sha256:c8b0126bb69c69d48836d3c36652dd64608f4596347577a1b6b0c91657e01db2`
- Registration root: `sha256:287261e48ad6ecc7566e23d0bc0212662d53ea79c9c2ad730c9f8e985f81fb4a`
- Permit-set root: `sha256:2197f871508416c8acafba676b57686ce79b9c1a50657a645943ec9aff72220a`
- Custody root: `sha256:835e81f4933d27118c12cede7dc5336d53a25cd2bec0601e971b4bd933d3fb44`

The producer branch was remote-equal, the commit had the stated sole parent and tree, the current main lineage was retained, strict Git object verification passed, and deterministic artifact regeneration returned the exact artifact root with no diff.

## Gate outcomes

### AD-F1 runtime capture and scoring: PASS

The maintained compiler accepts the actual runtime evidence shape, preserves request, response, tool-result, usage, terminal, and teardown custody, enforces sequential provider/tool lifecycle and fixed-denominator terminal retention, and publishes once. The scorer accepts only the compiled capture shape and retains the registered six-cell denominator and Decimal/tool secondaries.

Independent results:

- maintained runtime-capture tests: 7/7 PASS;
- artifact verifier/scorer tests: 39/39 PASS;
- Go runner and bridge packages: PASS;
- all six held qualifier bundles: PASS;
- lifecycle and workspace adversaries: PASS;
- zero-contact, non-success, one-tool, missing, reordered, duplicate-copy, and one-shot cases: PASS.

### AD-F2 shared file custody: BLOCKED

Final-file identity, hardlink, symlink, and direct name-replacement regressions pass. However, the shared absolute-path reader does not yet bind the complete path identity from a trusted descriptor root, and several scorer-owned registered inputs are still loaded outside that shared reader. A controlled file-identity adversary was accepted. This is a publication/runtime custody defect, not a scientific-design defect.

Minimal correction:

1. Traverse absolute evidence paths descriptor-relatively from an explicit trusted root without following mutable path components; preserve the pre-open/open/post-read/post-name identity invariant across the entire path.
2. Route every scorer-owned registration, assignment, permit, adjudication, and case input through the shared reader, validating its registered root while the descriptor remains open.
3. Add deterministic full-path and scorer-supporting-input identity regressions.

### SQ-F1 authored quality: PASS

Locked Ruff lint and format checks pass across the maintained qualification package and the complete authored diagnostic surface.

## Preserved state and claim ceiling

The state remains `0/6`: six diagnostic permits held, zero released, all twelve original Stage A permits held, zero credential-content accesses, provider calls, participant responses, terminal captures, and scoring attempts. Execution remains unauthorized, Stage B has zero selected families, and `authority_effect` remains `none`.

This review authorizes no provider call, permit release, scoring, merge, publication, Stage A/Stage B claim, cross-provider claim, scientific result, Frontier claim, Protocol/Core change, or Repository authority, Decision, or Standing action.

