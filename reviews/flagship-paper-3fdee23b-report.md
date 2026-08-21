# Independent review: Vela flagship paper draft

## Verdict

**BLOCKED** for producer commit
`3fdee23b4ab43c4e86b11e5ef32d08fcbc03e702`, tree
`4a8d99d845513cf7e34d14d954a42a8a7f57524a`, over base
`4685462c44b1f073870f31025ae73d1d8770ce73`.

This verdict is exact-byte scoped. The producer branch was remote-equal at the
reviewed commit. The reviewed range adds exactly four paths under
`paper/flagship/`, 555 lines total. Producer bytes were not modified.

## Blocking finding

### F01 — the manuscript cannot render to PDF

The inline formal-model expressions in `paper/flagship/manuscript.md` use
ordinary parentheses where Pandoc math delimiters are required. For example,
the source contains `(H(o)=\mathrm{SHA256}(B(o)))`. A direct clean-checkout
render with the repository's document toolchain fails:

```text
pandoc paper/flagship/manuscript.md \
  -o /tmp/vela-flagship-manuscript.pdf --pdf-engine=pdflatex

Error producing PDF.
! LaTeX Error: \mathrm allowed only in math mode.
...
l.193 (H(o)=\mathrm
```

The exact toolchain was Pandoc 3.9 and pdfTeX
3.141592653-2.6-1.40.26 (TeX Live 2024). This fails the assigned render gate and
the manuscript is not yet a public-ready paper artifact.

Minimal correction: use valid Pandoc math delimiters for all inline and display
formal-model expressions, add or document the exact flagship render command,
and hand off a new immutable commit whose PDF render passes. No evidence,
claim, result, protocol, or authority wording needs to change for this finding.

## Passing findings

- Remote producer ref resolved exactly to the reviewed commit; commit tree and
  base merge ancestry matched the handoff.
- The range contains only:
  `paper/flagship/CLAIM_EVIDENCE.md`, `paper/flagship/README.md`,
  `paper/flagship/manuscript.md`, and `paper/flagship/reproduce.sh`.
- `git diff --check` passed.
- Relative Markdown links resolve locally. All three Markdown files parse as
  GFM HTML with Pandoc 3.9.
- `./paper/flagship/reproduce.sh --integrity-only` passed and emitted
  `positive_gate=not_supported`, `authority_effect=none`, and
  `held_out_status=not_run`.
- The full `./paper/flagship/reproduce.sh` passed from the fresh clone. It
  reproduced Protocol 1 root
  `sha256:6ca327d6ec6f56c051e236be9eee42629e1f67dce393c1518e051ff8c14b279e`,
  portable divergence 2/2, benchmark verification and 16/16 tests, canonical
  post-result serialization fixture, and Erdős 264 retained unit checks 2/2.
- The sealed categorical result matches the manuscript: Git/documents
  112 points, 0 exact successes, 8 authority errors, restricted mean 600 s;
  Vela 130 points, 5 exact successes, 3 authority errors, restricted mean
  233.07823840475 s; ratio 0.388463730674583; and
  `positive_gate=not_supported`.
- The reviewed miss audit matches all 16 rows: every pair, consequence class,
  and safe action is exact; 11 semantic-none prose misses and 8
  path-without-digest misses are retained as directional observations only.
- Protocol 1 semantics are accurately bounded: Repository authority is local;
  only an authorized Decision changes Standing; Submissions, Verification
  Records, projections, and controllers do not acquire authority.
- Portable divergence is described as synthetic test-support evidence without
  global consensus, scientific-truth, or adoption claims.
- The controller claim is limited to one first-party trace and does not infer
  controller authority or general controller safety.
- Erdős 264 is limited to one retained source correction and scoped repair. The
  matched comparison remains 0/1 exact in both arms, and the manuscript does
  not use the later unlimited-heartbeat repair to rescore it. The historical
  source capsule further records that the five direct theorem declarations
  have `sorry` bodies and that the older hosted proof retains the natural-value
  definition; the draft does not claim those five consumers were proved.
- The prospective held-out replication is consistently `not_run`: there is no
  frozen registration, participant call, score, or result in the draft.
- No protected held-out labels or answer key appear in the four added paths;
  the reproduction entry point prints no protected material and makes no
  provider or authority call.
- No Core schema, Protocol object, policy language, Repository authority,
  Decision, Event, or Standing change is present.

## Exact reviewed file hashes

```text
sha256:a05371e945a03a505b830c4fc5ca3e91f72d219e73b351ec64110279f7314f65  paper/flagship/CLAIM_EVIDENCE.md
sha256:273168f45a113b65c6fce52cf2942ae267f9293c0a8238c3079b81936ad9e5aa  paper/flagship/README.md
sha256:5ee7c82d11ff524dbe7792e51deab202a9ad828d22e00006c7f1be04a5f57dba  paper/flagship/manuscript.md
sha256:c816b270ed806952d201954c5236a7c613803ef9ca38e718cff9b5bb0f3ac418  paper/flagship/reproduce.sh
```

The cumulative binary diff stream over the reviewed range has SHA-256
`b1e982b974e8e2d8704c27719ea43b20c86247ffac9fcbbc85a5c6ea81ff3707`.

## Authority and execution disclosure

No model inference, held-out execution, merge, scientific Decision, Repository
authority action, Event, or Standing change occurred. This review does not
reinterpret the sealed negative result and does not authorize any new run.
