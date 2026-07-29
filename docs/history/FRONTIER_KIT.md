# Frontier Kit

> **Historical kit.** This package predates the current repository epoch and
> is not a current onboarding or command contract.

A Frontier Kit gives an outside producer enough exact context to create one
useful Submission without importing Vela internals or authority.

## Required contents

1. Frontier repository URL, commit, tree, and full roots.
2. Bounded Target and exact packet root.
3. Completion contract and accepted result classes.
4. Artifact paths and size limits.
5. Frozen verifier name, version, capsule root, and replay command.
6. Scope caveats and known semantic gaps.
7. Producer identity and custody rules.
8. Authority ceiling: producer evidence only.

The kit must not contain a human key, repository-authority credential, policy
writer, event constructor, or instruction to edit canonical JSON.

## Producer path

```bash
vela status . --json
vela next . --limit 1 --json
vela start <target> --frontier . --as agent:<name> --json

# Run the packet's exact work and verifier.

vela submit --frontier . \
  --attempt <vat_id> \
  --claim "<bounded result>" \
  --type <claim-type> \
  --replayability <class> \
  --artifact <path>:<kind> \
  --caveat "<scope limit>" \
  --requires-verification "<independent check>" \
  --as agent:<name> \
  --json
```

The successful engineering outcome is a valid Submission, Registration Record,
and pending Proposal with an accepted-event delta of zero. Scientific
acceptance is a later repository-authority Decision.

## Review packet

A reviewer receives:

- the exact Submission and Registration Record roots;
- the Claim text and conditions;
- Artifact roots and replay instructions;
- independent Verification Records;
- the pending Proposal and proposed state diff;
- known limits, conflicts, and corrections; and
- the current Frontier and authority roots.

The packet is a view, not authority. Use:

```bash
vela review show . <vpr_id> --json
vela review diff . <vpr_id> --json
```

## Reproducibility

The kit passes only when a clean clone reproduces the exact objects and
strict state without network-only context:

```bash
vela check . --strict --json
vela reproduce .
vela show . <object_id> --json
```

Record commands, versions, roots, wall time, interventions, and any failure.
Never upgrade a bounded negative result into a universal claim.
