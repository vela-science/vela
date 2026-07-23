# Vela quickstart

Vela is version control for scientific state. Git publishes exact bytes;
agents produce Receipt v1 evidence; frozen verifiers reproduce it; a signed
policy or protected human decision admits an exact transition.

## Read an existing Frontier

```bash
git clone <frontier-url>
cd <frontier>
vela check . --strict --json
vela status . --json
vela reproduce .
```

For Profile v1, strict checking verifies more than event replay: it validates
the closed profile and settings, signed repository-boundary chain, exact Git
anchors and ancestry, retained canonical bytes, actor registry, and the
consumer's independent first-boundary pin whenever such a boundary exists. If
that pin is missing, obtain the full first-boundary content root through an
independent trusted channel and use `vela frontier trust pin`; never copy a pin
asserted by the checkout itself.

## Produce one bounded result

```bash
vela next . --limit 1 --json
vela work <target> --as agent:<name> --json

# Run the exact verifier and retain its artifact.

vela land --work <target> \
  --claim "<bounded result>" \
  --type computational \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --as agent:<name> \
  --json
```

`next` and `work` consume the fresh Target Index v2 and exact target packet.
The claim retains its target-task binding in Receipt v1. A verifier pass is
evidence, not acceptance. `Deferred` means the proposal awaits a protected
human decision; a policy admission means an already human-signed policy
authorized that exact class.

## Create a new Frontier

```bash
vela init ./frontier \
  --name "Bounded question" \
  --scope "Does the selected finite claim hold?" \
  --json
```

The result is a minimal Profile v1 repository with structural identity and no
administrator authority. A human steward then protects their identity,
bootstraps the exact empty actor registry, commits that delta, and runs the
two-phase `vela frontier bind` ceremony. See
[SIGNING.md](SIGNING.md#first-repository-administrator).

## Migrate a legacy Frontier

Profile v0.1 remains readable but is not a canonical writer in Vela 0.914.
Migration is a protected two-phase operation requiring external candidate
profile and target-index files:

```bash
vela migrate . --to frontier-repo-v1 --check \
  --profile ../profile.yaml \
  --target-candidate ../target-index-candidate.json \
  --as reviewer:<administrator> \
  --reason "Bind exact legacy repository" \
  --json
```

Inspect the plan before invoking the matching `--apply` command with its
`--confirm-root` and `--confirm-at`. Migration preserves all pre-boundary
canonical and evidence bytes and appends one signed non-scientific boundary.

## What to read next

- Producers and agents: [AGENT_QUICKSTART.md](AGENT_QUICKSTART.md) and
  [PRODUCER_QUICKSTART.md](PRODUCER_QUICKSTART.md)
- Commands: [CLI.md](CLI.md)
- Repository layout: [FRONTIER_REPOSITORY_PROFILE.md](FRONTIER_REPOSITORY_PROFILE.md)
- Human custody and approvals: [SIGNING.md](SIGNING.md)
- Byte and root meanings: [ROOTS.md](ROOTS.md)
- Protocol semantics: [PROTOCOL.md](PROTOCOL.md)
