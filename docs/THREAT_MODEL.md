# Vela threat model

Vela protects the integrity, attribution, authorization, and replay of one
bounded scientific Frontier. It does not determine whether the science is
true, novel, important, or ethical.

## Protected assets

- canonical Claim, Submission, Registration, Verification, Proposal, Decision,
  Event, and Artifact bytes;
- repository-authority keysets, policy material, and history;
- the authenticated principal and semantic action behind each canonical write;
- exact Git commit/tree, repository origin, object sets, and roots;
- the independently distributed sequence-one authority-record root;
- Target, packet, Submission, and Verification bindings; and
- deterministic replay and frozen verifier contracts.

## Boundary

```text
untrusted producer
  -> authenticated Submission + content-addressed Artifacts
  -> Registration Record + pending Proposal

untrusted verifier input
  -> signed scoped Verification Record

authorized semantic principal
  -> Decision
  -> repository-authority record
  -> canonical Event and Standing

verified Git Frontier
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
closed schemas, sizes, paths, digests, producer identity, Target references,
current repository context, and declared verification requirements before
intake.

A producer signature proves origin only. Intake creates a pending Proposal and
no accepted Standing.

### Verifier substitution

A Verification Record binds the exact Frontier, Claim, Submission, Proposal,
Artifacts, method, environment, scoped property, outcome, nonclaims, and
verifier identity.

A passing record cannot be reused for a changed Claim, broader property,
different Artifact, different Proposal, or substituted implementation.
Independence is explicit and checked where required.

### Decision substitution

One `review accept` or `review reject` action binds the exact Proposal, action,
reason, principal, policy, authority head, ordered Verification Records, read
set, binary identity, and canonical delta.

Any drift aborts before the commit marker. Agent producer identities are
refused for human review. There is no batch approval, wildcard, copied
confirmation root, custom signer, or Vela-managed human key.

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

The Target Index is derived but binds the current repository origin/root,
source inputs, packets, task contracts, and deterministic rank facts. `next`
validates the full index; `start` revalidates the chosen Target and prints a
write-free briefing.

A stale or invalid index yields no Offer. Ranking and graph position never
create authority.

### Concurrent writes and recovery

Canonical transactions bind expected repository and Git state, exact path
sets, read sets, and postimages. They do not consume unrelated staged work or
silently merge a changed base.

The recoverable journal and commit marker distinguish preflight, active,
committed, and recoverable states. A failed or cancelled transaction before the
marker creates no canonical effect.

### Reader compromise

Readers receive only exact rooted repository projections and no writer
credential. They expose no acceptance, signing, policy, Submission,
Verification, or Event mutation.

Resolve disputes from an exact Git checkout:

```bash
vela check . --strict --json
vela reproduce .
```

Agreement among readers is corroboration, not authority.

### Repository predecessor substitution

The current origin binds the exact predecessor remote, tag, commit, tree,
repository and authority roots, archived Event and actor roots, Git-object
manifest, archive digest, and equivalence report.

The transition is signed by repository authority with a null scientific
before/after effect. Missing objects, changed mappings, partial archives,
ambiguous Claims, altered Standing, or a mismatched plan fail before commit.

The one-time transition tool has been removed. Current verification retains the
signed origin boundary and rejects any reintroduced predecessor path.

## Partially mitigated risks

### Key compromise

A stolen repository-authority key can sign within the policy and principal
context available to the attacker. Restricted Cedar actions and exact read-set
binding reduce substitution and accidental misuse but cannot make a stolen key
safe.

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
- confidentiality for bytes committed to a public Frontier;
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

Do not include keys, credentials, private Frontier data, or exploit details in
a public issue. Use the repository security-advisory channel.
