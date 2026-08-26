# Vela organization: migration architecture

Status: canonical evidence-bound migration plan for VELA-ORG-1

This plan moves from [the observed current architecture](VELA_ORG_CURRENT.md)
to [the target architecture](VELA_ORG_TARGET.md). It authorizes no deployment,
DNS change, repository mutation, authority write, archive, rename, or deletion
by itself. Each tranche starts only when its stated evidence gate passes.

## Invariants for every tranche

- Vela `0.977.5`, its tag object, release commit, release tree, and Protocol 1
  manifest root remain immutable historical state.
- Math authority commit
  `cf6d76687b205a39e2515e9fec7087c819454d2f` and repository root
  `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`
  remain unchanged while the reader contradiction is repaired.
- A version label is not binary identity. Every active repin records the exact
  accepted binary digest and source/release identity.
- Historical receipts keep their original version and digest bindings.
- Problems remains frozen at
  `532241ba5db565e9ee35e13cbd7eff76393f6475` through WebMCP submission and a
  subsequent 24-hour exact-SHA stability window.
- WebMCP retains `authority_effect:none` and cannot sign or issue Verification,
  Decision, or Event records or mutate Standing.
- No archive, history, stash, local object, deployment, DNS record, or rights
  evidence is destroyed as an incidental cleanup step.
- Any failed gate stops its dependent tranches and records the failure as a
  blocker; it is not worked around by broadening scope.

## Dependency order

```text
G0 freeze exact baselines
 |
 +--> G1 O5 clean Math replay --> G2 downstream release/digest repins --+
 |                                                                    |
 +--> G3 WebMCP submit + 24h exact-SHA freeze -------------------------+
                                                                      |
                 G4 projection custody and rights + licensing --------+
                                                                      |
                 G5 www extraction and deployment parity -------------+
                                                                      |
                 G6 user-approved cutover and cleanup
```

G1 and G3 may progress independently, but G4 requires both. The later gates
are strictly ordered because they mutate shared Problems/`vela-web` custody or
public provenance.

## G0 — Freeze exact baselines

Evidence required:

- preserve the immutable Core release identities listed in the current map;
- record exact source commit, tree, working-tree state, remotes, stashes, and
  deployment SHA for every repository or project touched by a later tranche;
- export or otherwise retain provider configuration receipts without exposing
  secrets; and
- prove that the candidate diff for each tranche is limited to its named owner.

Pass condition: the tranche can be reverted or audited from exact Git and
provider receipts without using mutable labels such as `main`, `latest`, or a
version string alone. If a provider fact cannot be observed, mark it **UNKNOWN**
and stop any dependent destructive or provenance-changing action.

## G1 — O5 clean Math replay

O5 owns the smallest Core-side reconciliation of the two retained negative
credential-scan reports with strict replay/status parity.

Evidence required:

1. focused Core tests cover the intended retained-negative-report behavior and
   hostile positive, malformed, substituted, and ambiguous cases;
2. `vela replay` and `vela status` agree under the candidate reader;
3. a clean full clone at exact Math commit
   `cf6d76687b205a39e2515e9fec7087c819454d2f` replays to exact root
   `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4`;
4. the qualification binds the candidate reader's commit, tree, version, and
   binary digest; and
5. `git diff` and root comparison prove zero Math authority-record mutation.

Pass condition: clean-clone replay succeeds fail-closed and status/replay parity
is demonstrated at the frozen Math identity. If the repair needs a schema,
Protocol 1, authority-model, or Math-record change, G1 fails and the campaign
returns to the supervisor for a new decision.

## G2 — Downstream `0.977.5` or qualified-patch repins

This gate begins only after G1. The repin target is the exact reader qualified
by G1: released `0.977.5` if no new release is needed, otherwise an explicitly
qualified patch release. The patch identity is **UNKNOWN** until release
qualification; no document may guess it.

Apply bounded owner-local changes in this order:

1. Workbench: update the release pin, accepted binary digest, exact Math
   fixtures, and docs; add only the narrow status-backed readiness guard already
   supported by the audit.
2. Projection/reconstruction: update the accepted generator/reader digest and
   reproduce an exact candidate release from the same source set before moving
   any current-release pointer.
3. Active local/operator instructions and Math documentation: point to the
   qualified current reader without rewriting historical receipts.
4. www redirects: defer to G5 so the public link change ships with the parity
   cutover rather than as an unrelated deployment.

Pass condition: each consumer's focused tests pass, clean Math replay remains
exact, projection roots are reproduced or deliberately versioned with an
explained source change, and every active binary is invoked or accepted by
digest. DTOs, handoff schemas, Protocol, authority behavior, Workbench version,
and historical fixtures remain unchanged unless a separate decision authorizes
them.

## G3 — WebMCP submission and 24-hour freeze

G3 is independent of the Core repair and does not permit challenge-lane churn.

Evidence required:

- record the WebMCP submission receipt and its exact Problems source SHA;
- verify the submitted and production source remains
  `532241ba5db565e9ee35e13cbd7eff76393f6475`;
- start the stability clock only after submission is complete; and
- observe at least 24 continuous hours with that exact SHA in source and
  production and no emergency rollback or authority-boundary incident.

Pass condition: submission plus the full exact-SHA window is recorded. A new
commit, redeploy from a different SHA, rollback, or unresolved incident resets
the clock. No adapter, licensing, deployment, alias, or structural cleanup may
enter Problems before this pass.

## G4 — Projection asset custody, rights, and licensing

G4 starts only after G1, G2, and G3 pass. It separates release reconstruction
from private `vela-web` without changing scientific authority.

### Projection custody and rights review

Evidence required:

- enumerate every source adapter, authenticated asset, credential expectation,
  generated file, source license, and private package required to reconstruct
  the current projection release;
- identify the canonical owner of each byte and the right to publish, keep
  private, replace, or regenerate it;
- move or replace only assets whose custody and rights are proven, preserving
  exact historical release inputs where redistribution is not permitted;
- run a clean reconstruction from public Problems plus explicitly documented
  provider credentials, with no checkout or package import from `vela-web`;
- reproduce the expected projection root, or produce a new root whose complete
  input delta is reviewed; and
- prove the resulting Problems release path does not grant an adapter,
  database, Vercel, or WebMCP any Standing effect.

### Licensing closure

Evidence required:

- retain written permission evidence for the six published ITF/Fontshare WOFF2
  files or replace them and perform a reviewed history/purge decision;
- include the complete IBM Plex Mono OFL and copyright notice with redistributed
  files; and
- keep Tailwind Plus/shadcn.io Pro shared UI private and exclude component
  registry and lab-catalogue material from public output.

Pass condition: a rights ledger covers every redistributed asset, a clean
release reconstruction is self-contained at its declared public/private
boundary, license checks pass, and counsel or the designated rights reviewer
has no unresolved blocker. Written permission status is currently **UNKNOWN**;
absence of evidence is a failure, not permission.

## G5 — Tiny www extraction and deployment parity

G5 starts only after G4. Extract only the live `vela.space` behavior needed for
a minimal static orientation surface. Do not carry forward the old Problems
application, projection runtime, authentication, hosted activity, database
access, WebMCP, private UI catalogue, or speculative pages.

Evidence required:

1. define the exact route, content, asset, redirect, metadata, and accessibility
   parity set from deployed `vela.space@04741101bddf01c95a7e60145ab970f45b0ab30a`;
2. identify a rights-safe dependency closure and its canonical source owner;
3. prove a clean install and build from that owner without private
   `vela-web` dependencies or unreviewed fonts;
4. compare old and candidate routes, redirects, rendered content, status codes,
   metadata, assets, and mobile/desktop behavior;
5. update Vela links to the qualified `0.977.5` or patch identity and exact
   install boundary;
6. deploy a non-production candidate and retain exact source/deployment
   provenance; and
7. demonstrate rollback to the existing deployment before any relink.

Pass condition: the candidate meets the declared parity set, introduces no
stateful or authority surface, and has an evidence-backed source owner. Source
placement remains **UNKNOWN** until this gate proves it; no repository creation
or rename is implied.

## G6 — User-approved cutover, archive, rename, and cleanup

Technical qualification does not grant permission for externally visible or
destructive changes. Present one exact change set and rollback plan for user
approval before each applicable action:

- relink `vela.space` to the qualified www deployment and close the old
  `vela-web-www` rollback window;
- repair or remove the dangling `app.vela.space` CNAME, old deploy hook, alias,
  or provider project;
- change the custody or public status of `erdos.constellate.science`;
- archive, rename, change visibility, or otherwise retire `vela-web` or any
  other repository;
- rename a public repository or domain;
- remove the old Vercel `vela-web-problems` rollback project or its environment
  configuration;
- dispose of the two `vela-web` stashes; or
- delete local clones, worktrees, registrations, refs, objects, research
  directories, provider data, or legacy Observatory data.

Before approving `vela-web` retirement, prove that it owns no live deployment,
required projection-reconstruction asset, rights evidence, rollback obligation,
or unique stash/object history. Archive is preferred to deletion unless a
separate evidence-backed deletion review establishes otherwise.

Pass condition: the user approves the exact targets, the approved action is
performed with retained receipts, public parity and rollback checks pass, and
the post-change inventory is refreshed. Unapproved items remain untouched and
are reported as unresolved, not silently treated as target completion.

## Decision and evidence register

| Decision | Required before | Current state |
| --- | --- | --- |
| O5 repair qualifies without Protocol/authority/Math mutation | G2 | pending |
| Exact qualified patch identity and digest, if `0.977.5` is insufficient | G2 | **UNKNOWN** |
| WebMCP submission receipt and completed 24-hour window | G4 | pending |
| Public projection asset owner and redistribution rights | G4 | **UNKNOWN** |
| ITF/Fontshare permission or replacement/purge disposition | G4 | **UNKNOWN** |
| Rights reviewer acceptance of the complete asset ledger | G5 | pending |
| Existing canonical owner for tiny www source | G5 | **UNKNOWN** |
| Exact www parity set and rollback duration | G5/G6 | **UNKNOWN** |
| DNS, hook, alias, and domain disposition | G6 | user decision required |
| Repository archive, rename, or visibility changes | G6 | user decision required |
| Stash, worktree, clone, ref, object, directory, and legacy-data cleanup | G6 | user decision required |

The migration is complete only when every target completion property has
evidence and the inventory is re-frozen. A green check verifies its scoped
gate; it is not by itself a scientific Decision, release publication, rights
approval, deployment cutover, or user authorization.
