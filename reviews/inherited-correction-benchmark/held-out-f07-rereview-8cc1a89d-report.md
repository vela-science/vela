# Narrow F07/G06/G08 held-out prelaunch re-review

## Verdict

**PASS**, bound to corrective producer commit
`8cc1a89d7b1ae47cb6cabb36bfd79b46c3f4db81`, tree
`98b661c44225425ababecdbb7aead0090d09a4f7`, whose sole parent is blocked
producer `b1528898e5877a5dd0c863a28db71c9fd5623f60`.

This closes F07 and restores support for `G06_cold_successor_protocol` and
`G08_deterministic_custody`. The exact held-out runtime now binds independent
review `3c0c8cfa050b30a5d19c9e7e623fc549ac18264b`, whose machine verdict PASSes
producer `2fc59d5f57e45298f833e65f123ac9eafea2810b`, image
`sha256:1dee2374077c83e3dbdb2e09d32ef4fa3a414d200b800839857353e13d3c4e09`,
runtime-source root
`sha256:398f798daf4b2ebd86a878021025adbc073155e13d9123b140da2bc8fcb32b8a`,
and the locked Ajv Draft-2020 validator.

This PASS is narrow and non-authorizing. It does not freeze or expose the
pending protected adjudication, release or consume a permit, authorize a
participant/provider/model call, authorize paid inference, merge the branch,
or establish a result, lift, scientific acceptance, Protocol/Core validity,
authority, Standing, or a Decision effect. Status remains 0/36, `not_run`.

## Exact corrective scope

The pushed producer ref reconstructs exactly at the handed-off commit, tree,
and parent and is remote-equal. Live `origin/main` remains independently at
`4685462c44b1f073870f31025ae73d1d8770ce73`.

The corrective diff contains 61 modified files, all under
`paper/artifacts/inherited-correction-held-out/`. The only authored runtime
change is:

```text
runtime_review_commit:
  2ebf1ad8cb0f5d16b7bcee8e5510f3aed5dc1395
  -> 3c0c8cfa050b30a5d19c9e7e623fc549ac18264b
```

The other 60 files are deterministic dependent outputs: the study/runtime
roots, registration-bearing assignment and condition-configuration envelopes,
configuration mapping, 36 held permits, prelaunch freeze, and manifest.

Independent byte comparison against the blocked parent found no change to any
participant packet member, `prompt.txt`, or response-schema input. The family
source, seed, benchmark implementation, custody implementation, tests,
input-equivalence object, adjudication commitment, and held result are also
byte-identical. Therefore the families, facts, authority regimes, assignment
cells, scoring, gates, custody behavior, and claim ceiling are unchanged.

## Recomputed bindings

Independent recomputation matched the disclosed amended identities:

- runtime-binding bytes:
  `sha256:529e12937c649f4767347853d45826c673be40968375e21e15589e9078993bcd`;
- runtime root:
  `sha256:33f0d3b40e674a4c0934f27080e28325be9a30edfbd11682801066c22911ee6a`;
- preregistration bytes and root:
  `sha256:3e9b315166a3833e0844b46db6df3256a2bc229570f2056e4cf65301b72f83a9`
  and
  `sha256:185e781cd0b1a06d89488266e9e7147f42834d960063818f0cdf56209c6d3306`;
- assignment bytes and root:
  `sha256:117b83d0577c7905bc738d1be8a1b985753f2be4393ea4e8396dd728feb443cd`
  and
  `sha256:77c81b88813716466f5c935eb79b986874ed132a5986be34152bd8ac70da0205`;
- shared configuration root:
  `sha256:adb7aef1966a25631077dd3466a04a785e369698fdbae0e8f09ace5ca995380e`;
- configuration-mapping root:
  `sha256:709713f1d27852cbf3b5169091c476b55b490529eeed84f8aac2decf01fecb6d`;
- unchanged equivalence root:
  `sha256:12904756aa4683934eb925ae856d6afd50897dc1d855f3b55ce4e51ce6391bc1`;
- permit-set root:
  `sha256:e7f9ce0725c82250927355541ca1651fc44b4d925255e57481d0e2b4f85bb438`;
- prelaunch bytes and root:
  `sha256:e6f575d4a778f88e0cea8877a7517c88c982e34ee2744a822ead08375cb49d13`
  and
  `sha256:f1dc641ad0dac285dd94f84d37746a34fac6f30d05e8ee3d773599b01fb32ea8`;
  and
- manifest bytes and 214-entry artifact root:
  `sha256:200cf5deb246d27d2e689af4730c69269f5dd14189efbfc680d74b12344cf835`
  and
  `sha256:17f113d16aa7b474d91b9f09e4314dce133367b7274187ce7bef87a1bbf7c735`.

The held result remains byte-identical at
`sha256:73c682355f7e5c03362fe256bb33eb8273ddeff4a3db80f41e3e68746fac3797`.
All 36 permits remain `held`; no consumed-permit file exists. The adjudication
root remains null with answer bytes absent and release forbidden pending an
independent evaluator freeze and another prospective review.

## Deterministic reproduction

The following checks passed from a fresh detached checkout of the exact
corrective producer:

- Ruff 0.12.11 format and check on the three held-out Python files;
- held-out verification, prelaunch verification, and all 18 tests under
  CPython 3.10.8, 3.11.2, 3.13.13, and 3.14.4;
- isolated CPython 3.14 regeneration of all 215 artifact files, byte-identical
  with manifest SHA-256
  `200cf5deb246d27d2e689af4730c69269f5dd14189efbfc680d74b12344cf835`;
- prior inherited-correction verification and all 16 tests;
- exact participant packet/prompt/schema no-diff guard against blocked parent;
  and
- `git diff --check`.

No provider call, protected-key access, score, permit mutation, inference,
merge, authority action, Decision, or Standing mutation occurred during this
re-review.
