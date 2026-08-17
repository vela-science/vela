# Provider-loss qualification, 2026-08-09

Status: **pass for the current Vela 0.972.1 ecosystem**.

This is exercised continuity evidence, not a new protocol object and not a
scientific Decision. It records the first complete continuation after the Math
authority's UUIDv4 re-genesis.

## Inputs

- Vela release: `v0.972.1`, installed from the retained signed distribution
  assets and verified without treating the release host as a trust root.
- Math replica: `https://codeberg.org/vela-science/math.git` at commit
  `130fc283b99b8c55dea51b5f8f959a6c33a679f6`, after the replica job verified
  every ref against the active writer.
- Repository UUID: `8115c538-7688-40b7-ab75-3c4765bf3c19`.
- Retained source-adapter set:
  `sha256:9d90e127c8a21670e5f1fb479a9535211b32604b9482c24998d27d179546d57e`.
- Retained source-adapter artifact:
  `sha256:3f34bb1244cf135a7eaa65aa1f3ba67c879998a954f558f07ab24e9297626675`.
- Web projector commit:
  `c2bbadfc260cbd6e81ee013ebb9237790c8b025b`.

The human Decisions were made through the local repository-authority path. No
hosted service signed them or changed Standing. Publication to the active writer
and replication happened afterwards.

## Exercised results

1. An anonymous clone from Codeberg reproduced Math commit
   `130fc283b99b8c55dea51b5f8f959a6c33a679f6` and tree
   `3c99d1b9c969a8559605a664bdd7280e9729169f`.
2. Vela replay returned `ok: true`, one accepted Claim, three Proposals, three
   Submissions, seven Verification Records, and Repository root
   `sha256:db4d435c2989d43c7ab88fe135865e89a6ba095429315baedb78bcbd9e90ebdc`.
3. The retained source-adapter artifact and the Codeberg clone independently
   built the Observatory candidate twice into two fresh local PostgreSQL
   clusters. Both attempts produced manifest-core root
   `sha256:f8fe4a751950546dd957bf39ea14a8bcb1676dfa6e84906cc383ac54cb00f330`
   and local release root
   `sha256:51423217960cd2646a26ec6d0581484f3701ddde315db19fd33535d4678f40e3`.
4. Every projected table and row root verified before activation. The reader
   role had SELECT on every public projection table and no write privilege.
5. The same provider-independent inputs exported the `erdos-321` Result Dossier
   as `site.result-dossier-export.v1`: 9,890 canonical bytes, export root
   `sha256:90bd3ef52bc98ac9a98e9d635143f0d7af472dc9ee00219e693c19fc9840a7dd`,
   row root
   `sha256:412052ec3b058886f80f4e134db315349f17070c4b2938b4a499681ce06668b8`.
   Its declared authority effect is `none` and it retains five explicit
   nonclaims.
6. The public mirror workflow run
   `https://github.com/vela-science/vela-web/actions/runs/31314271076` also
   verified the Vela and Math Codeberg replicas, restored Math's signed 0.971.0
   predecessor bundle from Codeberg alone, and anonymously read back the pinned
   0.972.1 assets against committed digests.

## Limits

- The same frozen source-adapter artifact was used for both reconstructions.
  This proves retained-input reconstruction, not future reacquisition from
  mutable upstream sources.
- The macOS generator has a different binary digest from the Linux production
  generator, so its release root is intentionally platform-specific. Table
  roots, Repository inputs, and source-registry roots are the cross-platform
  equality boundary.
- This exercise establishes continuity and reconstruction only. It makes no
  scientific, adoption, reviewer-efficiency, or productivity claim.
