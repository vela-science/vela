<p align="center">
  <img src="docs/assets/canopus-readme-hero.jpg" width="960" alt="One bounded Canopus Run moves from a rooted target through an isolated worker and verifier to an optional Vela Submission." />
</p>

<p align="center"><strong>Bounded research for Codex.</strong></p>

<p align="center">
  Run one exact mission. Reproduce the artifact. Submit only when you choose.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@vela-science/canopus"><img alt="npm" src="https://img.shields.io/npm/v/@vela-science/canopus?style=flat-square&color=C9A664&labelColor=081224" /></a>
  <a href="https://github.com/vela-science/vela-research-harness/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/vela-science/vela-research-harness/ci.yml?branch=main&style=flat-square&label=build&labelColor=081224" /></a>
  <a href="LICENSE-APACHE"><img alt="Apache-2.0 OR MIT" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-4F8F8B?style=flat-square&labelColor=081224" /></a>
</p>

Canopus is a removable research runner over Vela and Git. It gives Codex one
rooted target, freezes the produced bytes, runs a separate verifier, and
reproduces the result from a clean clone.

A Run is nonmutating. It does not create a Proposal, Verification Record,
Decision, Event, or scientific Standing. A successful Run may be exported as an
authenticated `vela.submission.v1`; only the separate `submit` command registers
that Submission as pending review.

## The loop

```text
doctor → run → show → replay → export → submit
```

```sh
bun install --frozen-lockfile
bun run build

canopus doctor /path/to/frontier
canopus run /path/to/frontier --first
canopus show latest
canopus replay /path/to/run.json
canopus export /path/to/run.json --output /path/to/submission-bundle
canopus submit /path/to/submission-bundle /path/to/frontier
```

- `doctor` binds the exact frontier, target, Vela, Codex, profile, packet, and
  verifier identities.
- `run` executes in disposable workspaces and leaves the frontier unchanged.
- `show` inspects current and historical run records.
- `replay` reruns the frozen verifier without a model call.
- `export` creates a signed portable Submission and retains no producer key.
  If a retained Run from an older worker contract still says verification is
  pending after its verifier passed, export fails closed until the producer
  supplies one corrected bounded Claim and explicit scope limit.
- `submit` explicitly registers that Submission through Vela. The expected
  result is `pending_review` with accepted-event delta zero.

`inspect`, `--no-land`, Receipt authoring, and automatic landing are not current
interfaces.

## Authority boundary

| Component | Owns | Does not own |
| --- | --- | --- |
| Codex worker | One bounded attempt inside an isolated workspace | Host files, human keys, verifier, network tools |
| Frozen verifier | A scoped mechanical result over exact bytes | Scientific acceptance |
| Canopus | Run evidence, replay, Submission export | Verification Records, Decisions, Events, Standing |
| Vela | Registration, review, decisions, replay, Standing | Model execution |

The worker uses macOS Seatbelt or Codex Bubblewrap on Linux/WSL2. The verifier
runs separately with network and writes denied. Canopus never reads a human key
or interprets verifier success as acceptance.

## Exact product contract

Current source is Canopus `0.8.0-rc.5`, composed with released Vela
`0.940.5`, Codex CLI `0.145.0`, Bun `1.3.12`, Git, and a pinned verifier
container. Historical releases remain available for exact replay; they are not
current writers.

Mission v1 and profile v2 remain the advanced portable interfaces:

```sh
canopus mission prepare ...
canopus mission validate bundle/mission.json
canopus profile list
canopus profile show <name>
canopus profile validate <name>
canopus profile pack <name> --output <directory>
```

## Development

```sh
bun install --frozen-lockfile
bun run check
bun run pack:check
```

The installed package has no runtime npm dependencies.

## Documentation

- [Missions and profiles](docs/MISSIONS.md)
- [Run, export, and submit records](docs/RUN_RECORD.md)
- [Evaluation gates](docs/EVALUATION.md)
- [Nonmutating Runs and explicit Submission](docs/adr/0010-nonmutating-runs-and-explicit-submission.md)
- [Why Canopus stays removable](docs/adr/0001-harness-boundary-and-name.md)
- [Historical Build Week evidence](https://github.com/vela-science/vela-research-harness/blob/v0.6.5/BUILD_WEEK.md)

## License

Apache-2.0 OR MIT, at your option. Vela remains the protocol and authority
boundary.
