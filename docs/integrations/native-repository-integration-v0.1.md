# Native repository integration v0.1

Status: stable package-plane integration waist with `authority_effect: none`;
not a Protocol object, registry, endorsement, or initializer. Profiles stay
source-owned.

## Contract chain

```text
Manifest -> Profile -> Binding -> Method
```

- **Manifest:** the root `vela.toml` repository instance, exact native revision
  rule, rooted inventory, rights, availability, retention, and outputs.
- **Profile:** what conformance means, under an immutable versioned identifier
  and exact root.
- **Binding:** how exact native objects in this repository satisfy or expose one
  Profile, including mappings, translation dispositions, and required Methods.
- **Method:** how one property is checked, including exact implementation,
  environment, inputs, outputs, limitations, and nonclaims.

Documents use a closed shared envelope, a `v0.1` schema tag, a full lowercase
SHA-256 root, and the canonical framing in the conformance README. Core closes
the envelope, inventory, and Exact Reference shapes. It treats optional
source-owned objects as opaque except for shared authority and unavailable-result
refusals; native validators close and interpret those objects. Unknown versions
and fields fail closed at their owning contract.

## Exact Reference

An Exact Reference is a value shape, not a Protocol object:

```text
native_identity: system, object_kind, identifier
revision:         kind, value
content_fixity:   media type, SHA-256 digest, byte size
selector:         optional kind and value
locator:          URI, mutability, authentication requirement
```

Native identity, revision, fixity, selector, locator, authentication, and
authority must not be collapsed. A mutable locator is permitted only beside an
independently exact revision and fixity; it is never described as immutable.

## Mapping and translation

A Binding records semantic mapping relations separately from information
survival:

```text
mapping relation: exact | close | broader | narrower | related
translation disposition:
  preserved | normalized | derived | approximated | omitted |
  unsupported | assumed | unresolved
```

A successful parse, mapping, build, review, or check does not establish
semantic equivalence or scientific acceptance.

## Rights, availability, results, and provenance

Every Manifest and Profile states rights. Every Manifest states availability,
access, retention, and observation time. Unknown rights use an explicit value
such as SPDX `NOASSERTION`; unavailable or restricted evidence remains so. It
is never converted to pass, fail, or numeric zero.

An integration result may report only a scoped check outcome and explicit
nonclaims. It cannot carry a Decision, Event, accepted Standing, authority key,
Repository policy, or acceptance result. Outputs are limited initially to
Exact References, Submission drafts, and Verification inputs.

Consequential work keeps responsible Agent, Activity, model and tool Entities,
and Role separate. Actor kind does not determine quality. Method, inputs,
outputs, independence, scope, limitations, and nonclaims do.

## Required refusal behavior

The conformance corpus refuses wrong or shortened roots, mutable identity
claimed as immutable, revision or selector drift, path escape, missing Methods,
unknown schema or Profile versions, wrong root domains, omitted rights or
availability, collapsed mapping and translation, authority fields, build or
review results presented as acceptance, and unavailable evidence converted to
a result.

Native repositories must remain usable if all Vela integration files are
ignored. A cold consumer must need neither Math, authority credentials, private
maintainer context, a hosted Vela service, nor a mutable `latest` reference.

## Stable qualification

The structural waist qualified against two maintained, published consumers.
The exact reviewed heads are:

| Repository | Commit | Tree | Manifest root |
| --- | --- | --- | --- |
| Vela Core | `bea4ec2af0772e366a0670d49a10b7085a4c73c1` | `5b95e762e2560c56d1cdb0abd05255c706b47d1e` | not applicable; Core is not an Integration Manifest |
| `williamjblair/lean-proofs` | `06d1322e62aa28b860da1ec66465d913c1902c78` | `572395b76976c0b6940cbc58c15512adbc36a328` | `sha256:b56bc2cd1107c0e85f414a8c15d4a1dd561c36b61e6d5dad98ff6c274281a434` |
| `williamjblair/formal-conjectures` contributor fork | `3add37729550480fea34f67690ec708c549f524b` | `39b825f30aa9bfc43ef1acfded6e88c2a9bbbce5` | `sha256:7048a8aa1176c87d48b48557d366b506ba1520b9d1771adb70d1f5ed13e0ae2e` |

The final rooted consumer inventories are:

| Consumer | Profiles | Bindings | Methods |
| --- | --- | --- | --- |
| `lean-proofs` | `sha256:3c9f08205adf4059b7ea06ce547b78989075d0582d542270de86bd68248539ac`, `sha256:8358edad46299b717673061f21a607be30ecb8f2224a438163489870edd5d2d0` | `sha256:f34d4ab06923c942ba396c866ac5347231cdb90a7b5d623bbf1ed6e3042b4c8a`, `sha256:2a896bdc0a1ff5ac188c8a9bf5a2c5dad3a042af851d7ecefefc4f95221c19ef` | `sha256:d758f77d782611e23e633f1bb4640004d39f5ce61df50f6890a17f030fba09f9`, `sha256:1ee9b6d7d629e673639992a66b944a2a789664b999086d9fe1f7343c6296649c`, `sha256:fc96ed80cbdfc29461c3551896b3f46c00249add3dd0e62b2b40a5343586d893` |
| Formal Conjectures fork | `sha256:3c1a50de5f98899c5f85897449ca69484086489cf8dc657f5c2fcf7cee4b8eab`, `sha256:6207334bb4e00a033b21012bbac1f9a4413d845960c5a48787e42dceff703a45` | `sha256:01476435ebe8f7359d608b37e8d48595f1d694e41f960e287ce77d74f43619e5`, `sha256:08800e062dd8797742a6057b48c1c6b1a173ba3282eaef0c21341c73394ca712`, `sha256:6d5be0337c6a9616bc25145ef973764f24c04ae8fb84e06f28c7b4e4fa4af8c2` | `sha256:35cad804504ea2371e2c849c840b28f33506a5f4208c5b2352bad6c88532161e`, `sha256:84a3a40ef361206dc6c062066ea3bc4ded4dbc0721dea1d04153f4854fc518d6`, `sha256:e7f0f3152f5409e5e6e8eedf94d2c18141c860e981c418486cf74d1e668b264d`, `sha256:0a5c29f9ce0d53ce029e963aaf1fa782c7c88d00c3d46074061e55405d0ee944`, `sha256:b04be736f7a37d43e5d863ee33ba942658d8659a13834d99e3e9adc9da41e311`, `sha256:656d9fb5b48c69f53b913f6ed4cd9b26637074aa85880b6dc73d93b32fdcbc53` |

The extraction gate measures the maintained generic validation replaced by
Core, not later source-owned pinning, workflow, or cold-cache custody. From Core
baseline `6c5d050f5cbc4ab4a3fe2f06c8eff3775e9fb9b1` to the published Core head, the
Core tranche adds 1,165 lines and deletes 686 (`+479`). Delegation plus retained
source containment deletes 105 net lines in `lean-proofs` from
`fe4dd4a4089c9c94493e0c7a8e01c129b3f2a018` through
`16eb45ba283eec49bbd206bb86e97042aafc80ee`; Formal Conjectures deletes 375
net lines from `f8bd3dd2fd3065e4922ec169aac59a04595a5f7b` through
`5097f0253b521911166c8db470b50489ec94190f`. The shared extraction therefore
deletes one more maintained line than it adds. The later commits pin published
Core, add source-owned enforcement, and isolate the cold consumer; they do not
restore the deleted generic validators.

## Extraction boundary

The consumers proved the four rooted documents, Exact Reference, inventories,
mapping/translation separation, disclosures, and authority refusal in common—not
a scientific Profile. Lean retains proof/axiom semantics; Formal Conjectures
retains audit, review, condition, and fidelity semantics. `integration check`
validates the structure and `integration inspect` renders it. Neither runs
Methods or replaces source validation; pilots justify no Profile in advance.
