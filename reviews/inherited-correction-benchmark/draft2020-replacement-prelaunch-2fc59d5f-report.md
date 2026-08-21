# Independent Draft-2020 replacement prelaunch review

## Verdict

**PASS**, bound to producer commit
`2fc59d5f57e45298f833e65f123ac9eafea2810b`, tree
`29ff77485baf47d8c563400a71313e4502120071`, whose sole parent is the stopped
run commit `3931477893bd92015590a89a71129b581cc06ea3`.

This PASS qualifies only the prospective Draft 2020-12 validator repair, its
pinned container image, the immutable stopped-registration disposition, and
the fresh replacement registration/custody package at 0/16. The previously
passed scorer/custody implementation remains bound to producer
`7596c12291c22a5b4b81a1ab1eb49189318f57de` and independent review
`59c104efefd163d2e4c86e1bd535ac5f7c03f17d`.

This review does **not** release either replacement hold, authorize a permit
consumption or participant/provider/model call, revive any stopped-registration
permit, merge the branch, establish lift or scientific acceptance, or create
any Protocol, Core, Standing, authority, or Decision effect. Replacement
status remains 0/16, `not_run`; all replacement permits are unused and both
holds remain `hold`.

## Stopped-registration disposition

The original registration is closed rather than silently resumed:

- the exact run and capture trees are byte-identical to parent commit
  `3931477893bd92015590a89a71129b581cc06ea3`;
- `confirm-run-01` / `confirm-sol-01`, assigned to Vela, is the sole issued and
  terminal cell;
- its provider response, four provider events, receipt, consumed permit, and
  bridge record are retained exactly;
- the receipt is `non_result` and the bridge record is `failed` because default
  Ajv could not compile the registered Draft 2020-12 schema;
- attempt is one, with zero retry and zero substitution;
- the response was neither scored nor reinterpreted, and has no replacement
  denominator credit;
- `confirm-run-02` through `confirm-run-16` are enumerated as unissued and
  forbidden; the original study hold remains `hold`;
- protected-adjudication access count is zero and score status is
  `not_run_and_forbidden`.

The stopped record recomputes exactly to root
`sha256:d89fc730a9b9dc416996acb88e8e2156a39064f0c4dd6d6150c1db7b3af13e79`
and bytes
`sha256:fce69930e68f73262912f73d4dbd8478d908af1d6db656427f0e1a5da26206d3`.
Independent regeneration reproduced it byte-for-byte from the unchanged run and
capture evidence.

## Minimal Draft 2020-12 repair

The scientific task, response schema, packets, prompts, scorer, positive gate,
model, tool policy, timeout, and output ceiling are unchanged. The runtime
change replaces default `ajv` construction with the locked Ajv 8.17.1
Draft-2020 implementation at `ajv/dist/2020.js`.

The shared `compileResponseSchema` function is called by the actual
`run-once.mjs` participant entrypoint and by the offline preflight. It compiles
the exact unchanged Draft 2020-12 response schema under strict mode. Independent
container checks with `--network=none`, a read-only filesystem, no auth mount,
and no provider events established:

- the valid fixture passes;
- unknown fields, missing required fields, invalid enums, and invalid shapes
  fail;
- the exact retained stopped-run provider response is structurally valid under
  the repaired validator, without scoring or reinterpretation; and
- stderr and provider-event files remain empty.

The validator amendment root is
`sha256:ff22dbd04eec46237f2a8c135a647ffdf2c7947e365dbf7674f0a22d8b8edc2e`;
the offline schema-preflight root is
`sha256:6a3b185e7c14d9cb06365890c72ecb6d45f9cf891c3b6f983ad85846390177bf`.

## Pinned image

Two independent invocations of the registered build command,
`docker build --pull=false --provenance=false`, under separate tags resolved to
the same Linux/arm64 image:

`sha256:1dee2374077c83e3dbdb2e09d32ef4fa3a414d200b800839857353e13d3c4e09`.

The pinned base remains
`sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85`.
An offline image inspection reproduced CA-bundle SHA-256
`714d457d580922dbf1d0be8bd35ba236a842b50b0072ae791582a19adef772a5`
and `codex-cli 0.149.0`. The recomputed runtime-source root is
`sha256:398f798daf4b2ebd86a878021025adbc073155e13d9123b140da2bc8fcb32b8a`.

## Fresh replacement registration

The replacement does not reuse an original run or participant identity. The
fresh seed commitment
`sha256:46aed8a87244d03f7edb8347cc0c7edf114c9c626d0595d80fbc46b92b0059c3`
reproduces the committed schedule exactly: 16 unique `replacement-run-*`
identities, 16 unique `replacement-sol-*` identities, eight Git/documents and
eight Vela cells, fixed before any replacement output.

The two condition configurations are identical except for their registered
prompt roots. They retain Sol high/default, Codex CLI 0.149.0, one prompt, one
model turn, no tools, 600 seconds, output ceiling 8192, attempt one, and zero
retries or substitutions. Packet and prompt roots are unchanged from the
stopped registration.

All 16 replacement permits bind the exact replacement run, participant,
condition, assignment, condition configuration, prompt, packet, image, trust,
and attempt. No consumed permit or capture exists; provider calls are zero,
`permits_consumed` is empty, both hold files are `hold`, and the scheduler is
`none`.

Independent recomputation matched the disclosed roots:

- benchmark registration:
  `sha256:a8b4b729e7b2be4f371d4831caa9214959514cb51222da9e422b0634c408a575`;
- replacement registration:
  `sha256:988326b2f9ef7232795a73070823993251fa0481450a792b51905ce85d7e31b4`;
- prelaunch canonical root:
  `sha256:af17487d7fe3abbaef914215c9d832bc9f167754f59337ff748254c140b30b95`;
  and bytes
  `sha256:7cdbaab7b3b4749684bf45acb11caf6ee93dae13f6de04aa9eff1fe19cf293d3`;
- 36-entry artifact manifest:
  `sha256:a80a551fa00229ff29f45da8c23862cc78c47c68a42ca0c80ee72bcac84cf38f`;
- assignment:
  `sha256:c17d9b6860f6fb9f4aba352864ac66db0ad8f87f19d01ac14103f8fdf7a16c64`;
- authorization:
  `sha256:9dee4bdaa818313a20f09209c2647d022ce08b456681dbe6607ccd0a22e02041`;
- shared participant configuration:
  `sha256:f808a660fc663245cf67c298cf1ff53a4b35402fd704e1281d03a83d567319ff`;
- Git/documents and Vela condition configurations:
  `sha256:06bd82bdf59e48df0103400e0b04e51e04a943b9887d188130c80fb914217f1a`
  and
  `sha256:3055c1d82860faae76dc690a8099281e7c88177a779d38b3a6634a86043c9744`;
- authorized mapping:
  `sha256:fc83efbb02022a156f71e771db17d002ba93e8ac876b42f9dc62c66d505732f8`;
- permit set:
  `sha256:927dcd2af9069a2c47c3d4f530e34de339cc39c7f90de379f0ad893ecc1fe2f6`;
  and
- scoring bindings:
  `sha256:6538c1dd62bdfc0421258a1da0b5217ebd5627dd7329a3a7d8727420b14c13af`.

An isolated replacement freezer invocation reproduced all 36 manifest entries
plus `prelaunch-freeze.json` byte-for-byte, 37 files total.

## Immutable scope and deterministic checks

The pushed ref resolved exactly to the handed-off commit, tree, and parent. Its
59-path diff is confined to the inherited-correction benchmark/execution
artifacts. The stopped run/capture, canaries, pilot, original study, task
packets, response schema, scorer implementation/tests, positive gate, and
protected adjudication bytes are unchanged. Live `origin/main` independently
resolved to `4685462c44b1f073870f31025ae73d1d8770ce73`; no merge-compatibility claim is
made.

The complete disclosed command set passed from a fresh detached checkout:

- two exact pinned Docker builds;
- offline CA-bundle and Codex-version checks;
- Ruff 0.12.11 check and format check on the eight specified Python files;
- locked event/schema contract wrapper;
- 12 container-runtime tests;
- 10 replacement custody tests, including immutable pre-key scoring snapshots;
- 9 replacement prelaunch/stopped-registration tests;
- replacement custody prelaunch verification;
- benchmark verification;
- 16 benchmark tests;
- isolated byte-exact replacement and stopped-record regeneration; and
- `git diff --check`.

No replacement participant, provider/model call, paid inference, auth mount,
permit consumption, canary rerun, protected-adjudication read, real scoring,
human study, merge, authority action, Standing mutation, or Decision was
performed during review.
