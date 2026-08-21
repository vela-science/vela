# Independent review: prospective 36-cell flagship-paper update

## Verdict

**PASS** for exact producer commit
`384d95b742848d6c31d74fe957454dc9aed83f9e`, tree
`342073f4ff74fc6df4e4d99ce39cd476efcb92d8`, parent
`62ecb8ea7771793e9e1232fc99795e65d417dcb7`.

This PASS qualifies only the paper's prospective, explicitly unreviewed and
`not_run` 36-cell design description. It is not a prelaunch method review of
future fixtures, packets, protected labels, scorer, registration, or permits,
and it authorizes no participant execution.

## Exact scope

The producer branch was remote-equal at the reviewed commit. The delta changes
exactly:

- `paper/flagship/CLAIM_EVIDENCE.md`;
- `paper/flagship/README.md`;
- `paper/flagship/manuscript.md`.

The change replaces only the prior unexecuted 24-cell planning language with a
prospective 36-cell, three-family, three-arm nested ablation and its compressed
critical path. The renderer, reproduction script, protocol, Core, schemas,
fixtures, sealed run bytes, scorer, audit, and authority surfaces are
unchanged. `git diff --check` passed. The exact three-path diff stream has
SHA-256
`b7ba9f7e7de77f6c9dc26fee3f52373f7236b5db81daf6426a4bf3f3d088951d`.

## Method and claim assessment

- The design is consistently marked `open`, `unreviewed`, `not_run`, and root
  pending. It invents no registration root, permit-set root, capture, score,
  result, or independent verdict.
- The fixed prospective denominator is internally consistent:
  three unseen correction families by three arms by four fresh participant
  instances equals 36 cells, with 12 per arm.
- The three arms form an honest nested comparison:
  Git/documents (`G`), a neutral typed current/superseded-state and dependency
  wrapper (`N`), and Vela (`V`). The neutral wrapper is expressly prohibited
  from using Repository, Decision, Event, Standing, or authority-replay
  semantics.
- Candidate-visible atomic facts are required to be identical across arms.
  Packet and prompt length must be matched under a frozen prospective rule.
  The manuscript correctly notes that length matching cannot make the distinct
  representations identical.
- The additive estimands are coherent:
  `N-G` for the structured-presentation contrast, `V-N` for the bundled
  Vela-specific governance/inheritance contrast within the frozen design, and
  `V-G` for the total contrast. Their stated additivity is algebraic. The
  future registration must retain the manuscript's limitation that `V-N` is
  design-specific rather than a universal pure-governance effect.
- Authority regimes must be fixed before packet generation and differ across
  families. The paper does not claim an independently identified authority-
  regime effect; regime and family remain bounded by the future registration.
- The future registration must fix primary metrics, favorable directions for
  loss/error metrics, and gates. Restricted-time ratios are explicitly
  secondary and cannot move a primary additive gate.
- Protected labels and action answers remain evaluator-custodied. Only their
  root may be published before execution; the implementation lane may not
  inspect them. The design requires all 36 permits held through an independent
  prelaunch PASS, then single-use consumption with zero retries or
  substitutions.
- The existing sealed 16-session result remains exact and negative:
  `positive_gate=not_supported`, Git/documents 112 points, 0 exact successes,
  8 authority errors, and Vela 130 points, 5 exact successes, 3 authority
  errors. The update neither rescored nor reinterpreted it.
- The no-go language forbids treating the neutral wrapper as Vela, treating the
  prospective design as executed/reviewed/successful, turning the failed gate
  into positive lift, or claiming external validation, adoption, global truth,
  controller authority, or general productivity.

## StateMem citation boundary

The linked primary source is the official
[StateMem arXiv v1 record](https://arxiv.org/abs/2608.19652v1), submitted
2026-08-20 as *Can Agent Memory Systems Track Evolving State?* Its abstract
reports a length- and cost-matched control used to attribute part of a wrapper
gain to state structure rather than added context. That supports the paper's
narrow identification rationale.

The draft explicitly says StateMem's reported results do not count as evidence
for Vela, this benchmark, or adoption. It imports no StateMem result into the
Vela claim matrix and makes no comparative performance claim.

## Reproduction and render

- All local Markdown links resolve, including the unchanged internal links.
  The StateMem link resolves to the official versioned arXiv record.
- All three flagship Markdown files parse to standalone GFM HTML with Pandoc
  3.9.
- `./paper/flagship/reproduce.sh --integrity-only` passed with
  `positive_gate=not_supported`, `authority_effect=none`, and
  `held_out_status=not_run`.
- The full `./paper/flagship/reproduce.sh` passed from a fresh detached
  worktree: Protocol 1 conformance, portable divergence 2/2, inherited-
  correction benchmark verification and 16/16 tests, canonical result
  serialization fixture, and Erdős 264 retained tests 2/2.
- Two clean renders were byte-identical under Pandoc 3.9 and pdfTeX
  3.141592653-2.6-1.40.26 (TeX Live 2024):
  - PDF root:
    `sha256:6961a60750370a1150616ad91408509c37c93b44fc46e07e66e23b05f0a97754`;
  - PDF size: 259,722 bytes, six pages;
  - source timestamp: `1787346342`;
  - manuscript source root:
    `sha256:e1e679d6bf45d2ec823836bd5d5f0c28d79d491841c91f6aa0f4f46c1fc89d02`.

## Source hashes

```text
sha256:ed153189d12976269ec42ce0a59acd45954b19f9808eb6815981c80e56499766  paper/flagship/CLAIM_EVIDENCE.md
sha256:e9a47f90cb8b74dd0a1f0df227bca617cb80f0f9fafdda834a460591b453eb52  paper/flagship/README.md
sha256:e1e679d6bf45d2ec823836bd5d5f0c28d79d491841c91f6aa0f4f46c1fc89d02  paper/flagship/manuscript.md
```

## Critical-path assessment

The arithmetic is internally consistent: fastest stages total 26--36 hours;
expected ranges total 3.5--7 days. Eight to twelve hours for sequential
36-cell execution is credible within the frozen 600-second cap plus custody
overhead. The table expressly assumes no infrastructure failure and only one
narrow correction cycle per review boundary. External reproduction adds zero
time because it is correctly classified as downstream validation rather than
an internal paper-ready prerequisite.

## Execution and authority disclosure

No inference, participant call, permit consumption, protected-key access,
outreach, merge, scientific Decision, Repository authority action, Event, or
Standing change occurred.
