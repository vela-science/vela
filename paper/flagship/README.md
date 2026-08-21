# Vela flagship evidence paper

This directory contains a separate working manuscript for Vela's flagship
evidence program. It does not replace [`paper/vela.md`](../vela.md), change
Protocol 1, or report a scientific Decision.

The draft separates four questions:

1. Do conforming readers replay the same valid Repository bytes to the same
   roots and Standing?
2. Can one authenticated Submission cross Repository boundaries while each
   Repository retains its own authority and Decision?
3. Can an external controller orient work without gaining authority over
   Standing?
4. Does a Vela-organized correction packet lower cold-successor continuation
   cost under a preregistered matched comparison?

The first three have bounded executable evidence. The sealed 16-session study
did not satisfy its preregistered positive gate. A fresh 36-cell,
three-family, three-arm held-out design remains work in progress and has not
run.

Read the [claim-evidence matrix](CLAIM_EVIDENCE.md) before the
[manuscript](manuscript.md). The matrix controls the wording of empirical
claims.

## Reproduction contract

Run the retained evidence checks from a clone that contains the historical
commits named in the manuscript:

```bash
./paper/flagship/reproduce.sh
```

The command creates disposable detached worktrees, checks the immutable roots,
then runs the Protocol 1, portable-divergence, inherited-correction, and
Erdős 264 retained checks. It never runs a model, invokes a Decision, changes
Standing, or prints the protected held-out adjudication.

Use `--integrity-only` to check the bound commits, roots, categorical outcomes,
and manuscript evidence without compiling Rust or restoring Python
dependencies.

## PDF render

The qualifying local renderer uses Pandoc 3.9 and
pdfTeX 3.141592653-2.6-1.40.26 from TeX Live 2024. It requires a clean source
tree and sets `SOURCE_DATE_EPOCH` to the exact `HEAD` commit timestamp before
Pandoc starts. Run:

```bash
python3 paper/flagship/render.py
```

The renderer reports the commit, tree, source timestamp, source and support
roots, and generated PDF root. Two renders of one clean commit must be
byte-identical.

The public-ready paper gate requires this command to pass from the paper's
exact source commit. An outside person or institution running the package
after publication supplies downstream external reproduction; the manuscript
does not count that future event as current evidence.
