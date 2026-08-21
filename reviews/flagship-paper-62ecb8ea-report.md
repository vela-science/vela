# Independent narrow F02 re-review: Vela flagship paper draft

## Verdict

**PASS** for exact corrective producer commit
`62ecb8ea7771793e9e1232fc99795e65d417dcb7`, tree
`e7672745f016f57b9111209c5bb55bb310cce50a`, parent
`e4782b4c3aa87d06aecb499b54680ff5bdf019b5`.

This PASS is exact-byte scoped to the deterministic flagship renderer and its
documentation. It closes F02 from review
`3f43b3eaad98943b0b772e964677f7dfb3ebce47`. It does not authorize merge,
inference, held-out execution, scientific claims, or authority action.

## Exact corrective scope

The producer ref was remote-equal at the reviewed commit. The delta from the
blocked parent changes exactly two paths:

- `paper/flagship/README.md`, replacing the raw Pandoc recipe with the checked
  renderer command and documenting its clean-tree, commit-time, runtime, and
  byte-identity contract;
- executable `paper/flagship/render.py`, adding the exact checked renderer.

The manuscript, claim-evidence matrix, and reproduction entry point are
byte-identical to the blocked parent. No empirical result, claim status,
Protocol 1 statement, controller ceiling, Erdős 264 boundary, held-out status,
or authority wording changed. `git diff --check` passed. The corrective diff
stream has SHA-256
`52e7c10f1e9d21ed9e5abd32e34a2ddc62af49c5e32b8280ed443d47196fc6b5`.

## Deterministic render result

From a fresh detached worktree, the exact documented command and toolchain
reported:

```json
{"pandoc_version":"pandoc 3.9","pdf_bytes":246912,"pdf_root":"sha256:a8ccfb67fc5deab594b6fb4a7b2906c9e38920c8b7d115445252dd4ca95f000e","pdflatex_version":"pdfTeX 3.141592653-2.6-1.40.26 (TeX Live 2024)","qualifying_clean_build":true,"source_date_epoch":1787345750,"source_root":"sha256:612395c6ec113e7eda8afa11fa6360eca8240aaddbe4be6d5b4ae1fb97238f65","vela_commit":"62ecb8ea7771793e9e1232fc99795e65d417dcb7","vela_tree":"e7672745f016f57b9111209c5bb55bb310cce50a"}
```

Two executions in the first clean detached worktree produced byte-identical
PDFs. A third execution from a second detached worktree at a different path
also matched byte-for-byte. All three outputs were 246,912 bytes with root
`sha256:a8ccfb67fc5deab594b6fb4a7b2906c9e38920c8b7d115445252dd4ca95f000e`.

The reported `source_date_epoch` exactly equals
`git show -s --format=%ct 62ecb8ea7771793e9e1232fc99795e65d417dcb7`.
The renderer sets `SOURCE_DATE_EPOCH` before Pandoc starts and also fixes
`LANG=C`, `LC_ALL=C`, and `TZ=UTC`.

## Fail-closed and support bindings

- A clean detached worktree is required. An untracked sentinel caused exit 1
  with `flagship paper render: Vela worktree must be clean` before any PDF was
  produced.
- Pandoc and pdfLaTeX first-line versions must exactly equal the documented
  versions before rendering.
- The result binds the exact Vela commit, tree, source timestamp, manuscript,
  Lua filter, preamble, tool versions, PDF root, and PDF byte count.
- Support roots reproduced as:
  - filter:
    `sha256:dd9388017b4b9880658fdbfefd1cc46a6e2fe573d1b956df84e6837d07d12010`;
  - preamble:
    `sha256:4e966fe97fc9ef43677748597ff0989ecd25cf9b7759fff62717f2309991677b`.

## Additional focused checks

- All relative flagship Markdown links resolve.
- All three flagship Markdown files parse to standalone GFM HTML with Pandoc
  3.9.
- `./paper/flagship/reproduce.sh --integrity-only` passed and retained
  `positive_gate=not_supported`, `authority_effect=none`, and
  `held_out_status=not_run`.
- Source hashes:

```text
sha256:4be0614fdbd5f0e85e64a741e0f47ffc7898dd3026ab2e89d04cf5122787475b  paper/flagship/README.md
sha256:8ad7973ff8d2e936452f29c478486806a472c2b372394b7d95fc637e32f9ad79  paper/flagship/render.py
sha256:612395c6ec113e7eda8afa11fa6360eca8240aaddbe4be6d5b4ae1fb97238f65  paper/flagship/manuscript.md
sha256:a05371e945a03a505b830c4fc5ca3e91f72d219e73b351ec64110279f7314f65  paper/flagship/CLAIM_EVIDENCE.md
sha256:c816b270ed806952d201954c5236a7c613803ef9ca38e718cff9b5bb0f3ac418  paper/flagship/reproduce.sh
```

## Execution and authority disclosure

No provider call, participant rerun, held-out execution, protected-key access,
merge, scientific Decision, Repository authority action, Event, or Standing
change occurred. The sealed negative result and all claim ceilings remain
unchanged.
