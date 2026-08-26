# VELA-RC-1 release checklist

No checklist item authorizes release.

## Candidate identity

- [ ] exact qualifying HEAD and tree recorded
- [ ] working tree clean
- [ ] version decision recorded
- [ ] Protocol and schema versions recorded
- [ ] release bytes traceable to qualified source

## Mandatory gates

- [ ] R1 semantic audit passes
- [ ] authoritative conformance matrix passes
- [ ] full local Core union passes
- [ ] clippy release-candidate gate passes
- [ ] R2 clean install passes on a supported environment
- [ ] deterministic public replay fixture passes with the expected accepted
      set, Repository root, and authority Event-log root
- [ ] corrupt/missing Artifact fixture fails closed
- [ ] R3 first-user documentation and CLI gate passes
- [ ] R4 two-domain examples pass without a Core fork
- [ ] R5 product surfaces do not misrepresent protocol semantics
- [ ] R6 package, provenance, license, locks, CI, and migration audit passes
- [ ] R7 blind test passes, if authorized by its dependency gate

## Release integrity, only after later user authorization

- [ ] rerun exact hosted conformance
- [ ] rerun clean-install fixture
- [ ] generate honest release notes and limitations
- [ ] build exact authorized tag through `scripts/release.sh`
- [ ] retain checksums, manifests, SBOMs, attestations, and supported signatures
- [ ] verify released source digest equals the qualified source digest

Current release status: `NOT AUTHORIZED`.
