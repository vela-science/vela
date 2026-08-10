# Publishing a Repository

A published Repository is one independently clonable Git repository with an
exact current Vela repository origin. Publication distributes bytes; it does
not create scientific acceptance.

## Before publishing

```bash
vela status . --json
vela replay . --json
vela reproduce .
git status --short
```

Require:

- clean tracked state;
- current repository origin;
- valid sequence-one authority trust anchor;
- contiguous repository-authority history;
- canonical object and repository-root parity;
- no active or corrupt recovery journal;
- frozen verifier results for claims being cited; and
- no private coordination or credential material in tracked files.

## Publish exact Git state

Use a normal Git commit and protected public ref. Record:

- repository URL;
- full commit and tree;
- Vela version and binary SHA-256;
- Repository UUID;
- origin ID and root;
- repository root;
- authority head and Event-log root;
- accepted and pending Claim-set roots;
- Proposal, Verification, and Artifact roots; and
- reproduction commands.

Tags and release archives should be immutable. Checksums and build attestations
identify distributed artifacts; they do not replace Vela replay.

## Consumer verification

A consumer:

1. obtains the intended repository URL and commit through the publication
   channel;
2. obtains the sequence-one authority-record root independently;
3. installs the local public trust anchor;
4. runs strict repository verification; and
5. runs the declared frozen reproduction.

```bash
git clone <repository-url>
cd <repository>
git checkout <full-commit>
vela authority trust pin . --record-root sha256:... --json
vela replay . --json
vela reproduce .
```

The trust pin reads no key, grants no authority, and changes no Repository byte.

## Reader publication

The Observatory and other projections must bind:

- exact Repository source URL, commit, and tree;
- repository origin and root;
- current authority head;
- object-set counts and roots;
- projection schema and root; and
- generation version and time.

Readers label historical projections as historical. They never display
Verification as acceptance or graph rank as Standing.

## Updating

Every new canonical write creates a new Git commit and repository root.
Publishers do not amend, force-push, or regenerate old canonical bytes.

A correction is a new Submission, Decision, and Event. It preserves the prior
Claim and exposes the new Standing through replay.

## Lineage migrations

The current origin is always a genesis and carries no predecessor block. A
pre-1.0 wire migration instead retains a separately signed attestation naming
the predecessor and successor Git commits, trees, Vela roots, archive digest,
re-admission mapping, and declared losses. Keep the predecessor tag and bundle
reachable. The attestation is continuity evidence beside the current
Repository; it does not make predecessor bytes active state or assert that two
different objects have the same root.
