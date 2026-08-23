# Final independent review: flagship-paper negative-result reconciliation

## Verdict

**PASS** for exact producer commit
`f09206c9946ce96f4b8d417df8f9ef4ae36dc968`, tree
`d6d70f15db1c19f8d9bd8f4e0fe71ef1871b59ea`, whose sole parent is
`85077212ed1a1465b803fcc904f1a15bc224ca50`.

This PASS qualifies the exact paper reconciliation to the terminal held-out
negative result. It does not authorize or claim a rerun, rescore, provider
call, protected-key access, merge, publication, external reproduction,
scientific acceptance, Protocol or Core change, Repository authority action,
Decision, Event, or Standing change.

## Exact paper scope

The producer ref was fetched independently and resolved to the reviewed commit
and tree. Its parent and exact four-path delta are:

- `paper/flagship/CLAIM_EVIDENCE.md`, SHA-256
  `9ec1e2cb39a57fe943eb8f6830e8aa57dacccf9208ef380aaa076a2d4243397f`;
- `paper/flagship/README.md`, SHA-256
  `9d20ce5077f9e10e82de36235914227cb7ebaf758fc7dd52c7692a470123ba58`;
- `paper/flagship/manuscript.md`, SHA-256
  `5aafa03cb8178d946c41712ee663b2138e2199dad5e47fbcbbfd859ff6590051`;
- `paper/flagship/reproduce.sh`, SHA-256
  `feef443c5e71f55f4cc57588d24eab1638dd0bb68464467f7c216932c0d3b69f`.

No other path changed. The reproduction script remains executable and
`git diff --check` passes.

## Held-out result identities and custody

The paper's identities match the frozen result lineage:

- sealed capture parent `5694bebac03b062d6acdce5a2a900551850e6a1c`,
  tree `feec0ff21b9b13be8cbb97083f441ef66bdd48f2`;
- result producer `4524c8f776943a267e04e03e9a237ecaed14bc2c`,
  tree `4d5650a999ac0be59e71d5bd664e885cad5192c7`;
- independent result review
  `e6d8348bea3a57e88c5f9426d44a480b7a026fbd`.

Independent reconstruction, without scoring or protected-key access, produced:

- complete capture root
  `sha256:4a592d88b43dc02d5495d7679834535d6fa97f20759600400253677a946f87fd`;
- complete custody root
  `sha256:ccf69e70a3887c8a9f9ddffa2d62051e114a8974b2d2ae83c72366a1eb98dcef`;
- score-capture root
  `sha256:f74229b3346cf56e2128d78b366f5fb99380872c27285d196c13862738bc8e98`;
- result-byte root
  `sha256:ae0c980a18633832a83b73e0c715ee11e702aeb56660c4e027d5ece03425f372`;
- canonical result root
  `sha256:92eed5bcb9e6b647d52a53282563077d3829b28c426e0dd9898a073f2590b8a5`.

The 362-entry outer capture manifest exactly covers every retained execution
file other than itself; every length and digest matches. Recomputing
`complete_custody()` from all 36 run directories reproduced the committed
object byte-for-byte. Recomputing the capture snapshot from those exact run
and response bytes produced the score-capture root above.

The execution is exactly 36 strictly sequential attempt-one cells: 12
Git/documents, 12 neutral wrapper, and 12 Vela. All 36 have distinct participant
identities, one turn, one response, and a consumed bound permit. The retained
evidence records zero retries, substitutions, timeouts, tool calls,
compactions, validation errors, or retained credentials.

## Arithmetic, gates, and claim ceiling

The committed result and independent review agree exactly:

| Arm | Sessions | Exact | Impact-complete | Authority errors | Restricted mean seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Git/documents | 12 | 12 | 12 | 0 | 12.800895867 |
| Neutral wrapper | 12 | 12 | 12 | 0 | 13.98268798558333 |
| Vela | 12 | 11 | 12 | 1 | 63.252235329 |

Structure, governance/inheritance, and total gates are all `false`;
`positive_gate=not_supported` and `authority_effect=none`. The manuscript,
README, claim-evidence matrix, and reproduction output consistently present
this as a falsification of the registered positive-lift claims for the fixed
synthetic benchmark. They do not convert the result into a positive claim or
generalize it into a universal Vela disadvantage.

The earlier sealed 16-session result remains separate, unchanged, and negative
at `positive_gate=not_supported`. Its directional audit is not rescored or
promoted into lift. The Erdős 264 case remains explicitly one bounded real
source correction whose matched comparison was 0/1 versus 0/1; it supplies no
causal lift or general scientific claim. The controller evidence remains one
first-party removable trace with authority effect `none`, not a general
controller-safety or productivity result.

The paper makes no scientific acceptance, external-validation, adoption,
general-productivity, Protocol/Core, Repository-authority, Standing, Decision,
or global-truth claim. It does not expose protected adjudication, and the
review did not open a protected key or rescore the result.

## Registry, Frontier, and authority architecture

The reconciled paper consistently describes a global registry/index over
plural Repository-local authority histories. Each Repository retains its own
authorization, Decision boundary, Events, replay, and Standing. Derived global
Frontiers query current Repository-local Standing, own no records, carry no
authority, and cannot reconcile or change local histories. Registry-wide
visibility creates neither isolated-repository semantics nor a single global
truth ledger. The paper adds no protocol object, schema, Core semantic, global
Decision authority, or consensus mechanism.

## Reproduction, links, and render

- All flagship local Markdown links resolve.
- All three Markdown documents parse as GFM with Pandoc 3.9.
- `bash -n paper/flagship/reproduce.sh` passes.
- `./paper/flagship/reproduce.sh --integrity-only` passes and reports both
  negative gates and `authority_effect=none`.
- The full `./paper/flagship/reproduce.sh` passes from the exact producer
  bytes: Protocol 1 conformance, portable divergence 2/2, held-out benchmark
  verification, custody hold verification, held-out tests 24/24 and runtime
  tests 9/9, inherited-correction verification and 16/16 tests, deterministic
  result fixture, and Erdős 264 retained tests 2/2. It reports zero provider
  calls and authority effect `none`.
- Two clean renders with Pandoc 3.9 and pdfTeX
  3.141592653-2.6-1.40.26 (TeX Live 2024) are byte-identical:
  - manuscript source root
    `sha256:5aafa03cb8178d946c41712ee663b2138e2199dad5e47fbcbbfd859ff6590051`;
  - PDF root
    `sha256:77f6ff1d855468bf2b755ad0e7124075fc0ea38f02cea5926f76f7431028cd94`;
  - PDF size 263,743 bytes, seven letter-size pages;
  - source timestamp `1787447411`.
- Visual inspection of all seven rendered pages found no clipping, overlap,
  malformed equations, broken glyphs, or unreadable layout.

## Execution disclosure

No inference, participant run, rescore, adjudication access, protected-key
access, merge, outreach, publication, Core or Protocol mutation, scientific
Decision, Repository authority action, Event, or Standing change occurred.
