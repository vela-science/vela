# Missions

Mission v1 is the current Canopus mission contract. Canopus owns only the
**Produce** step in Vela's five-step product story: produce → preserve → check
→ decide → reuse. The frontier Git repository preserves; Vela checks and
governs standing; signed policy or a protected human decision decides; the
Observatory and other replaceable readers support reuse.

## Mission v1: current

Mission v1 keeps the same four roles and authority boundary while adding a
tool-enabled local worker. `mission prepare` selects the first ranked attack,
derives the clean Git and Vela roots, registers the complete pre-existing
strict blocker set, and copies the exact packet, native permission profile,
structured-output schema, and verifier capsule into a portable bundle. The
Codex binary, model, verifier image, exact Linux verifier platform, resource
ceilings, and every copied byte are hash-pinned. Historical Mission v1 records
without a platform retain their old replay behavior; newly prepared missions
always bind one. Product-prepared missions also retain the selected profile's
exact name and full root, including Defer-only runs; older Mission v1 records
without that non-protocol field replay unchanged.

The producer runs through the native Codex CLI under a default-deny platform
profile: Seatbelt on macOS and Codex's Bubblewrap sandbox on Linux or WSL2. The
writable workspace contains only the exact target packet; the full source
checkout, Vela home, host home, and authentication file remain outside
command-readable paths. Provider transport is available only to the Codex
process. Shell commands have no network access.

Worker status is producer completion, not verifier or scientific standing.
`success` means the worker supplied every artifact byte required by the output
contract and explicitly leaves verification pending. Only such a draft proceeds
to the separate network-denied verifier. `null` means the bounded work produced
no candidate; `failed` means the worker could not produce a contract-complete
candidate or observed disqualifying evidence. Both are preserved in
`engine-result.json` and stop before verification or Receipt landing. After a
verifier pass, Canopus publishes exactly the frozen artifact sources in one
unsigned non-authoritative Git commit and then calls `vela land`.
The frozen verifier manifest reports the actual Docker boundary and bound Linux
architecture; the older macOS-Seatbelt manifest remains only for Mission v0.
`canopus inspect latest` includes safely stopped runs as well as completed runs;
its failure projection distinguishes no landing attempt from an observed Vela
effect that requires the retained landing-recovery evidence. Withdrawal's
`latest` selector remains limited to completed, proposal-bearing runs.

Defer-only Mission v1 bundles preserve their original zero-delta behavior. A
profile may register `permit` only when it also freezes one closed positive
`canopus.result-contract.v1` and a `vela.execution-binding.v1` over the full
packet, profile, verifier-capsule, and result-contract roots. The worker result
must be an exact computational success, the verifier must pass, its claim is
replaced by the contract's exact canonical claim, and every required artifact
kind must be present before Canopus authors the binding through Vela's Receipt
builder. Any mismatch stops before landing. Vela alone evaluates the
already-signed policy; Canopus then requires the registered route and
accepted-event delta and reproduces the retained Receipt from a clean clone.
Canopus does not treat its own Receipt verifier row as load-bearing Vela
assurance.

For exact construction profiles, prefer Vela-native `vela-witness` JSON and a
packaged `vela-verify` executable. This gives the isolated producer verifier,
Vela policy derivation, strict replay, and third-party reproduction one shared
pure verifier and claim-fidelity contract. A target-specific wrapper is kept
only when Vela has no suitable verifier; it is not duplicated for presentation
or convenience.

## Mission roles

One Mission v1 bundle runs one explicit role. The role changes the worker
instruction, not the trust boundary:

- `producer` constructs the smallest candidate or preserves a null result;
- `adversary` seeks a concrete counterexample or narrows the claim;
- `verifier` checks correspondence between the candidate and declared tests,
  while the separate frozen executable remains the mechanical verifier; and
- `fidelity` checks that prose claims do not outrun frozen artifacts and
  verifier facts.

A research loop may fan these into separate missions against the same exact
accepted roots. Each result lands its own Receipt and route. A repair mission
names an immutable parent candidate, but a substantive downstream mission may
inherit only state that Vela reports as accepted. Defer, a verifier pass, or a
Canopus candidate digest is not an inheritance edge.

This deliberately avoids a workflow DSL, agent society, or second graph. Git
stores the bytes, Vela stores authority and accepted lineage, and Canopus is
replaceable orchestration over those primitives.

## Mission v0: historical replay only

Mission v0 remains readable so immutable historical run records can be
inspected and replayed with their released Canopus version. The current branch
does not ship its retired tool-free execution adapter or benchmark corpus. Do
not prepare new product missions with v0; use Mission v1. Exact v0 source and
results remain available from the release tags that created them.
