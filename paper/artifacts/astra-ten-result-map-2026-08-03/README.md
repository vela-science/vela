# Astra ten-result release-map replay

This directory retains the historical environment recipe and rooted outputs
for the complete `openai/ten-proofs` Comparator profile pass. It is
cross-release campaign evidence, not a Vela protocol object, an Astra Frontier,
or a scientific Decision.

The recipe is not a supply-chain-reproducible rebuild. Its base image and final
source commit are pinned, but the image resolved operating-system packages and
downloaded the `elan` installer from a mutable branch at observation time. A
new build may therefore acquire different toolchain bytes; any new result needs
a separately versioned recipe with fully pinned inputs or retained build
artifacts.

The rooted result is `result.json`:

`sha256:5a60c3be27036c65a6a37bf55dce71abcb024cfecece92b8e7dcaf1324b095d0`

The consequence-aware ten-family read model is `consequence-map.json`:

`sha256:f219d4a1fe8ab71e516012fc8bd60a06db863e56be2c2be3920104b7472282dc`

At the retained boundary, all ten advertised result families, all twelve
Comparator profiles, and all 41 terminal theorem declarations were present.
Every profile passed Comparator, Nanoda, and Lean's default kernel. A separate
`#print axioms` audit found the same exact axiom set for all 41 declarations:
`propext`, `Classical.choice`, and `Quot.sound`.

The historical image build used network access to acquire its named sources and
toolchains. The retained replay ran only after that image was complete, as UID
10001, with all Linux capabilities dropped, no new privileges, private IPC,
bounded processes, and Docker networking disabled. Comparator additionally
confined each build, export, and Nanoda invocation with real Landrun/Landlock.

The pinned Landrun release strips Lean4Export's nested `--` delimiter. The
small retained wrapper restores only that delimiter before invoking the exact
Lean4Export binary. This compatibility boundary is explicit and means the
result is not an unmodified zero-shim Comparator invocation.

The exact release commit is no longer reachable from an advertised current
head or tag. The build therefore fetched the exact SHA directly. The later
replay checked both the commit and tree before scoring; that fetch and check
passed at observation time.

Build:

```sh
docker build --platform linux/arm64 \
  --tag vela-astra-map:2026-08-03 \
  paper/artifacts/astra-ten-result-map-2026-08-03
```

Hardened replay:

```sh
docker run --rm --network none --cap-drop ALL \
  --security-opt no-new-privileges --ipc private --pids-limit 2048 \
  vela-astra-map:2026-08-03
```

The aggregate Lake target retains one upstream infrastructure defect: it names
the nonexistent
`ComparatorChallenges.C_PermanentSuperquadraticStandalone`. Qualification
therefore follows the twelve JSON-declared profile modules directly. The
result also retains two Docker disk-exhaustion incidents as invalid
infrastructure attempts; every scored profile was restarted from the
beginning after scoped cache cleanup.

Kernel passage does not establish manuscript fidelity, novelty, field
acceptance, source status, or Vela Standing. Source-local review now assesses
Erdős 146 as faithful producer evidence, retains a qualified original-versus-
corrected-statement mismatch for Erdős 180, and preserves the verified Erdős
183 Claim as pending human Decision. The first eight families' theorem-level
fidelity and external-review status remain explicitly unassessed.
