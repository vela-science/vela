# Narrow AD-F2 corrective review

## Verdict

**BLOCKED** at producer `1da7c53a3f7a5bce172aa50f0a069964c1b67b15`, tree `ed69531d346d593ffc754e6beaf53f7ecaefccdc`.

AD-F2 itself passes. The exact trusted-root reader, registered scorer inputs, held-byte parsing, complete-path identity checks, and file-identity adversaries reproduce. SQ-F1 also remains PASS. The overall prelaunch verdict remains BLOCKED because the ordinary locked maintained runtime-capture test command no longer passes on the supported macOS host.

## Exact binding

- Parent: `5dddab01f1555ab4ef6f44176f06cad03b89fa25`
- Prior BLOCKED review: `611f4c0a3c61a4543945d5d5ccae3c0cd2c35808`
- Current-main ancestor: `66e33872`
- Runtime artifact: `sha256:b8535016ae32438159f55966aba8692c1df4d2f948b395378d558691e1c3e615`
- Runtime offline record: `sha256:d823263fc7293f48cbc0e178c5b6c9e02542cbbf6dcc9a0736d9310e869621bd`
- Runtime registration: `sha256:16587502b39c4af3092743e3e908eb47b66759c7382dcbe4e31472885969d52c`
- Diagnostic artifact: `sha256:e4f4b44cba1cd7f1cd953d7869e6a6193e57858476e34fac72e7908d1562fc78`
- Diagnostic registration: `sha256:d18bb4ef8aa79b3c8fb2ef59c5cc434bd6146211a26320ae5f7632aa6fb02ac0`
- Permit set: `sha256:2197f871508416c8acafba676b57686ce79b9c1a50657a645943ec9aff72220a`
- Custody: `sha256:cb8da1b113d23d616b619e59234bda6857897fa9acae409aa595d95a2dbba73f`

Remote equality, sole-parent topology, main ancestry, strict Git object checks, and deterministic regeneration passed with zero diff.

## Gate outcomes

- AD-F2 trusted-root and scorer custody: PASS. Shared-reader tests 5/5, diagnostic scorer/verifier tests 40/40, evidence tests 6/6, maintained qualifier tests 59/59, all six held bundles, and seven bundle adversaries passed.
- AD-F1 runtime behavior: substantive canonical-root execution PASS 7/7, but ordinary locked host execution errors 5/7 because the test fixture supplies a platform-aliased temporary root that the corrected strict reader properly rejects.
- SQ-F1: locked Ruff lint and formatting PASS over the complete authored surface.
- Go runner/bridge and diagnostic verifier: PASS.

## Minimal correction

Make the maintained runtime-capture fixture create and pass a canonical, non-aliased trusted root on every supported host, and add an explicit regression confirming that aliased trusted roots fail closed. The production reader should remain strict. No scientific or participant bytes need to change.

## Preserved state and claim ceiling

The state remains 0/6: six diagnostic permits held, zero released, all twelve original Stage A permits held, zero credential-content accesses, provider calls, participant responses, terminal captures, or scoring attempts. Execution is unauthorized, Stage B has zero selected families, and `authority_effect` remains `none`.

This verdict authorizes no call, permit release, score, merge, publication, scientific or cross-provider claim, Protocol/Core change, or authority, Decision, or Standing action.

