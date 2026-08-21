# Final independent review: flagship-paper terminal reconciliation

## Verdict

**PASS** for exact producer commit
`85077212ed1a1465b803fcc904f1a15bc224ca50`, tree
`dcc661bc3c77dedce5cf8f3a8ef96edab66ee75f`, parent
`384d95b742848d6c31d74fe957454dc9aed83f9e`.

This PASS qualifies only the terminal paper reconciliation. The held-out study
remains prelaunch-qualified, `0/36`, and `not_run`. This review authorizes no
adjudication freeze, protected-key access, permit release, participant or
provider call, scoring, result, merge, Core or Protocol change, scientific
Decision, authority action, or Standing change.

## Exact paper scope

The producer ref was remote-equal at the reviewed commit. The delta changes
exactly:

- `paper/flagship/CLAIM_EVIDENCE.md`;
- `paper/flagship/README.md`;
- `paper/flagship/manuscript.md`.

The renderer, reproduction script, protocol, Core, schemas, scientific
records, sealed 16-session evidence, and held-out implementation bytes are
unchanged. `git diff --check` passed. The exact three-path diff stream has
SHA-256
`7aa14650bb5a7bcf70119ac19fc18df0293d5ac09fa97eb8c48a61d5cf17a4f8`.

## Held-out identity and custody reconciliation

The paper's new identities match the immutable held-out producer and its
independent review:

- producer commit:
  `8cc1a89d7b1ae47cb6cabb36bfd79b46c3f4db81`;
- producer tree:
  `98b661c44225425ababecdbb7aead0090d09a4f7`;
- preregistration root:
  `sha256:185e781cd0b1a06d89488266e9e7147f42834d960063818f0cdf56209c6d3306`;
- 214-entry artifact root:
  `sha256:17f113d16aa7b474d91b9f09e4314dce133367b7274187ce7bef87a1bbf7c735`;
- independent PASS commit:
  `f9b5d67a55c1ad41fcb67cc1d7ebe86d03d37782`, tree
  `b7742e7bf67db630ead4faf580925a8026f7b599`.

Independent reconstruction from a detached checkout of the held-out producer
passed `benchmark.py verify` and `custody.py verify-prelaunch`. The artifact
manifest contains exactly 214 entries and its canonical root matches the paper.
All 36 permit templates have status `held`; zero consumed-permit files exist.
The result remains:

```json
{"fixed_denominator":36,"positive_gate":"not_evaluated","sessions_completed":0,"status":"not_run"}
```

The adjudication commitment has a null root,
`answer_bytes_present_in_producer_artifact=false`, and status
`pending_independent_evaluator_freeze`. The prelaunch freeze reports zero
provider calls. The independent PASS explicitly supplied no permit release,
provider, scoring, merge, Core, Protocol, authority, Standing, or Decision
authorization.

## Paper status and remaining decision

- `prelaunch-qualified` is defined as reviewed registered bytes and held
  custody without execution or result authority.
- C7, the README, methods, results, artifact manifest, limitations, no-go
  section, paper-ready gate, and critical path consistently report `0/36`,
  `not_run`, 36 held permits, zero consumed permits, and adjudication pending.
- The sealed 16-session result remains unchanged at
  `positive_gate=not_supported`, Git/documents 112 points, 0 exact successes,
  8 authority errors, and Vela 130 points, 5 exact successes, 3 authority
  errors. It is neither rescored nor promoted into positive lift.
- The paper identifies exactly one remaining user decision: separate explicit
  authorization for the independent evaluator to freeze and bind protected
  adjudication and then execute this exact registered 36-cell study under its
  frozen zero-retry, zero-substitution custody. It does not propose a duplicate
  study.
- No result may be reported until all 36 terminal captures freeze and receive
  independent post-result review.

## Registry, Frontier, and authority architecture

The paper no longer frames Vela as a collection of isolated repositories. It
states a global discovery/indexing registry over plural Repository-local
authority histories:

- every Repository retains its own authorization, Decision boundary, Events,
  replay, and Standing;
- global Frontiers are derived queries across current Repository-local
  Standing;
- Frontiers own no records and carry no authority;
- registry-wide visibility cannot reconcile or change local histories;
- neither the registry nor a Frontier constitutes a single global truth
  ledger.

This is explanatory paper architecture, not a new Core object, global Decision
authority, consensus mechanism, or Protocol semantic. The exact three-path
change adds no schema, code, authority record, Event, or Standing transition.

## Reproduction and render

- All local Markdown links resolve.
- All three flagship Markdown files parse to standalone GFM HTML with Pandoc
  3.9.
- `./paper/flagship/reproduce.sh --integrity-only` passed with
  `positive_gate=not_supported`, `authority_effect=none`, and
  `held_out_status=not_run`.
- The full `./paper/flagship/reproduce.sh` passed from fresh detached bytes:
  Protocol 1 conformance, portable divergence 2/2, inherited-correction
  benchmark verification and 16/16 tests, deterministic post-result fixture,
  and Erdős 264 retained tests 2/2.
- Two clean deterministic renders were byte-identical under Pandoc 3.9 and
  pdfTeX 3.141592653-2.6-1.40.26 (TeX Live 2024):
  - PDF root:
    `sha256:e632861832163c2c18559ff677ba62173b26ba8e8bce33a67939e91b81b4951d`;
  - PDF size: 261,237 bytes, six pages;
  - source timestamp: `1787348062`;
  - manuscript source root:
    `sha256:0f0757a44b96dec583b2be54fb075643cd358fdcd8d0f6630277d95ea4a39611`.

## Source hashes

```text
sha256:6b8966796efbfa72d2380f92abe17bc623a33e0fc9391c54fd7752e12e958b8f  paper/flagship/CLAIM_EVIDENCE.md
sha256:bcc4ae1f3bceeb46e07dba78cc01a3d0bd440d0463318281e35304b70f3d79c2  paper/flagship/README.md
sha256:0f0757a44b96dec583b2be54fb075643cd358fdcd8d0f6630277d95ea4a39611  paper/flagship/manuscript.md
```

## Execution disclosure

No experimental call, adjudication freeze, protected-key access, scoring,
permit mutation, outreach, merge, Core or Protocol change, scientific
Decision, Repository authority action, Event, or Standing change occurred.
