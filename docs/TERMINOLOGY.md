# Vela terminology

This document keeps Vela's product and architecture language small
and consistent. Normative protocol names remain governed by the Vela protocol,
accepted ADRs, and released schemas. These definitions do not create new
canonical objects or authority.

## The product story

```text
produce -> preserve -> check -> decide -> reuse
```

| Verb | Owner | Meaning |
| --- | --- | --- |
| Produce | Any suitable workbench; optionally Canopus | Create bounded work, artifacts, and verifier evidence. |
| Preserve | One canonical Frontier Git repository | Retain exact objects, roots, and history. |
| Check | Vela | Replay bytes, roots, verifiers, relations, and strict signals. |
| Decide | Existing Vela policy or protected human decision | Change scientific standing through the sole authority boundary. |
| Reuse | Observatory or another removable reader | Inspect, cite, reproduce, compare, and continue. |

## Scientific records

| Term | Meaning | Not the same as |
| --- | --- | --- |
| **Problem** | A bounded scientific question or target that can organize claims, obligations, and attempts. | A claim, work offer, or accepted result. |
| **Claim** | A scientific assertion whose exact revisions can be retained and evaluated. | A label, topic, verifier result, or decision. |
| **Claim revision** | One exact, content-addressed version of a claim and its conditions. | The stable readable finding handle. |
| **Finding** | Vela's retained claim record, including revision and standing projections. | An automatically true statement. |
| **Obligation** | A specific unresolved requirement needed to advance or assess a claim. | A generic task or project milestone. |
| **Attempt** | One bounded effort against an obligation or work target. | Its outcome, artifact, or scientific acceptance. |
| **Trajectory** | An ordered, rooted sequence of attempts and resulting state changes. | Raw private model history. |
| **Artifact** | Retained bytes with exact identity and provenance. | Evidence merely because it exists. |
| **Evidence** | An artifact or observation offered for or against a claim under explicit scope. | Verification or authority. |
| **Verifier observation** | A reproducible result from a named verifier over exact inputs. | Scientific acceptance. |
| **Proposal** | An exact pending request to change scientific standing. | The evidence or a decision. |
| **Decision** | An authorized accept, reject, withdraw, correct, supersede, or other governed action. | Verification success or publication. |
| **Standing** | Vela's replayed status for an exact record under the governing events and policy. | Confidence, popularity, graph rank, or universal truth. |
| **Correction** | A retained state transition that changes current standing without erasing prior history. | Editing or deleting old canonical records. |

## Repositories and collections

| Term | Meaning | Boundary |
| --- | --- | --- |
| **Frontier** | One ordinary Git repository with one stable Vela identity, scientific history, and authority/correction boundary. | Split when authority, correction policy, confidentiality, namespace, or stewardship diverges. |
| **Boundary** | The exact point at which identity, authority, correction, or interoperability rules change. | A visual grouping or database partition. |
| **Catalog** | An attributed enumeration of problems, objects, or sources. | Evidence that each entry is true or unresolved. |
| **Topic** | A discovery concept used to organize or search records. | A scientific claim. |
| **Field** | A maintained intellectual area with recognizable methods and vocabularies. | A fixed protocol namespace. |
| **Domain** | A packageable semantic and verification context, such as graph theory or stabilizer quantum codes. | An authority boundary by itself. |
| **Ecosystem** | The composed tools, repositories, packages, readers, and people around Vela. | A second canonical database or protocol. |
| **Constellation** | A rooted, curated collection of related Frontiers or records for a question or narrative. | A merged Frontier or inferred authority. |
| **Atlas** | A removable cross-Frontier navigation and comparison view. | Canonical scientific state, a graph authority, or a hosted mutation service. |

## Analysis and interoperability

| Term | Meaning | Authority effect |
| --- | --- | --- |
| **Vela Kernel** | The shipped protocol boundary: exact objects, roots, transitions, replay, verification state, and authority. | Sole owner of replayed standing and authorized transitions. |
| **Frontier Algebra** | A root-bound, read-only derivation of exact support/opposition routes, corrections, cut sets, and repair requirements. | None. |
| **Discovery Calculus** | Optional root-bound information and decision lenses for choosing research actions. | None; rankings remain advisory. |
| **Accepted Information Channel** | A lens model of raw result → verifier outcome → authority outcome → accepted delta or explicit null outcome. | Descriptive only. |
| **Semantic package** | A content-addressed, versioned set of terms, constraints, mappings, fixtures, licenses, and generated interoperability artifacts. | None. |
| **Transition envelope** | The minimum cross-system record needed to retain exact identity, scope, evidence, check results, residual uncertainty, and a proposed state change without replacing domain semantics. | None until an ordinary authorized Vela transition occurs. |
| **Verification scope** | The exact claim, inputs, method, environment, and property covered by one verifier observation. | None; it bounds what the observation means. |
| **Assurance profile** | A versioned description of which assurance dimension a named check addresses, its prerequisites, and what a pass does not establish. | None. |
| **Independence disclosure** | Retained facts about shared models, code, data, specifications, maintainers, and other possible common-mode causes across producers or verifiers. | None; it is evidence for a policy or reviewer, not a global score. |
| **Vocabulary** | A controlled set of discovery or domain terms. | None. |
| **Mapping** | A versioned relation between exact package terms with a declared consequence tier. | None by itself. |
| **Bridge** | A maintained set of mappings between domains, including every premise and scope needed for permitted transport. | None by itself. |
| **Transfer** | A typed transformation or transport from one exact scientific context to another. | Only the explicitly certified consequence; never automatic acceptance. |
| **Adapter** | A replaceable producer-edge translation from an exact workbench export to explicit Vela-compatible artifacts plus a loss report. | None. |
| **Reader** | A removable projection for inspection, search, comparison, or reproduction. | None. |
| **Lens** | A rooted model that selects a view, metric, or action ordering under declared assumptions. | None. |

## Mapping consequence tiers

Mappings must state one consequence tier. The default is `discovery`.

| Tier | Permitted consequence |
| --- | --- |
| `discovery` | Search, navigation, and candidate association. |
| `organization` | Grouping under an explicit scheme. |
| `identity` | Attributed co-reference without merging canonical Vela objects. |
| `logical_transport` | Transport through an exact checkable transformation and all premises. |
| `empirical_transport` | Transport under an explicit causal or measurement model, scope, uncertainty, and calibration evidence. |

Shared labels, embeddings, graph proximity, `skos:exactMatch`, or
`owl:sameAs` never automatically transport standing.

## The three distinctions that must remain visible

```text
artifact integrity != verifier success
verifier success != scientific acceptance
package or mapping validity != scientific standing
multiple checks != independence without disclosed lineage
```

Likewise:

```text
publication != acceptance
database presence != canonical identity
graph position != importance or authority
```
