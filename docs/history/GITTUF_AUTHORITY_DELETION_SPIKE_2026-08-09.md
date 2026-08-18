# Gittuf authority deletion spike

> **Historical deletion study.** This completed 2026-08-09 spike explains why
> gittuf was not adopted. It is evidence for the current no-second-authority
> decision, not current product or integration guidance.

- Status: completed; do not adopt in the current Vela runtime
- Exercised: 2026-08-09
- Gittuf: v0.15.0
- Vela fixture: current unreleased wire contract at `47ff7315`

This is the deletion-oriented comparison required before Vela freezes its
pre-1.0 authority architecture. The question was not whether gittuf and Vela
share words. It was whether gittuf could become the sole owner of generic Git
security and delete enough Vela machinery to justify another permanent trust
and verification layer.

Gittuf is a forge-independent Git security system with repository-local root,
policy, and reference-state metadata. Its own project still describes the
current release as beta and recommends trying it beside existing security
mechanisms. Its rules begin default-allow and become default-deny only for an
explicitly protected namespace. Those are appropriate properties for a Git
publication layer, but adoption here required sole ownership and net deletion,
not a complementary installation. See the current [project status](https://gittuf.dev/),
[policy rules](https://gittuf.dev/documentation/maintainers/policy/rules), and
[consumer verification contract](https://gittuf.dev/documentation/consumers/verifying).

## Exercised fixture

The spike created one disposable Vela Repository with UUID
`587d160b-973e-48f4-86f5-0053ea0a7d2e`, one local bare Git remote, and two
ephemeral SSH signing identities: one authorized gittuf principal and one
negative-case principal. The ordinary Vela loop then produced:

- one authenticated Submission from `agent:gittuf-spike`;
- one independent passing Verification Record from `verifier:gittuf-spike`;
- one exact human `review accept` Decision signed through the standard OpenSSH
  agent; and
- one accepted Claim at repository root
  `sha256:daf1503d8b1d37d0444b0def9c2e0ad8b110d9a0188e8634591194c1f77cbd16`.

Gittuf protected `refs/heads/main` at threshold one. After each Vela-authored
Git commit, the authorized principal recorded the new ref state and
`gittuf verify-ref main` passed. A Reference State Log entry signed by the
unauthorized key failed verification. An unsigned Git commit followed by an
authorized signed RSL entry passed: the protected fact was the attributed ref
transition, not Vela's scientific intent or Decision.

The clean-reader exercise cloned the same local remote without a forge. An
ordinary `git clone` had enough data for strict Vela replay but not for gittuf:
verification failed with `unable to find RSL entry` until the reader explicitly
fetched `refs/gittuf/*`. After that fetch, both verifiers passed and reproduced
the same accepted Claim and Vela repository root.

## Measurements

| Measure | Simplified native Vela | Vela plus gittuf v0.15.0 |
| --- | --- | --- |
| Maintained Vela lines deleted | not applicable | **0** |
| Scientific authority objects | authorization model, keyset, DSSE authority records, Events, Decisions | unchanged |
| Additional Git metadata in the successful fixture | none | 3 live custom refs, 11 RSL commits, 91 loose Git objects |
| Clean-clone verification, tiny fixture | 0.09 s strict replay | 0.57 s after an explicit custom-ref fetch, plus 0.09 s Vela replay |
| Forge-independent verification | yes | yes |
| Unauthorized publication signer | outside Vela's scientific question | rejected by gittuf |
| Passing publication policy implies acceptance | never | no; the Vela Decision remained independently required |
| Key/policy lifecycle | one Vela repository authority and one closed action model | additional gittuf root users, policy administrators, principals, rules, and RSL signing workflow |

Times are single local wall-clock observations on the same machine, useful for
order of magnitude only. The deletion count is the deciding measurement.

## What could and could not move

Gittuf successfully owns a broader generic question: whether a Git ref update
followed repository publication policy. It cannot replace the Vela objects that
answer who made one scientific Decision, under which exact authorization,
against which Proposal, read set, current root, intended consequence, and
before/after Standing. Deleting those would delete the product's semantic
boundary.

It also does not replace Vela's local compare-and-swap transaction. That check
prevents a stale or raced scientific write before installation; gittuf verifies
the recorded publication history. Keeping both is complementary defense, not
deletion. Re-expressing Vela's `review_accept`, `review_reject`, authority
rotation, and exact Resource context as Git path/ref rules would require a Vela
adapter while preserving the current scientific evaluator, increasing the
independent-reader burden.

The upstream trust architecture intentionally adds root users, separate policy
administrators, delegations, and thresholds. Vela deliberately needs none of
that ceremony for its current one-authorized-operator repository profile. The
gittuf [trust architecture](https://gittuf.dev/documentation/maintainers/design)
is valuable for repositories that need those controls; unused expressiveness
is not a reason to duplicate them here.

## Decision

Keep the simplified native Vela authority architecture and do not add gittuf to
the current runtime, repository format, mirror contract, or operator loop. The
spike produced zero deletion, added a second root and policy lifecycle, required
custom-ref synchronization beyond ordinary clone, and still required every
Vela scientific authority check.

Reconsider only if a stable gittuf release can become the sole owner of a
demonstrated generic Git-security requirement, works with Vela's standard
OpenSSH-agent custody without a second signing ceremony, is carried by ordinary
retention and clean-clone workflows, passes the Vela adversarial authority
fixtures, and deletes substantially more maintained Vela code than its adapter
adds. Until then, forge rules or optional external gittuf verification may
complement publication without entering Vela's protocol or acceptance claims.
