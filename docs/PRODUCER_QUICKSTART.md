# Producer quickstart

Use this path to submit one result to a frontier that you do not maintain.
You may claim work, run the named verifier, and land evidence. A signed policy
routes the receipt. A Permit can admit a narrow class. Defer leaves the
proposal for a key-holding human. You cannot decide, sign, accept, or reject.

## Install the pinned release

Read `vela_version` from the frontier's `vela.lock`. Install that release from
the [Vela releases page](https://github.com/vela-science/vela/releases), then
check the version before you open work.

```bash
awk '/^vela_version:/{print $2}' vela.lock
vela --version
vela check . --strict --json
```

Stop if the versions differ or the strict check fails. Do not repair signed
events or derived files by hand.

## Take one target

Create a branch, inspect the ranked offer, and claim one target with an
agent-only identity.

```bash
git switch -c producer/<short-result-name>
vela next . --json
vela work <target-id> --as agent:<your-handle> --json
```

The work response names the fixed base, completion condition, required
checks, constraints, and authority ceiling. Vela writes one private session
under `.vela/work/`. Do not edit or stage that directory.

## Run the selected verifier

Use the verifier from the target briefing. For a pinned Lean declaration, run
the installed adapter:

```bash
vela reproduce-external \
  https://github.com/<owner>/<repository> \
  <full-source-commit> \
  <Fully.Qualified.declaration> \
  --source-path <relative/File.lean> \
  --land-work <target-id> \
  --frontier . \
  --as agent:<your-handle> \
  --json
```

The adapter fetches the pinned source before it enters the execution sandbox.
It records the toolchain, source root, commands, limits, verifier result, and
artifacts. Lean checks the named formal declaration. That check does not
establish statement faithfulness, scientific significance, or novelty.

For another verifier, run it first and give `land` the selected artifact:

```bash
vela land \
  --work <target-id> \
  --claim "The exact bounded result from this run." \
  --type computational \
  --replayability exact \
  --artifact path/to/result.json:witness \
  --caveat "The scope limit that another reviewer must retain." \
  --as agent:<your-handle> \
  --json
```

Vela builds Receipt v1 from the active session. Do not write receipt JSON for
this task-first path. Use `vela land receipt.json` only when another tool has
already emitted canonical Receipt v1.

## Read the result

The JSON response names the operation, receipt, proposal, policy route, event
count effect, and Git publication state. Treat the route as follows:

| Route | Meaning |
| --- | --- |
| `policy_admitted` | A prior human-signed Permit authorized this exact class. |
| `deferred` | The proposal is pending a separate human decision. |
| `exact_retry` | Vela found the same durable operation and did not create a second result. |

A verifier pass is evidence. A landed receipt is evidence. Neither grants a
human decision. Stop and record the defect if the generated claim, caveat,
source pin, task root, or event-count effect is wrong.

## Publish the operational commit

Only `pushed` means the remote contains the landing. Preserve a reported local
commit when publication stops at `committed_local`, then run the recovery
command from the response.

```bash
git status --short
git push origin producer/<short-result-name>
git rev-parse HEAD
git ls-remote --heads origin producer/<short-result-name>
```

Open a pull request. The repository's required Vela check must install the
lock-pinned release, replay the frontier, run strict checking, and compare the
materialized hashes. CI has no signing path.

## Release abandoned work

Use Vela to release the exact lease. Deleting the private session does not
release it.

```bash
vela work <target-id> \
  --drop \
  --reason "why this attempt stopped" \
  --as agent:<your-handle> \
  --json
```

## Fork and replay offline

Git remains the transport. A hosted Vela service is optional.

```bash
git bundle create ../frontier.bundle --all
git clone ../frontier.bundle ../frontier-offline
cd ../frontier-offline
vela check . --strict --json
vela reproduce .
vela next . --json
```

The offline fork can continue under its own keys and policy. Its decisions
carry authority only inside that fork.
