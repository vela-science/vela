# Vela threat model

Vela protects the integrity, attribution, authorization, and replay of one
bounded scientific Repository. It does not determine whether the science is
true, novel, important, or ethical.

## Protected assets

- canonical Claim, Submission, Verification, Proposal, Decision,
  Event, and Artifact bytes;
- repository-authority keysets, policy material, and history;
- the authenticated principal and semantic action behind each canonical write;
- exact Git commit/tree, repository origin, object sets, and roots;
- the independently distributed sequence-one authority-record root;
- producer-declared execution-binding roots, Submission, and Verification
  bindings; and
- deterministic replay and frozen verifier contracts.

## Boundary

```text
untrusted producer
  -> authenticated Submission + content-addressed Artifacts
  -> pending Proposal

untrusted verifier input
  -> signed scoped Verification Record

authorized semantic principal
  -> Decision
  -> repository-authority record
  -> canonical Event and Standing

verified Git Repository
  -> disposable Observatory and indexes
```

Git hosts distribute bytes and control repository access. Vela authority
records authenticate canonical repository writes. The Observatory, Neon,
search, graphs, exports, and caches are not trust roots.

## Attacker classes

1. **Malicious producer.** Submits fabricated Claims, Artifacts, citations,
   paths, metadata, or prompt injection.
2. **Compromised verifier.** Signs an incorrect, underspecified, dependent, or
   substituted scoped result.
3. **Compromised local process.** Races or changes files writable by the user
   and lies in its presentation.
4. **Compromised repository-authority key.** Signs actions available to its
   authenticated and authorized environment.
5. **Compromised Git host.** Hides, replaces, forks, or rolls back refs and
   hosted objects.
6. **Compromised reader.** Omits or forges projections, ranking, search, or
   freshness.
7. **Compromised build channel.** Distributes a binary that misrenders,
   authorizes, or signs a different transaction.
8. **Resource attacker.** Supplies oversized objects, graphs, or repeated
   expensive validation requests.

## Defended properties

### Canonical object tampering

Full content identities rederive from closed canonical bytes. Relationships
bind full roots. Altering a protected field, substituting an object, or using a
shortened digest fails strict validation.

### Malicious producer input

`submit` treats the Submission and every Artifact as untrusted. It validates
closed schemas, sizes, paths, digests, producer identity, exact Claim targets
for corrections, current receiving-repository context, execution-binding root
shapes, and declared verification requirements before intake.

A producer signature proves origin only. Intake creates a pending Proposal and
no accepted Standing.

### Verifier substitution

A Verification Record binds the exact Claim, Submission id and root, Proposal,
Artifacts, method, environment root, scoped property, outcome, nonclaims, and
verifier identity. It names no Repository; import resolves every reference
against exact repository membership, so a record is usable only in the one
repository that holds all of its subjects.

A passing record cannot be reused for a changed Claim, broader property,
different Artifact, different Proposal, or substituted implementation.
Independence is explicit and checked where required.

### Decision substitution

One `review accept` or `review reject` action binds the exact Proposal, action,
reason, principal, policy, authority head, ordered Verification Records, read
set, binary identity, and canonical delta.

Any drift aborts before the commit marker. Agent producer identities are
refused as human review principals. The repository-authority service key is
loaded once into the standard OpenSSH agent for the authenticated local OS
session. There is no batch approval, wildcard, copied confirmation root,
custom signer, or Vela-managed human key. A trusted native agent may execute a
named Decision or campaign the operator explicitly authorized; the native
runner supplies that workflow authorization while Vela still checks each exact
Decision. Forwarding the unconstrained authority-agent socket to remote,
untrusted, or proposal-supplied code remains a custody failure.

### Repository-authority substitution

Authority records form a full-root DSSE chain. Each record commits to the
previous record, keyset, policy, principal, authorization, semantic action,
Event roots, repository roots, and complete write set.

The checkout cannot choose its own sequence-one trust anchor. Consumers install
the full first authority-record root from an independent channel.

### Fork and rollback

Strict checking verifies the repository origin, predecessor commitment, Git
commit/tree, authority-chain continuity, repository manifest, and current
objects. A valid Git commit or authority signature alone does not prove that a
consumer has the intended fork.

Independent clones, protected refs, predecessor archives, and pinned trust
roots make replacement detectable. Vela cannot recover bytes that no honest
copy retains.

### Stale or malicious work projection

Vela core owns no Target catalogue or `next`/`start` command pair. A
source-owning Repository or read product may expose exact next obligations,
packets, and ranking under its own rooted freshness contract. That owner must
detect stale or substituted source inputs; `vela replay` does not validate the
projection.

No work projection creates authority. A compromised reader can misorient a
producer, but only an authenticated Submission enters review and only an
authorized Decision changes Standing.

### Concurrent writes and recovery

Canonical transactions bind expected repository and Git state, exact path
sets, read sets, and postimages. They do not consume unrelated staged work or
silently merge a changed base.

The private journal and commit marker distinguish uncommitted Prepared state
from a committed transaction whose exact installation may be incomplete. A
failed or cancelled transaction before the marker creates no canonical effect.
One valid unfinished transaction stops ordinary writes with its exact operation
ID; corrupt or multiple state supplies no guessed ID. Read-only commands do not
mutate recovery state and no command auto-recovers a different operation.

`vela recover --repo <PATH> <OPERATION_ID>` acquires the repository-wide lock
and opens only the named journal. It may abort only an exactly Prepared journal
whose marker is definitely absent. A valid marker authorizes policy-free,
idempotent installation of the already-bound postimages; it does not deserialize
permission or reacquire a signer, trust material, authority policy, or
Decision. Malformed or unreadable markers, root/Profile-identity binding or
path substitution, missing or corrupt blobs, postimage conflicts, and
ambiguous incomplete state fail closed rather than being treated as absence.

Recovery ends after that one filesystem transaction. It does not resume the
semantic command, begin another write, or create, move, or publish a Git ref.
Terminal Completed and Aborted journals are safe to name again without
rewriting semantic state.

Native genesis has a separate post-transaction Git/trust tail. A retry accepts
only one exact Completed sequence-one operation whose signed request, result,
read set, canonical delta, repository/account context, scaffold bytes, and
runtime-validated private residue all agree. Git initialization and publication
strip ambient `GIT_*` redirection and disable inherited configuration, hooks,
filters, attributes, alternates, replacement refs, prompts, pagers, signing,
and transports. The parentless commit uses the recorded transaction time and
fixed Vela identity, not the retry clock. Unexpected public paths (including
case or Unicode aliases), mutable ignore-based residue, dirty index/ref state,
or a colliding trust pin fail before publication; every fallible repository and
Git verification runs before pin installation. Recovery itself never enters
this tail and never needs a signer or trust credential.

### Reader compromise

Readers receive only exact rooted repository projections and no writer
credential. They expose no acceptance, signing, policy, Submission,
Verification, or Event mutation.

Resolve disputes from an exact Git checkout:

```bash
vela replay . --json
vela reproduce .
```

Agreement among readers is corroboration, not authority.

### Repository migration substitution

The current origin is a genesis and binds no predecessor. Pre-1.0 continuity
between two wire lineages is retained separately as a signed migration
attestation over exact predecessor and successor commits, trees, Vela roots,
archive digest, object mapping, declared losses, and equivalence limits.

A consumer must obtain the predecessor tag or bundle and recompute those
bindings before relying on the attestation. Missing objects, changed mappings,
partial archives, ambiguous Claims, or altered Standing invalidate continuity;
they do not invalidate independent replay of the current genesis. Current
verification rejects any attempt to reintroduce a predecessor path into the
origin itself.

## Partially mitigated risks

### Key compromise

A stolen repository-authority key can sign within the policy and principal
context available to the attacker. The closed action vocabulary and exact
read-set binding reduce substitution and accidental misuse but cannot make a
stolen key safe.

Rotation, revocation, and incident response must be explicit authority
transitions. Deleting local key material cannot rewrite historical signatures.

### Verifier underspecification

A deterministic verifier can check the wrong property. Vela records exact
scope, inputs, method, outcome, environment, and nonclaims, but expert review
must still assess statement fidelity, significance, and missing assumptions.

New verifiers need positive fixtures, adversarial mutants, malformed cases,
resource bounds, reproducible environments, and independent review of the
property they establish.

### Build compromise

Source hosts, compilers, release workflows, package registries, and installers
remain supply-chain dependencies. Pin exact releases, verify checksums and
attestations, and review authority-path changes. The scientific-state protocol
cannot make a malicious executable safe.

### Workstation compromise

Vela cannot protect a repository when the operating-system account, active
authority agent, binary, and filesystem are all compromised. Operator
hardening, short-lived agent loading, device security, and incident procedures
remain required.

## Explicitly out of scope

- deciding scientific truth, novelty, importance, or ethics;
- confidentiality for bytes committed to a public Repository;
- preserving source bytes Vela never received;
- Git-host availability and organization-account recovery;
- automatic key recovery;
- hosted authority or a public mutation API;
- restricted-reader authentication;
- universal scientific ontology or domain semantics; and
- making one authorized but scientifically bad judgment correct.

Vela makes a judgment exact, attributable, replayable, and correctable. It does
not make it true.

## Security reports

Do not include keys, credentials, private Repository data, or exploit details in
a public issue. Use the repository security-advisory channel.
