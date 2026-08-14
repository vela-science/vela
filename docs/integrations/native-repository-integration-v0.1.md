# Native repository integration draft v0.1

Status: Phase 0 draft with `authority_effect: none`. This is not a Protocol 1
object, wire schema, registry entry, endorsement, or authority initializer.
Source-specific Profiles remain in their native repositories through Phase 1.

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

All documents use closed fields, a schema tag ending in `v0.1`, a full
`sha256:<64 lowercase hex>` root, and the document-specific canonical framing
defined by `conformance/integration-v0.1/README.md`. Unknown versions and fields
fail closed.

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

The draft conformance corpus refuses wrong or shortened roots, mutable identity
claimed as immutable, revision or selector drift, path escape, missing Methods,
unknown schema or Profile versions, wrong root domains, omitted rights or
availability, collapsed mapping and translation, authority fields, build or
review results presented as acceptance, and unavailable evidence converted to
a result.

Native repositories must remain usable if all Vela integration files are
ignored. A cold consumer must need neither Math, authority credentials, private
maintainer context, a hosted Vela service, nor a mutable `latest` reference.
