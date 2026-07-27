# Publishing a Frontier

A published Frontier is one independently clonable Git repository with an
exact current Vela repository epoch. Publication distributes bytes; it does
not create scientific acceptance.

## Before publishing

```bash
vela status . --json
vela check . --strict --json
vela reproduce .
vela repository verify . --json
git status --short
```

Require:

- clean tracked state;
- current repository epoch;
- valid sequence-one authority trust anchor;
- contiguous repository-authority history;
- canonical object and repository-root parity;
- no active or corrupt recovery journal;
- exact Target Index status;
- frozen verifier results for claims being cited; and
- no private coordination or credential material in tracked files.

## Publish exact Git state

Use a normal Git commit and protected public ref. Record:

- repository URL;
- full commit and tree;
- Vela version and binary SHA-256;
- Frontier ID;
- epoch ID and root;
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
git clone <frontier-url>
cd <frontier>
git checkout <full-commit>
vela authority trust pin . --record-root sha256:... --json
vela check . --strict --json
vela reproduce .
```

The trust pin reads no key, grants no authority, and changes no Frontier byte.

## Reader publication

The Observatory and other projections must bind:

- exact Frontier source URL, commit, and tree;
- repository epoch and root;
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

## Predecessor epochs

The current epoch binds the predecessor tag, commit, tree, object manifest,
archive digest, roots, imported Claim set, and equivalence report. Keep those
objects reachable. Do not present the predecessor as a second live Frontier.
