# Vela architecture

Vela is version control for scientific state.

The product story is:

```text
inspect -> attempt -> submit -> verify -> decide -> continue
```

A Frontier is one ordinary Git repository for a bounded scientific scope.
Vela defines the portable records, replay rules, repository authority, and
Decision boundary. Producers and readers remain replaceable.

## Scientific-state flow

```text
workbench
   |
   +-- Attempt -- Submission ------------------+
   |                                           |
verifier -- Verification Record ---------------+
                                               v
                                      pending Proposal
                                               |
                                     authorized Decision
                                               |
                                               v
                                         Event + Standing
                                               |
                                               v
                                    read-only projection
```

Each object has one job:

| Object | Meaning | Authority effect |
| --- | --- | --- |
| Attempt | Local bounded work against exact starting roots | None |
| Submission | Authenticated producer input | Requests change |
| Registration Record | Proof that exact input entered the repository | None |
| Verification Record | Scoped verifier observation over exact inputs | None |
| Claim Record | Versioned assertion and evidence identity | Standing is derived |
| Proposal | Candidate state transition | None until decided |
| Decision | Authorized accept or reject action | Determines transition |
| Event | Canonical admitted transition | Changes replay |
| Standing | Root-bound replay result | Read projection |

Git publication is not acceptance. Verification is not acceptance. A
signature authenticates exact bytes; it does not establish that a Claim is
true.

## Repository ownership

The active public topology is intentionally small:

| Repository | Sole responsibility |
| --- | --- |
| `vela-science/vela` | Protocol, CLI, replay, repository authority, schemas, conformance, releases, architecture, roadmap |
| `vela-science/canopus` | Optional bounded producer, run evidence, verifier execution, Submission export, evaluation adapters |
| `vela-science/vela-web` | Editorial site and read-only Observatory |
| `vela-science/erdos-frontier` | Canonical Erdős Frontier |
| `vela-science/formal-conjectures-frontier` | Canonical formal-conjectures Frontier |
| `vela-science/sidon-frontier` | Canonical Sidon Frontier |
| `vela-science/quantum-codes-frontier` | Canonical quantum-codes Frontier |
| `vela-science/.github` | Organization profile, reusable workflows, security policy, repository templates |

`vela-science/vela-research-harness` is the current repository name for
Canopus. It will become `vela-science/canopus` after the `0.8.0` release gate;
GitHub's repository redirect preserves existing clones and release links.

`vela-science/vela-internal` is a transition repository, not a product
component. It is being decomposed and will be archived after every
load-bearing check and current document has moved to its owner. No supported
Vela, Canopus, Frontier, or Observatory workflow may depend on it.

## Release boundaries

Each component releases independently and publishes its own version, commit,
artifacts, checksums, provenance, and supported interface versions.

- Vela releases the protocol implementation and CLI.
- Canopus tests against released Vela binaries through the public CLI.
- Each Frontier verifies and reproduces its own exact state.
- Vela Web verifies its read projection against exact Frontier sources.
- Organization workflows test the compatibility matrix without becoming a
  canonical writer or synthetic ecosystem release.

An exact scientific Run still pins every binary and digest it used. That
execution identity does not require all components to share a release number.

## Non-goals

The architecture does not add:

- a hosted authority or second writer;
- a canonical database or public mutation API;
- a universal ontology or work graph;
- a mandatory model runner;
- a private meta-repository that outsiders must understand;
- or an integration repository without an installable distribution.

Private memos and exploratory work remain noncanonical and non-release-gating.
They do not become product infrastructure merely because they are useful.
