# Independent final-runtime neutral calibration confirmation

## Verdict

**PASS**, bound to producer evidence commit
`53d3015f3a8e705ecffa610e60ea748963c80995`, tree
`f05cf29a189cf769bc0b2b6a00e88c96ba3ce520`, parent
`04717caf38ca2581aca5c9905baf14ed9c2a21e0`, and live remote branch
`refs/heads/codex/inherited-correction-study` at that exact commit.

The frozen evidence establishes exactly one neutral, non-scientific provider
call on attempt 1 against the independently passed final cross-day runtime.
The distinct calibration permit was atomically consumed before provider start,
the returned response is the exact preregistered response, all receipt and
runtime bindings recompute, and teardown is clean. No participant session
started. The participant study remains 0/36 with all 36 participant permits
held and unconsumed.

This review made no provider call, released no permit, accessed no protected
adjudication plaintext or key, performed no scoring, and authorizes no
participant launch, merge, Core or Protocol change, Repository authority
action, Standing change, or Decision effect.

## Commit and scope

The review used an isolated clean clone and refreshed the live remote ref. The
commit, tree, parent, and remote matched exactly. The producer delta adds nine
files under only
`paper/artifacts/inherited-correction-held-out-replacement-calibration-execution/neutral-schemafix-calibration-01-crossday-04717caf/`.
It modifies or deletes no pre-existing byte.

The complete held replacement artifact has Git tree
`2fd10305806d644f4b33c57971c28912b4b78593` in both parent and producer. The
earlier independently passed neutral-calibration evidence has tree
`7045aa36d1ebd1c153b48544703a35b07b2c386a` in both. The stopped original
execution and original held artifact likewise retain their exact parent tree
identities. Therefore participant packets, prompts, both schemas, study design,
model configuration, gates, scientific bytes, protected commitment, stopped
evidence, Core, Protocol, Standing, authority, and all prior calibration bytes
are unchanged.

## Calibration custody

All eight terminal evidence files and six frozen input bindings match their
recorded byte counts and SHA-256 digests. Removing only the self-field and
recomputing the canonical manifest gives custody root
`sha256:9f2f97766c2bd2d04e7fb3b99b69bd831a8e11c8c719ab60d535dabf6f2d19df`.

The consumed permit canonical root is
`sha256:007795635b658e136e3a7e3c35d8f56920916ba4304a236eba2890a548c52e7a`.
After excluding only the mutable issuance fields `status` and `expires_at`, its
identity equals the frozen held calibration permit exactly. It binds run
`neutral-schemafix-calibration-01`, participant identity
`neutral-schemafix-sol-01`, condition `neutral-calibration`, and attempt 1.

The launch binds the consumed permit bytes at
`sha256:67cf3e74b1f7bfe4abaee741bc872577899dad0bebdfaf1a8baa73ea3ef6b85c`.
The unchanged reviewed runtime atomically renames the issued permit to the
consumed path before launch recording or provider start. The recorded provider
start is one millisecond after consumption, and completion precedes permit
expiry. The execution evidence contains one consumed calibration permit and no
unconsumed calibration permit.

## Exact provider evidence

The event stream contains exactly four events: one thread start, one turn
start, one agent response, and one turn completion. It contains no tool,
command, patch, file-change, search, computer-use, compaction, continuation, or
retry event. The only agent message parses to the exact retained response.

- provider events bytes:
  `sha256:6b0bc40ab39d08b25da5b4848f647bea642ed8725ee5cb695d7f7c82b5eac52b`;
- provider stderr bytes: the empty-file root
  `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`;
- response bytes and canonical expected-response root:
  `sha256:9748c08fee1cc90f44c8c50bfd28e8911edd1ca819a0f0a224e209cb04158372`;
- terminal receipt bytes:
  `sha256:b93531a77f63b515ea3e7154922b733c46e95431e0f7c9263b1873a58b671e95`.

Recorded usage is 6,765 input tokens, zero cached input tokens, zero cache-write
input tokens, 374 output tokens, and zero reasoning-output tokens. The process
exited zero without timeout after one turn, with zero tools, compactions,
retries, or substitutions. The retained evidence contains no
credential-shaped material.

## Runtime and schema bindings

The receipt, permit, result, and frozen inputs agree on these independently
recomputed bindings:

- image manifest: `sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1`;
- image config: `sha256:0b41c9eb78b4afcd34b8e6c8c3bf85d81eda431fa4f7f99445c6d951eaa49348`;
- complete OCI tar: `sha256:87a1b1d80a27dbc92a0fd5dd69543c4c55386d3cfef77e7c76dab37d2c905183`;
- runtime root: `sha256:3f7a753141306771b05c582d1c0ff30489cdb8a35c556e21ac5fdabb9a431ba8`;
- runtime source: `sha256:163f0bab3459e95f59ef503a4105600c9ee096dd16745c3187982a104e731971`;
- registration: `sha256:820b725d04cd3780e4bbdb6a89f3ee980a5bf993259c1f089984a3e7f7407f2b`;
- calibration assignment: `sha256:953e6a92190480949ffbbec00d2ffe5595b249a2a6cfd5b53b1260e9bd374774`;
- calibration configuration: `sha256:b2ee95d14d17f950eec5d433013cdf163568f456b9580379bd8b8290d0a8728c`;
- registered schema: `sha256:ac96be686e749792956dfa1dfe9560f85c53d55c27fe2e8fd32bcc2a96a634ba`;
- provider derivative: `sha256:896f242086805d3b51e81ed04e6d50f33eb2b7deb71b7a1689e9abeba3b67eaf`;
- prompt: `sha256:eed8ad5804e999b60457ae959b316a5047c1b310661dcb59a4a7460b1292396e`;
- neutral packet: `sha256:4c94831df6a848e1685b47d8d59714064a377d7feb4fdb366450feb5a4491f1c`.

The exact response passes the unchanged registered schema and its exact
provider derivative. A fresh `--network=none`, read-only, ephemeral execution
inside the bound image independently confirmed valid-response acceptance by
both schemas, provider-only duplicate acceptance, registered-schema duplicate
rejection, no provider-contact possibility, and empty events and stderr.

## Teardown and held study state

The reviewed launch contract uses an ephemeral `--rm`, read-only container and
a read-only credential mount. No container with the calibration launch name
remains. Receipt, result, and custody manifest record clean teardown and no
retained credential.

The calibration has no denominator credit and no Standing or authority effect.
The replacement participant result remains `not_run`, 0/36. Exactly 36
participant permit templates remain `held` with `expires_at=not_authorized`;
no replacement participant consumed-permit or replacement participant
execution evidence exists. Recorded replacement participant provider calls,
protected-adjudication accesses, and scoring runs are zero.

## Focused checks and adversaries

The benchmark verifier, prelaunch custody verifier, all 30 focused Python
tests, Ruff, network-none schema preflight, manifest/file/root recomputation,
and `git diff --check` pass. Independent mutations for a duplicate turn event,
a forbidden tool event, a missing terminal event, participant/permit identity
drift, and a duplicate registered-schema binding all fail closed.

## Residual boundary

This PASS confirms only the terminal neutral calibration at the exact producer
evidence commit. It neither releases nor authorizes any participant permit.
Participant execution remains a separate explicitly authorized action.
