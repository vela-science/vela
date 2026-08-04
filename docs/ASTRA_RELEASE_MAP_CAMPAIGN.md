# Astra ten-result release-map campaign

## Objective

Turn the exact OpenAI `ten-proofs` release into a consequence-aware scientific
map: ten result families, twelve Comparator profiles, exact source and theorem
bindings, axiom inventory, fidelity conclusions, external status, local review
state, and one honest next obligation per selected result.

## Entry state

- Exact release commit: `29362184c2b698c1b279bc85b3957ee813646c63`
- The exact commit is not currently reachable from an advertised head or tag,
  but direct SHA fetch succeeds and reproduces tree
  `730bf2c6a13dbb96606024c5fd681a48633fb393`.
- All ten advertised result families and twelve Comparator profiles pass
  Comparator, Nanoda, and Lean's default kernel in the retained
  network-disabled Linux path.
- The exact inventory contains 41 terminal theorem declarations. A complete
  `#print axioms` audit reports `propext`, `Classical.choice`, and
  `Quot.sound` for every declaration.
- The rooted replay result is
  `sha256:5a60c3be27036c65a6a37bf55dce71abcb024cfecece92b8e7dcaf1324b095d0`.
- The Erdős 183 fidelity Claim has a passing scoped Verification but remains
  pending human Decision. Its exact source-local checkpoint is owned by
  [`erdos-frontier`](https://github.com/vela-science/erdos-frontier/blob/main/campaigns/erdos-183-astra-fidelity.md).
- The Erdős 146, 180, and 183 fidelity matrices and the consequence-aware
  ten-family status map remain incomplete.
- The aggregate Lake challenge target is stale because it names nonexistent
  module `ComparatorChallenges.C_PermanentSuperquadraticStandalone`; this
  does not invalidate the twelve JSON-declared profile runs.

## Work packages

1. **Complete:** freeze release commit/tree, both PDF roots, Lean project,
   manifest, license, toolchains, and all twelve profile roots.
2. **Complete:** build `All` from a clean pinned checkout and record the
   complete 41-declaration axiom inventory.
3. **Complete:** run every declared Comparator profile under the retained
   hardened contract; preserve the stale aggregate target and Docker
   disk-exhaustion incidents without scoring either as a scientific failure.
4. Create a ten-family map separating announcement wording, manuscript theorem,
   Lean declaration, checker result, novelty claim, external review/status,
   Vela Verification, and local Standing.
5. Complete definition/quantifier/hypothesis/conclusion matrices for Erdős 146,
   180, and 183.
6. Resolve the source-local Erdős 183 checkpoint only by explicit human
   Decision or documented deferral; this cross-Frontier campaign must not
   duplicate or exercise Erdős repository authority.
7. Derive one next obligation for each result selected for further review.

The replay result closes only the native-checking portion of this campaign.
Items 4–7 remain the active scientific work.

## Completion gate

- all ten result families are present with no invented status;
- all twelve profiles have a retained pass/fail/error outcome;
- clean offline replay and exact axiom inventory exist;
- the three Erdős fidelity matrices are consequence-complete;
- Erdős 183 is explicitly decided or deferred; and
- no checker passage is presented as novelty, acceptance, or global solution.

## Dossier reuse gate

Astra/Erdős 183 becomes case three only if the shared Dossier builder can bind
its source, check, fidelity, Decision/deferral, discrepancies, dependencies,
and nonclaims without adding Astra-specific authority code.

## Stop conditions

Do not repair a failing upstream proof inside this campaign, silently substitute
a profile, infer missing external review, or create an Astra Frontier. A failed
profile becomes an exact obligation; it is not omitted from the release map.
