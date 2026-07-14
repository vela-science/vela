# RO-Crate and immutable software locators

Vela proof packets include an RO-Crate-shaped metadata projection so ordinary
research-object tools can discover the packet and its files. This is export
metadata, not Vela authority. Verify the packet manifest, event log, and
signatures separately.

The exporter uses the official [RO-Crate 1.2
context](https://w3id.org/ro/crate/1.2/context). Its metadata descriptor names
the 1.2 specification and points `about` to the Root Data Entity:

```json
{
  "@id": "ro-crate-metadata.jsonld",
  "@type": "CreativeWork",
  "conformsTo": {"@id": "https://w3id.org/ro/crate/1.2"},
  "about": {"@id": "./"}
}
```

Compatibility note: Vela's packet v1 historically names this canonical file
`ro-crate-metadata.jsonld`. RO-Crate 1.2 specifies
`ro-crate-metadata.json`. The projection therefore uses the official graph
shape and context but does not yet claim strict filename-level RO-Crate 1.2
conformance. Renaming a canonical packet file needs an explicit packet-version
migration with legacy validation, not a silent rewrite.

The Root Data Entity describes the proof packet as a `Dataset`, lists packet
members with `hasPart`, and retains Vela's own schema separately. External
artifact entities preserve their source locator, media type, license, Vela
content hash, and retirement state when those fields are public.

## SWHID examples

Software Heritage persistent identifiers are useful immutable locators for
Git-shaped software objects. The official SWHID 1.2 grammar is:

```text
swh:1:<cnt|dir|rev|rel|snp>:<40 lowercase hex>
```

Official specification examples include:

```text
swh:1:cnt:94a9ed024d3859793618152ea559a168bbcbb5e2
swh:1:dir:d198bc9d7a6bcf6db04f476d29314f157507d505
swh:1:snp:c7c108084bc0bf3d81436bf980b46e98bd338453
```

See the [SWHID 1.2 core identifier
specification](https://www.swhid.org/specification/v1.2/5.Core_identifiers/).
These are examples from the standard, not identifiers for Vela or this
frontier.

For a Vela receipt, use an actual computed SWHID only when the named object was
resolved or independently computed. Preserve the full core identifier and any
`origin`, `visit`, `anchor`, or `path` qualifiers. A Git commit OID, SWHID, URL,
and Vela SHA-256 content root are different facts; never relabel one as another.

## Generate and verify

```bash
vela proof examples/sidon-sets --out /tmp/sidon-proof-packet
vela packet validate /tmp/sidon-proof-packet
jq '."@graph"[0:2]' /tmp/sidon-proof-packet/ro-crate-metadata.jsonld
```

No RO-Crate or SWHID field accepts a claim, upgrades a verifier result, or
proves semantic faithfulness.
