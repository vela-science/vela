# POSI 2.0 self-assessment

Assessment date: 2026-07-14. Status: project self-assessment, not a claim of
formal POSI adoption.

This assessment uses the [Principles of Open Scholarly Infrastructure
2.0](https://openscholarlyinfrastructure.org/), released in October 2025 and
last modified 2026-05-26. POSI permits initiatives without a legal entity to
self-assess. Vela currently has strong technical insurance properties and weak
institutional and financial ones. The honest conclusion is **partial, not
adopted**.

## Governance

| Principle | Evidence now | Gap and owner | Revisit trigger |
| --- | --- | --- | --- |
| Coverage across scholarship | Domain-neutral Receipt v1, Git frontier model, and verifier adapters | Current examples overrepresent formal mathematics and software. Project steward owns broader pilots. | Two maintained non-math frontiers |
| Stakeholder governed | Authority is frontier-local and key-bound; forks do not depend on a central service | No stakeholder board or representation policy. Human project owner. | Before a hosted service becomes relied upon |
| Non-discriminatory participation | Public dual-licensed code, documented receipt boundary, ordinary Git contribution | No adopted membership or accessibility policy. Human project owner. | Before formal membership or hosted access tiers |
| Transparent governance | Protocol authority and signed policy behavior are public and replayable | Legal, financial, appointment, conflict, and appeal processes are not established. Human project owner. | Before accepting money or institutional dependencies |
| Cannot lobby for narrow self-interest | No lobbying program | No adopted advocacy policy. Human project owner. | Before any policy advocacy |
| Living will | Git clone, standard bundle, open formats, and exit drill reduce technical lock-in | No approved successor criteria, asset-transfer plan, staff plan, domain continuity plan, or archival deposit. Human decision required. | Before external reliance |
| Review purpose and community value | ADR primitive budget and outside-producer gates resist expansion | No scheduled public review or accountable body. Project steward. | First external production dependency, then annually |

## Sustainability

| Principle | Evidence now | Gap and owner | Revisit trigger |
| --- | --- | --- | --- |
| Transparent operations | Public roadmap, ADRs, conformance, security policy, and release history | No public finances, staffing map, service levels, or operating metrics. Future operator. | Before paid operation |
| Time-limited funds for time-limited work | No grant-funded core operation is claimed | No funding policy. Future governing body. | Before accepting grants |
| Generate surplus | Not applicable to the current non-service project | No operating model. Future governing body. | Before hosted service launch |
| Reserve policy | None | Define purpose, minimum, maximum, custody, review, and wind-down access. Future governing body. | Before recurring revenue |
| Mission-consistent revenue | Technical split permits paid services without owning protocol bytes | No adopted revenue review policy. Future governing body. | Before first paid contract |
| Revenue from services, not data | Architecture supports hosting, support, and service-level revenue | No binding policy preventing sale of community data. Human decision required. | Before commercial operation |
| Recognize volunteer labor | Git history preserves technical authorship | No systematic time accounting, risk valuation, or public recognition process. Project steward. | First annual operations report |
| Transition planning | Reproducible build, conformance, ordinary Git, documented exit | Bus factor remains one for authority, release, and project direction. Appoint a second steward before reliance. | Any external production dependency |

## Insurance

| Principle | Evidence now | Gap and owner | Revisit trigger |
| --- | --- | --- | --- |
| Open source | Reference implementation is Apache-2.0 OR MIT | Audit third-party assets and any future hosted control plane. Release steward. | Every release |
| Open and secure data access | Public state is cloneable; restricted receipts expose opaque custodian references without payload or equality digest | No approved restricted-data transfer, retention, breach, custodian succession, or deletion policy outside the technical rules below. Data steward required. | Before storing third-party restricted data |
| Available and preserved | Git transport, bundles, checksums, source archive metadata, offline replay | No trusted third-party archival deposit or preservation SLA. Project steward. | First citable stable release |
| Patent non-assertion | No patent claim is made in the repository | No adopted covenant; this is a human/legal decision. | Before formal adoption or outside reliance |
| Interoperability and open standards | Git, JSON, SHA-256, Ed25519, DSSE/in-toto, RO-Crate export, SWHID examples, Receipt v1 conformance | Independent adoption evidence and long-term compatibility policy remain incomplete. Protocol steward. | Two producers and two consumers |

## Data export and transfer policy

Public canonical state, public receipts, public artifact bytes, schemas,
conformance fixtures, and documentation may be exported according to their
recorded licenses. Export preserves content roots, source licenses, provenance,
and authority events. A locator is not substituted for a digest.

Restricted material is different:

- public records contain only the allowed opaque custodian reference and typed
  availability metadata;
- no export may add a payload, opening, machine-local path, equality digest,
  byte size, secret, token, or resolvable location;
- a transfer requires an identified lawful custodian, purpose, access list,
  retention period, secure transport, deletion procedure, and incident owner;
- if those facts are absent, Vela transfers the public opaque reference only;
- deleting private payloads must not rewrite historical public events. A later
  signed retirement, correction, or availability event records the change.

The reference implementation cannot by itself supply legal authority, consent,
institutional approval, or a successor custodian.

## Governance trigger

Before any hosted Vela service becomes a dependency for an outside group, the
project must appoint a second steward with documented release and incident
authority, publish succession and conflict procedures, run the exit drill with
that steward, and place recoverable public assets with a trusted third-party
archive. Until then, hosted components are experimental conveniences.

## Human-owned open questions

- Approve successor criteria and a living will.
- Decide whether to adopt POSI formally and on what review cadence.
- Decide a patent non-assertion covenant.
- Define legal ownership, financial reserves, and mission-consistent revenue.
- Assign a restricted-data steward and incident process.

No agent, fixture, commit, or protocol event resolves these institutional
decisions.
