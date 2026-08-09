# Astra / Erdős #183 verification evidence

This directory records one bounded reproduction of the exact OpenAI
`ten-proofs` release. It is evidence for the Vela campaign, not a new protocol
surface or a scientific Decision.

The retained result binds the upstream Git commit and tree, the exact Erdős
#183 challenge and solution bytes, the Lean and Comparator dependency
revisions, the locally built checker binaries, and an independent exact source
observation from `teorth/erdosproblems`.

The following commands were run from a clean checkout of the retained OpenAI
commit:

```sh
lake exe cache get
lake build All
lake build lean4export comparator
COMPARATOR_LANDRUN="$PWD/.lake/packages/comparator/scripts/fake-landrun.sh" \
COMPARATOR_LEAN4EXPORT="$PWD/.lake/packages/lean4export/.lake/build/bin/lean4export" \
COMPARATOR_NANODA=/path/to/pinned/nanoda_bin \
  lake exe comparator ComparatorChallenges/I_MulticolorTriangleRamsey.json
```

`lake build All` completed 8,666 jobs. Comparator reported that both Nanoda and
Lean's default kernel accepted the solution.

A second execution reproduced the same result as an unprivileged user in an
offline Linux container with every capability dropped, no-new-privileges,
private IPC, bounded processes, and real Landrun/Landlock confinement. Both
kernels accepted all four theorems and Comparator exited successfully. One
upstream argv defect remains: Landrun strips Lean4Export's nested `--`, so the
passing run used a content-hashed 14-line wrapper that restores only that
delimiter. The result is therefore a hardened offline Docker+Landrun
reproduction, not the literal pre-Linux-7.1 native `systemd-run` guarantee in
Comparator's documentation and not an independent scientific Decision.

The exact result and all nonclaims are in `result.json`.
