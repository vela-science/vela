# Independent narrow re-review: Vela flagship paper draft

## Verdict

**BLOCKED** for corrective producer commit
`e4782b4c3aa87d06aecb499b54680ff5bdf019b5`, tree
`de87a29d4d536fc45300c24628421b8fed3d7777`, parent
`3fdee23b4ab43c4e86b11e5ef32d08fcbc03e702`.

The original math-mode render failure is resolved. The sole remaining blocker
is that the documented render command does not reproduce the handed-off PDF
SHA-256 and does not produce stable PDF bytes across repeated executions.

## Exact corrective scope

The producer ref was remote-equal at the reviewed commit. The delta from the
blocked parent changes exactly two files:

- `paper/flagship/manuscript.md`: replaces the malformed inline and display
  formal-model notation with valid Pandoc math delimiters and equivalent
  transition notation;
- `paper/flagship/README.md`: adds the exact Pandoc/pdfTeX render command.

No empirical count, result root, claim status, Protocol 1 statement, controller
ceiling, Erdős 264 evidence boundary, held-out status, or authority wording
changed. `git diff --check` passed. The corrective diff stream has SHA-256
`4f51e1155447566a12203ab464c129d72cd960ec510aa1a8d80ac5cea5858275`.

## Blocking finding

### F02 — documented PDF render is not byte-reproducible

The exact command documented in `paper/flagship/README.md` now renders
successfully with Pandoc 3.9 and pdfTeX
3.141592653-2.6-1.40.26 (TeX Live 2024), so F01's malformed-math failure is
closed. However, a clean detached worktree produced:

```text
sha256:6d873e30d55d8c16bf46c08081ce47387655ce75203784ad572b18c8ea5591d1
```

instead of the handed-off expected root:

```text
sha256:ed69832eae3a396b381a8e8782ba247f5e00b5db58d5e95ba3a62252031c3fa4
```

A second execution of the same documented command produced a third root:

```text
sha256:000726321a1c5d8f64c1f00a2c4ac85fd90cb9eabc02058f4f2d9e1ce01b699a
```

Both PDFs are 246,920 bytes and five pages. `pdfinfo` shows their creation and
modification times differ, confirming that the documented command leaves
pdfTeX time metadata unfrozen. The claimed exact PDF root therefore cannot be
reproduced from the immutable producer bytes and documented command.

Minimal correction: freeze the render environment, including a deterministic
`SOURCE_DATE_EPOCH` derived from an exact bound commit, document that complete
command or provide a checked render entry point, regenerate the PDF root from
those exact bytes, and hand off a new immutable commit. No claim, evidence, or
protocol change is required.

## Passing checks

- Corrective commit, tree, parent, remote ref, two-path scope, and file modes
  matched the handoff.
- All relative Markdown links resolve.
- All three flagship Markdown files parse to standalone GFM HTML with Pandoc
  3.9.
- The corrected manuscript renders successfully to a five-page letter-size PDF
  with the documented Pandoc/pdfTeX toolchain.
- `./paper/flagship/reproduce.sh --integrity-only` passed with
  `positive_gate=not_supported`, `authority_effect=none`, and
  `held_out_status=not_run`.
- The full `./paper/flagship/reproduce.sh` passed from the clean detached
  worktree: Protocol 1 conformance root
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`,
  portable divergence 2/2, inherited-correction benchmark verification and
  16/16 tests, canonical post-result serialization fixture, and Erdős 264
  retained tests 2/2.
- The unchanged claim-evidence matrix and reproduction script retain SHA-256
  `a05371e945a03a505b830c4fc5ca3e91f72d219e73b351ec64110279f7314f65`
  and
  `c816b270ed806952d201954c5236a7c613803ef9ca38e718cff9b5bb0f3ac418`,
  respectively.

## Corrected source hashes

```text
sha256:f58b1b90505c5182770db76f214980850e14e25b658f3d59e627862d40ac938a  paper/flagship/README.md
sha256:612395c6ec113e7eda8afa11fa6360eca8240aaddbe4be6d5b4ae1fb97238f65  paper/flagship/manuscript.md
```

## Execution and authority disclosure

No provider call, held-out execution, participant rerun, merge, protected-key
access, scientific Decision, Repository authority action, Event, or Standing
change occurred. This review leaves the sealed negative result unchanged.
