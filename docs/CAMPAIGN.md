# Current Vela work

Vela core has one job: make exact scientific state easy to create, inspect,
verify, decide, replay, and transfer without becoming a research runner or a
hosted authority service.

## Active product loop

```text
init -> submit -> verify -> decide -> replay
```

The current tranche is successful when a new operator can:

1. create a signed, replayable Frontier with one `vela init` command;
2. submit bounded evidence without first building a Target adapter;
3. retain scoped Verification without confusing it with acceptance;
4. make one explicit attributed Decision;
5. reproduce the repository from a clean clone; and
6. get one truthful next action from every command and failure.

Vela `0.965.3` removes the separate authority-initialization ceremony and the
empty-Frontier dead end. The remaining work is defect-driven: exercise this
loop on real Frontiers, fix reproduced failures, and delete redundant paths.

## Boundaries

- Scientific campaigns, Target packets, proofs, computations, and local
  Decisions belong to their source-owning Frontier repositories.
- Vela Web owns read-only projections and presentation.
- Vela core owns protocol semantics, the CLI, schemas, conformance, and replay.
- Verification remains evidence. Only an authorized Decision changes Standing.

Vela core will not run reviewer-recruitment studies, maintain case-specific
campaign portfolios, host a package registry, schedule research agents, or add
an interoperability service without a concrete second consumer. Git history
preserves retired experiments; they are not active product surface.

## Acceptance

Every core change must pass the focused crate tests, cold-start lifecycle,
clean-clone replay, formatting, and `git diff --check`. A release candidate also
passes the repository release check. Claims are limited to the behavior those
checks establish.
