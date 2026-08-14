# Verification

Vela separates byte integrity, mechanical verification, scientific authority,
and presentation. Conflating them is the central failure this gate prevents.

## Four different facts

1. **Artifact integrity:** a digest identifies the exact bytes.
2. **Verifier outcome:** a named, pinned method returned an outcome for those
   bytes and inputs.
3. **Claim binding:** the checked property matches the scoped claim recorded in
   Submission v2 envelope.
4. **Admission:** an attributed, authorized Decision admitted the exact state
   transition. Its performer is explicitly human or agent. Historical
   signed-policy outcomes remain replay facts, not a current writer.

A signature alone does not prove correctness. A passing verifier alone does not
prove novelty, significance, statement faithfulness, or acceptance. Admission
does not retroactively broaden what a verifier checked.

Human, AI-model, organization, and deterministic-tool reviews use the same
Verification boundary. When the retained method is a canonical
`vela.review-method.v1`, it names the performer, provider, stable identifier,
known version, attesting actor, procedure, output contract, and nonclaims. The
Verification Record still carries the outcome and signature. See
[`REVIEW_PROVENANCE.md`](REVIEW_PROVENANCE.md).

Those four reviewer kinds are peers. Evidentiary weight comes from the fitness
of the named method, exact inputs, independence and shared dependencies,
retained outputs, scope, outcome, and limitations. Reviewer kind establishes no
quality rank. Only an authorized Repository Decision may change Standing;
human and agent performers use the same exact-root, policy, and replay checks.

## Replay and source-owned methods

`vela replay [<repo>] --json` validates the current repository origin, taking
the repository as a positional or `--repo` and discovering it upward when
omitted:
content addresses, authority-history continuity, admitted-event replay,
required Artifacts, exact Git ancestry, retained canonical objects, derived
parity, and the independently installed sequence-one authority trust root. It
does not execute or replace a domain method.

Run it from the exact Git tree being cited:

```bash
vela replay . --json
```

`vela replay . --json` reports the typed repository-context
defects and keeps that context invalid. It is diagnostic, not a bypass:
invalid authority or repository context grants no identity, signature, or
historical exemption, and canonical writers fail before journaling.

Scientific methods remain source-owned. A Repository may retain an exact
method manifest, implementation, inputs, outputs, and native command, but Vela
does not scan for or execute them. A verifier runs the pinned method in its
native environment and records the scoped result through `verification
record|import`. A digest, historical pass record, or source-local script is
never silently treated as executable verification.

## Verifier evidence

Submission v2 may record producer checks as attributed evidence:

- method and implementation identity;
- exact inputs and artifact digests;
- pinned toolchain or environment;
- outcome and retained log;
- the property checked;
- caveats and checks not performed.

The producer must report only checks that actually occurred. A failed or
inconclusive check is useful negative state and must not be rewritten as a
pass. A producer check remains `producer_reported`; it is not an independent
Verification Record merely because it is deterministic.

Durable independent evidence is retained with:

```bash
vela verification record . <vpr_id> \
  --profile exact-replay-v1 \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "Scientific acceptance." \
  --output reviews/exact-review-report.json \
  --independent-of agent:<producer> \
  --as verifier:<name> \
  --json
```

The method manifest is one bounded, regular, repository-relative file retained
unchanged in the current Git commit. Its exact bytes become the record's
environment root. Vela resolves the current Claim, Submission, Proposal, and
Submission Artifacts before it loads or creates the verifier's local agent key,
then signs and imports the record through the ordinary atomic intake path.
When the Submission declares exactly one verification requirement, that exact
requirement becomes the Verification property automatically. A Submission with
multiple requirements must select one exact requirement with `--property`.
An additional observation must use `--property ... --complementary`; it remains
useful evidence but does not satisfy a registered requirement.
Missing, decided, or stale Proposals and missing, dirty, untracked, empty,
symlinked, oversized, or escaping method paths fail closed.

`vela verification import . verification-record.json --as verifier:<name>
--json` remains available for an already signed interoperable record.

An import wrapper may pin the Vela executable used for repository intake, but
it must pin immutable copied bytes or a released artifact. It must not pin a
mutable build path such as `target/debug/vela`: rebuilding the same source can
replace those bytes and turn an otherwise unchanged signed Verification into
an avoidable operational failure. The intake-binary pin is separate from the
Verification Record's method, implementation, environment, and report roots;
changing one must never silently rewrite the other.

The current Verification Record requires the exact Claim, Submission,
Proposal, and Artifact bindings it bears on; a named method and implementation;
full execution-evidence roots; an outcome; scope; and explicit nonclaims. It
rejects stale or substituted inputs and cannot accept, reject, or finalize the
Proposal. `vela review show` reports reproduction, verification, and
attributed Decision state as separate facts.

## Submission and admission

The ordinary path is:

```text
native run and scoped evidence
    -> vela submit
    -> pending Proposal
    -> independent Verification Record(s)
    -> review accept | review reject
    -> authorized Decision + canonical Event
```

The signed Submission and its exact Proposal package reference prove retained
intake, not correctness or acceptance. A producer cannot mint a Verification
Record, Decision, Event, or accepted Claim Record. The current writer creates
a pending Proposal and leaves the consequential
action to an authorized Decision.

For a current Submission, the Decision gate re-reads the exact retained
Submission and Verification Record bytes. Each declared
`verification_requirements` entry must equal the `scope.property` of an
independent passing record bound to the same Claim, Submission, and Proposal.
Producer-dependent passes do not count. A fail blocks the route; missing,
invalid, inconclusive, or error records cannot make acceptance available.
Unperformed or unavailable verification yields no current Verification Record
and therefore also cannot satisfy the gate. These checks constrain an
authorized Decision; they do not perform one.

## Attributed Decision

Direct `vela review accept|reject` commands are the Decision surface. A command
binds one Proposal, action, reason, attributed performer, performer class,
optional source-owned session reference, authority principal, policy, authority
head, read set, binary identity, and exact canonical delta. The local
operating-system session authenticates the Repository authority principal, the
closed authorization profile authorizes the action, and the standard OpenSSH
agent Repository-authority key signs the covering DSSE record. Vela reads no
personal scientific key and accepts no copied root, timestamp, batch answer, or
custom-helper response.

Use `--as human:<id>` or `--as agent:<id>` to state who performed the Decision.
Agents SHOULD also retain a source-owned checkpoint or session with
`--session-ref <ref>`. `VELA_ACTOR_ID` and `VELA_SESSION_REF` provide the same
values for native integrations. Reviewer kind neither weakens nor strengthens
the Decision; the exact method, evidence, independence, limitations, reason,
policy, and roots remain inspectable separately.

Any drift aborts before the commit marker. Acceptance additionally requires an
eligible Proposal and strict aggregate Engine gate, then verifies that
the covered scientific domain event and explicit review event replay together
across both histories. Rejection changes no accepted scientific state.

Acceptance is a statement of scoped judgment, not a claim that every
possible property was verified. The decision record should retain the relevant
scope and caveats, especially for formal results where kernel checking and
informal statement faithfulness are separate questions.

## Source-owned method requirements

A method belongs in a source Repository only when it has:

1. a closed input and output contract;
2. deterministic behavior under a pinned environment;
3. explicit resource bounds and failure modes;
4. positive fixtures with independently known expected results;
5. meaningful mutants and malformed negative fixtures it rejects;
6. exact claim-binding rules;
7. source and license suitable for independent reproduction;
8. focused conformance tests shared by implementations;
9. documentation of what a pass does and does not establish.

The bar is adversarial. A checker that accepts the positive fixture but cannot
reject realistic mutants is a rubber stamp. A verifier that fetches mutable
state, calls a model, or requires undisclosed credentials must report those
dependencies and limitations explicitly. None of these methods is a Vela
authority path.

## External Lean

External Lean reproduction is an optional producer adapter. It pins the source
commit, Lean and Mathlib environment, declaration, axiom policy, and retained
logs, then emits an authenticated Submission v2 envelope. A kernel pass
establishes that the named declaration checks under that environment. It does
not establish that the declaration faithfully formalizes the intended theorem
or that the result is novel and important.

External Lean runs only by an explicit manual request. Ordinary changes never
select it automatically, including changes to the adapter itself. It is not a
generic Vela release gate, and no early external project is a compatibility
authority. Formal Lean checks inside Vela remain separately changed-path
selected.

## No transparency-log primitive

This release ships no Merkle construction. An earlier draft of this document
described `crates/vela-protocol/src/objects/merkle.rs`, an optional RFC
6962-style tree for local event-log proofs; the file was removed with the
predecessor protocol runtime and nothing replaced it. Authority-chain
continuity is checked by the contiguous full-root record chain, not by an
inclusion proof.

The standing constraint on any such primitive, should one return: it would be
an offline protocol primitive, not a service dependency. A derived reader does
not issue signed tree heads, hold a transparency signing key, expose proof
authority, or turn agreement between readers into scientific authority.

The decisive checks remain the exact Git bytes, event signatures and policy
certificates, deterministic replay, and scoped Verification results bound to
source-owned Methods. Core ships no domain verifier.
