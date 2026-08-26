# VELA-RC-1 release checklist

No checklist item authorizes release.

## Candidate identity

- [x] exact qualifying HEAD and tree recorded
- [x] working tree clean
- [x] version decision recorded
- [x] Protocol and schema versions recorded
- [x] release bytes traceable to qualified source

## Mandatory gates

- [x] R1 semantic audit passes
- [x] authoritative conformance matrix passes
- [x] full local Core union passes
- [x] clippy release-candidate gate passes
- [x] R2 clean install passes on a supported environment
- [x] deterministic public replay fixture passes with the expected accepted
      set, Repository root, and authority Event-log root
- [x] corrupt/missing Artifact fixture fails closed
- [x] R3 first-user documentation and CLI gate passes
- [x] R4 two-domain examples pass without a Core fork
- [x] R5 product source does not misrepresent protocol semantics
- [x] R6 package, provenance, license, locks, CI, and migration audit passes
- [x] R7 blind test passes with recorded first-user limitations

## Release integrity, only after later user authorization

- [ ] rerun exact hosted conformance
- [ ] rerun clean-install fixture
- [ ] generate honest release notes and limitations
- [ ] build exact authorized tag through `scripts/release.sh`
- [ ] retain checksums, manifests, SBOMs, attestations, and supported signatures
- [ ] verify released source digest equals the qualified source digest

Current release status: `READY FOR USER AUTHORIZATION WITH LIMITATIONS`.
