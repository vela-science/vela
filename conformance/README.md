# Vela conformance

This directory contains the small, implementation-independent corpus for
Vela's current public object boundary.

Run all checks:

```bash
uv sync --project conformance --locked --all-groups
uv run --project conformance --locked ./conformance/check-core.sh
```

The independent reader supports Python 3.11 through 3.14. CI uses an exact
interpreter while `requires-python` keeps the reader contract independent of a
single minor line. CI-affecting Python tools are part of the same lock:

```bash
uv run --project conformance --locked ruff check conformance
uv run --project conformance --locked zizmor --offline --min-severity medium .
```

The corpus protects five contract families:

1. canonical JSON bytes and SHA-256 roots;
2. byte-identical Submission and Verification emission from an independent
   JavaScript implementation; and
3. checked JSON Schema 2020-12 descriptions and frozen current-object fixture
   roots;
4. exact witness and bounded-Claim agreement; and
5. a non-authoritative correction-impact projection, including independent
   support-route survival and bounded fail-closed diagnostics.

`current-objects/` contains deterministic signed Submission and Verification
vectors. The seed files are public fixture material only. They are never used
as production identities. `manifest.v1.json` freezes the exact fixture bytes;
the schemas document structure and carry no authority or Standing effect.

`fixtures/exact-witness-floor-v1.json` is a normative test vector.

## `frontier_lint.py`

Everything above is about protocol bytes. `frontier_lint.py` is about
repository shape, and it is the one thing here that takes an argument:

```bash
uv run --project conformance --locked python conformance/frontier_lint.py /path/to/a-frontier
uv run --project conformance --locked python conformance/frontier_lint.py /path/to/a-frontier --json
```

It exits 1 on findings, 2 when a declaration it reads has gone missing, and 0
otherwise. It changes nothing and reads nothing outside the two roots it is
given: the Vela checkout it ships in, and the Frontier named on the command
line. `../vela` is never consulted, because in the CI job that matters most
there is no `../vela` to consult.

Five rules, each reading the fact it needs from whatever declares it, so that
none of them can go stale on its own:

| Rule | Reads |
| --- | --- |
| `shared-package-copy` | the real file list, `__all__`, and module symbols of every package under `packages/` |
| `non-production-dependency` | each `vela.package-consumer-reference.v1` in the Frontier, and `subdirectory =` in its `pyproject.toml` |
| `unpinned-action` | every `uses:` in `.github/`, checked for a 40-character SHA and nothing else |
| `retired-path` | the fenced list under `<!-- frontier-lint:retired-paths -->` in `docs/FRONTIER_REPOSITORY_PROFILE.md` |
| `generated-file` | the lock schema and console-script name published by the package that generates the lock |

`unpinned-action` deliberately does not check that the comment beside a SHA
still names the tag it had. Tags move; that check was already red this week for
four repositories whose pins were correct.

`test_frontier_lint.py` fails every rule on purpose and then passes it, and CI
runs it. A rule that can no longer be made to fire is reported coverage that
does not exist, which is worse than an absent rule.

`fixtures/correction/diamond-input.v1.json`,
`diamond-expected.v1.json`, and `diamond-adversarial.v1.json` are synthetic
conformance vectors only. They let the Rust reader in `vela-edge` and the
clean-room Python reader agree on exact bytes before a real correction fixture
exists. They earn no scientific or protocol-breakthrough credit.

Historical reducer cascades, AcceptancePolicy experiments, actor-registration
previews, and their duplicate Python/TypeScript readers remain available in
Git history. They are not current runtime contracts and are intentionally
absent from this corpus.
