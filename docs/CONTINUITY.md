# Continuity

`docs/THREAT_MODEL.md` puts Git-host availability and organization-account
recovery explicitly out of scope. That is the right boundary for the protocol
and the wrong place to stop for an operator: nothing in Vela depends on GitHub
being reachable, and until now nothing said so, named the replacement, or said
who decides.

This document states those rules. It adds no protocol object, no schema, no
command, and no new trust root. Everything it relies on already exists and is
specified elsewhere; where that is true it cites rather than restates.

## 1. What defines the authority repository

Four bindings, jointly. Any one of them alone identifies nothing.

| Binding | Where it is retained | Example (`vela-science/math`, 2026-08-18) |
| --- | --- | --- |
| `repository_id` | `vela.toml`, `vela.repository-profile.v1` | `3d012325-3768-4b95-a385-c94e9f2a57a6` |
| Origin ID and origin root | `.vela/origin.json`, `.vela/repository.json` | `vro_a6a12da8762f1252`; `sha256:a6a12da8762f1252afe1ac1c75361ef54c583924a2b88474232ead6227873dca` |
| Current repository root | derived by replay over the canonical object set | `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4` |
| Sequence-one authority-record root | distributed independently of the checkout | `sha256:efae3e02b5be6dfccf6701ebe26f87f00bb64f5b4372674e572a633844d95469` |

The first three travel inside the checkout. The fourth deliberately does not:
`docs/SIGNING.md`, "Consumer trust", requires the first authority-record root to
arrive through a channel the checkout does not control, and
`docs/THREAT_MODEL.md`, "Repository-authority substitution", states the reason —
a checkout cannot be allowed to choose its own trust anchor.

The consequence for continuity is the whole point. A repository whose four
bindings reproduce is the authority repository wherever its bytes are found. A
repository that reproduces three of them and disagrees on the fourth is a fork,
and no amount of hosting makes it otherwise.

## 2. Host URLs are locators

A URL says where to fetch bytes. It never says whose bytes they are.

`docs/PUBLISHING.md`, "Publish exact Git state", already lists what a
publication records, and the repository URL is one line of that list beside the
full commit, tree, origin, repository root, authority head, and Claim-set roots.
The verification recipe in the same document, "Consumer verification", clones
from a URL and then verifies against roots obtained separately. The URL is the
first step and carries none of the proof.

So:

- Changing where Vela is hosted changes no identity, no root, and no Standing.
- Two remotes that serve the same commit and tree serve the same repository.
- A remote that serves a different commit under the same URL is a fork with a
  familiar address, and `vela replay` is what says so.
- Any document, manifest, or interface that treats a host URL as the name of a
  repository is wrong and should name the four bindings from §1 instead.

## 3. One active writer; every other remote is a read replica

At any instant exactly one remote is the **active writer**: the remote that
canonical commits are pushed to and that the operator's tooling treats as the
publication ref.

Every other configured remote is a **read replica**. A read replica may be
fetched from, cloned from, verified against, and restored from. It is not
pushed to by ordinary operation and it does not become the writer by being more
current, more available, or more convenient.

This is an operational rule, not a protocol one. The protocol already refuses
the failure it prevents — `docs/THREAT_MODEL.md`, "Concurrent writes and
recovery", binds every canonical transaction to expected repository and Git
state — but a protocol that refuses a divergent write still leaves an operator
with two half-published histories to reconcile. One writer is how that
reconciliation never has to happen.

Record the active writer and replicas in the repository's own operator docs
and signed release manifest. Core does not maintain an aggregate hosted-service
status database.

## 4. Mirrors and backups confer no Standing

A mirror is a copy of bytes. A backup is a copy of bytes with a retention
policy. Neither is a Decision.

`docs/REPOSITORY_BOUNDARIES.md` states the underlying rule in one sentence:
moving or rendering a record never changes Standing, and only an attributed,
authorized Decision admitted by the named repository can. Copying is the most
ordinary kind of moving, so it is covered, and this section exists only to make
the covered case explicit.

Concretely, none of the following changes Standing, and none of them is a
publication under `docs/PUBLISHING.md`:

- a bare mirror on an independent provider;
- a periodic `git bundle` carrying all refs;
- an object-storage snapshot of the working tree;
- a hosted read projection such as problems.science; or
- an archived tag, release asset, or source archive.

A mirror that carries a Decision carries it because the Decision was already
made in the authority repository and signed there. Restoring from that mirror
restores the same Decision, with the same signature, to the same Standing. It
creates nothing.

The same rule read backwards is the one already stated in `docs/PUBLISHING.md`,
"Predecessor origins": keep the objects reachable, and do not present a second
copy as a second live repository.

### A repository whose bytes outlive its reader

Preserving bytes is not the same as preserving a reader for them, and for four
repositories the difference is live. `erdos-frontier`, `sidon-frontier`,
`quantum-codes-frontier` and `formal-conjectures-frontier` are archived epoch-1
repositories. Their signed history is intact and mirrored, and the current
`vela` binary cannot read any of it: they carry `frontier.toml` and a
`vela.frontier-profile.v1` profile, so every read path refuses them as
`repository_predecessor_layout` and directs the reader to `v0.966.4`, the last
release that reads them. [Publishing](PUBLISHING.md) defines predecessor
retention without adding a current compatibility path.

Continuity for those four is therefore continuity of bytes plus continuity of a
pinned historical binary, and the second half is a real obligation: an install
path that can no longer retrieve `v0.966.4` ends the ability to read them, and
no mirror of the repositories would show it. ADR 0039 accepted that cost
deliberately, and re-admitting state into `vela-science/math` through ordinary
Submission → Verification → Decision is the sanctioned way to bring any of it
forward. Nothing here claims the cost was wrong; it is written down because a
reader looking for what survives a provider should not have to discover it from
a parse error.

## 5. Promotion is a human decision with a recorded runbook

Promoting a read replica to active writer is the one continuity action that
changes something. It is a human decision. It is never automatic, never
triggered by a health check, and never inferred from a host being unreachable.

It is also not a scientific Decision, and it is not signed by the
repository-authority key. `docs/SIGNING.md` fixes that boundary: the repository
authority key attests that a principal, an authorization, a semantic action, a
read-set recheck, and a canonical write matched. Choosing where to push is none
of those. Promotion changes the locator, and §2 says a locator is not identity.

### Runbook

Perform these in order. Stop at the first step that fails and do not continue.

1. **Confirm the outage is a locator problem.** Fetch the intended commit from
   any replica. If replay succeeds against the pinned sequence-one root, the
   repository is intact and only its usual address is gone. If replay fails,
   this is an integrity incident, not a promotion, and the runbook ends here.

2. **Pick one replica and record why.** Write down the remote, the commit, the
   tree, and who chose it. One writer means one choice; two people promoting
   two replicas is the divergence this rule exists to prevent.

   ```bash
   git ls-remote <replica> refs/heads/main
   ```

3. **Verify the candidate from a clean clone, not from the working checkout.**

   ```bash
   git clone <replica> /tmp/promote && cd /tmp/promote
   git checkout <full-commit>
   vela authority trust pin . --record-root sha256:<sequence-one-root> --json
   vela replay . --json
   ```

   Run any declared scientific method through the source Repository's pinned
   native tooling.

   The pin must be the root already installed by consumers, obtained
   independently. A replica that requires a new anchor is a fork.

4. **Compare against the last published state.** The commit and tree must match
   what the previous active writer last published, or be a fast-forward
   descendant of it. Anything else is a rewind or a fork; both stop the runbook.

5. **Demote the old writer in configuration, not by deleting it.** Its bytes
   remain evidence. Keep it fetchable for as long as it is reachable.

6. **Announce the new writer with the four bindings from §1**, not with a URL
   alone. Consumers who pinned correctly need no action; the announcement exists
   so that the ones who pinned a URL learn that they pinned the wrong thing.

7. **Record the promotion where §3 says the writer is recorded**, with the date,
   the operator, the replica chosen, and the commit verified.

Nothing in this runbook reads a private key, and step 3 is the same read-only
verification a consumer runs. Promotion is a configuration change performed
under verification, which is why it can be written down in advance.

## 6. Every hosted projection is reconstructible from exact retained state

A projection is disposable by construction. `docs/THREAT_MODEL.md` puts the
Problems surface, Neon, search, graphs, exports, and caches outside the trust
boundary, and `docs/ROOTS.md` requires a derived projection to name its exact
source roots and never substitute its own digest for a canonical root.

Continuity turns that from a property into an obligation: a hosted projection
must be rebuildable from retained canonical state alone, with the projection
host, its database, and its build history all gone.

The test is not "the projection can be regenerated." It is:

- every input the projection reads is either a canonical object in an authority
  Repository or a pinned external source acquired through a source-owned exact
  contract whose adapter, native identity, retained occurrence evidence, and
  input roots are bound by the projection release;
- the release the projection was built from names its source roots, so a rebuild
  can be compared against the published one rather than merely produced; and
- the rebuild path runs without any credential that only the projection host
  holds.

A projection input that satisfies none of these is a fact the repository does
not contain. [Repository boundaries](REPOSITORY_BOUNDARIES.md) forbid a read
product from turning such a fact into authority. Continuity tests that rule
rather than asserting it.

## 7. Acceptance test

The rules above are worth exactly as much as this test passing. Run it on a
clean machine with GitHub unreachable.

1. Retrieve `vela` and the mathematics authority repository from an independent
   provider — a bare mirror, a `git bundle`, or object storage. No GitHub
   fetch, no `gh`, no release API.
2. Verify the pinned roots: install the independently obtained sequence-one
   authority-record root with `vela authority trust pin`, then run
   `vela replay . --json` against both repositories. Run declared scientific
   methods with each source Repository's pinned native tooling.
3. Replay the same Standing. The accepted and pending Claim sets, Event-log
   root, and repository root must equal the ones the last publication recorded.
4. Make one authorized local Decision with `vela review accept` or
   `vela review reject`, signed by the repository-authority key held in the
   local OpenSSH agent. This is the step that proves the authority survived the
   host: no hosted service participates in it.
5. Rebuild the read projection from that state and compare its roots against the
   last published release.

### Where it stands today

Provider-loss qualification is recorded by the repository or product that owns
the deployment and exact replica it tested. It is operational evidence, not a
Core protocol object or a generated ecosystem-wide status file.

The independent retrieval was exercised on 2026-08-09 under the topology in
force that day. After the UUIDv4 re-genesis and human Decisions, `math` was
cloned anonymously from Codeberg with no GitHub involvement and replayed to
`sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc`,
the same Repository, origin, authorization-model, and authority-keyset roots as
the active writer. That retained qualification is historical evidence; it does
not present the former locator as a current replica.

`install.sh` no longer fails step 1. It prefers the signed release manifest —
`ssh-keygen -Y verify` against the `vela-release` namespace, then a digest
comparison from the manifest to the archive — which needs OpenSSH and a checksum
tool and nothing else. `VELA_RELEASE_BASE_URL` points it at a mirror or a local
directory, and `VELA_ALLOWED_SIGNERS` supplies the trust root out of band, so
neither the bytes nor the verifier need come from GitHub.

`v0.968.1` first closed the two release-distribution gaps. It is the first
release published with signed
manifests: `release.yml` publishes a draft, `scripts/sign-published-release.sh`
signs each manifest against the bytes CI built, checks every digest, uploads the
sidecars and then publishes — so the release is immutable and signed from the
moment it is visible. Exercised both ways: `VELA_VERSION=v0.968.1
VELA_REQUIRE_SIGNED_MANIFEST=1` installs from GitHub and from
`codeberg.org/vela-science/vela/releases/download/v0.968.1`, in both cases
reporting `Verified by: signed release manifest (provider-independent)` with the
provider-coupled fallback refused.

`v0.968.0` is immutable and unsigned and cannot be repaired; it stands as the
last unsigned release.

The current `v0.977.4` release follows the same signed-before-publication path.
Its two published manifests agree with their archives and SBOMs, both
signatures verify under the out-of-band distribution identity, and a clean
consumer installation with `VELA_REQUIRE_SIGNED_MANIFEST=1` reports
provider-independent verification. Replica publication remains owned by the
Problems release transaction and advances when its exact Vela release pin does.

The mirror mechanism carries signatures too. It reads each retained asset back
over the anonymous public URL and compares it to the SHA-256 committed in
vela-web's `vela-release.v1.json`, rather than to the copy just uploaded. The
last fully exercised replica path is `v0.975.1`: its retained assets and
signatures pass anonymous Codeberg readback with no GitHub dependency in the
retrieval or verification path. `v0.977.4` is not claimed replicated until the
Problems release advances that pin and completes the same readback.

Mirroring stays scoped to the release the Problems projection pins rather than the
historical archive. Section 11.1 asks for what it takes to install and reproduce
the current system, which `vela-release.v1.json` names exactly.

Steps 4 and 5 passed in the 2026-08-09 exercise. The human Decisions were
signed through the local OpenSSH-agent authority path; a Codeberg-only clone of
their published state then rebuilt twice into fresh local PostgreSQL clusters
from one retained source-adapter artifact, verified identical manifest and
table roots, enforced a SELECT-only reader, and exported the root-bound
Erdős 321 read projection. Exact inputs, roots, limitations, and the historical
mirror run are retained in
`docs/history/PROVIDER_LOSS_QUALIFICATION_2026-08-09.md`. That result does not close the
present step-1 gap after the Math replica was removed from the declared
topology.

## 8. Exact state replay is not a computational rerun

`vela replay` reconstructs and validates scientific **state**. It does not
execute the computation, model, proof checker, instrument, assay, or review
method that produced evidence. The distinction is observable and intentional:
a complete checkout can replay the same Standing while a source-owned method
is unavailable, and a retained method can be available for a new attempt
without that attempt recreating the historical Decision.

The current implementation has no second materialized Standing database or
Standing file. The accepted and pending Claim indexes live in the canonical
repository manifest and are covered by its root. Strict replay validates those
indexes against the admitted Events, authority history, exact object set, and
Git ancestry. In campaign notation, the implemented invariant is:

```text
root(strict replay of authoritative history and exact objects)
  == current canonical repository root, including its Standing indexes
```

The same authoritative history and exact object bytes therefore produce the
same root and Standing projection on the original checkout and a complete
clean clone. Missing or changed canonical Claim, Submission, Verification,
Proposal, Withdrawal, Artifact, Event, authority, origin, or manifest bytes
fail closed. A changed trust anchor, authority model, keyset, authorization
request, Event linkage, or Git ancestry also fails closed.

The operation-typed evidence boundary is deliberately smaller than a generic
receipt system:

| Retained binding | What can be reconstructed exactly | What remains a separate native rerun |
| --- | --- | --- |
| Submission DSSE envelope and derived Claim/Proposal | Authenticated producer bytes, requested change, exact Artifact roots, caveats, provenance, and pending relation | The producer's external session, workflow, or opaque `source_run` |
| Canonical Artifact under `records/artifacts/sha256/` | The exact retained bytes, path, full SHA-256 identity, and every Claim or Verification reference to them | Interpretation or execution of those bytes by a native tool |
| Verification Record DSSE envelope | Exact subject roots, Artifact inputs and outputs, verifier identity, scope, outcome, nonclaims, method profile/path/root, and times | Re-executing the declared check in its native environment |
| Source-owned Review Method file | When present, the read projection verifies its bytes against `method.environment_root`; missing bytes are reported `unavailable`, and substituted bytes fail root resolution | Acquiring tools, models, dependencies, credentials, data, instruments, or physical conditions and attempting the method again |
| Authority Record and semantic Events | The authenticated principal, authorization request and result, exact deltas, Decision attribution, transition order, and derived Standing | Repeating scientific judgment or creating another Decision |

The Submission's `replayability` value is an authenticated producer disclosure
about expected native re-execution. Even `exact` does not expand `vela replay`
into a workflow runner or turn a producer claim into Verification. A
computational rerun may be deterministic, stochastic, approximate, unavailable,
or impossible; a physical rerun is necessarily a new attempt. Either may
produce new evidence, but neither changes Standing without a new Verification
Record where applicable and an attributed authorized Decision.

## What this document does not do

It does not make GitHub availability a protected asset, and it does not add
hosted authority. `docs/THREAT_MODEL.md` keeps both out of scope and this
document does not move them. What it changes is that losing a host is now an
operator procedure with a stated outcome, rather than an undefined condition.

It also does not promise recovery of bytes no honest copy retains.
`docs/THREAT_MODEL.md`, "Fork and rollback", is explicit that Vela cannot
recover what nothing retained. Continuity is retention plus verification.
Without the retention, verification has nothing to verify.
