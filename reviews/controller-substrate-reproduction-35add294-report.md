# Independent review: controller/substrate reproduction

## Verdict

**PASS**, strictly for producer commit
`35add294b5400f3a8cdc1ec08d680992fee8f137`, tree
`05b3dad910fbf0e61e0d23ed178c95dfc2dbc79d`, over base
`4685462c44b1f073870f31025ae73d1d8770ce73`.

This verdict qualifies the exact paper-only reproduction artifact. It does not
authorize a merge, release, inference run, scientific Decision, authority use,
Standing change, deployment, or publication.

## Review method

I cloned `vela-science/vela` into a fresh isolated directory, fetched the exact
producer ref, detached at the reviewed commit, and confirmed the handed-off
commit, tree, ancestry, and six-path diff. I read the current repository
instructions before inspecting the artifact.

I then performed both the producer's documented offline reproduction and a
separate reconstruction:

1. recomputed every manifest-bound file size and SHA-256;
2. independently canonicalized the sorted file entries and recomputed the
   artifact root;
3. verified, unbundled, shallow-boundary restored, and ran `git fsck --full`
   on both Git bundles;
4. inspected every retained commit object, parent, tree, Repository manifest,
   Target index, authority record, and Decision Event from the reconstructed
   repositories;
5. cross-checked the retained map artifacts and exact materializer source;
6. ran the verifier, all five focused tests, locked Ruff check/format, and
   `git diff --check` from the fresh producer checkout.

## Exact identities

- Manifest bytes: `sha256:d3a53fe7954957c1159b06bca72d972c492e45c4c4bbac8b9e09c5c6d3efd5bc`.
- Artifact root: `sha256:39cd088440a6a4175f7a7299419adc13a705c9ca54db8c1692594dbfdae488a9`.
- Erdős bundle: `sha256:8bfbfd434643dd0135460aa47846cbbfb2a73ed23a330753ace4ed3d2744274d`.
- Vela Web bundle: `sha256:9c7d6ec608235ac5510ef921313cbb28065479aef8c7e1b3dcd50c303493d518`.
- Deterministic verifier output: `sha256:5b092942bf064b1771c456dc030d81df4138a2976351bb07046b4ba5d088474a`.

## Findings

### Bundle and history custody: PASS

The Erdős bundle reconstructs exactly four commits:

1. `43c7a1418ccd16c304a3c9c0e62ba0ead26d06ab`, tree
   `bf2efb827bcaafe45feef1b26d95896467833972`;
2. `85b7a35e4c83fc552da5598f5f7998bda2026c8a`, tree
   `d501de4fa3c284ce2eebc156d816007f3400d45a`;
3. `606f2f4b50193b1feccf1df4e1f31d50d3a8dd99`, tree
   `79c190d8f6fe899185494b52541b5df6d7677d41`;
4. `80606bdccb51fa86524111a1a61876bb08e45d79`, tree
   `fd233bf5ef27196dab3eaef5d4c664ed56f8ed92`.

The Vela Web bundle reconstructs exactly
`834598bf3c38117e97789805417ace797eb3e62c` / tree
`539097ebf757ffdbd593792f586addbb49c5952b`, followed by
`6a4ae82442d396b053a1fbb8d804d1349e0e5747` / tree
`fc9c1a791ba889939f67c47cd3d863c38f0a552f`. Object parents match the
manifest even at each shallow boundary.

### Standing and authority separation: PASS

The exact canonical `.vela/repository.json` bytes reproduce these stages:

| Stage | Repository root | Accepted | Pending | Bounded Claim |
| --- | --- | ---: | ---: | --- |
| pre-run | `sha256:8a98ff1c632232c7b227d87a0f1015aaa3429d38c83592ca66f8e465b06b0ee5` | 2771 | 0 | absent |
| Submission | `sha256:da38399a15d9deae5a012d9df9f8eae745bf8f3c44e22da973720a8e256cb1c0` | 2771 | 1 | pending review |
| Verification | `sha256:8b1c2bbc99b9e9aade2bfb56d3493be02cdad954eefa3cd98a14ac41128ae0d4` | 2771 | 1 | pending review |
| Decision | `sha256:9679827bc76de9f6433bfafa8e2e966b9780ca1273c7948d97c2ae042f5cab1a` | 2772 | 0 | accepted |

The Submission authority record
`sha256:1ad63ab73427529461f4d07a3aaa1dd59c54512a330a90f7355d4d6efdadd994`
and Verification authority record
`sha256:8af8d3a2d6f61337278020379d557293841ce0458ef218adc3a89292213bd4a1`
both bind agent principals, empty semantic approvals, empty event lists, and an
unchanged event-log root. Neither stage changes accepted Standing.

The later authority record
`sha256:47a9bc4342624a0605063b12814322b5d4421c221ac3d7ae8e9095a73ada2e50`
binds a human principal, one `review_accept`, and exactly two events:

- review event `sha256:fb48df2660288285a8dd838e94e1969cef6da95a13a9f7b483641c7f54d1006e`;
- applied finding event `sha256:d0d8c70b2a35a87591fe3f29712ab878698d24266df895f7294f7d723c09c6d9`.

Both events bind the same human principal and exact Repository transition from
the Verification root to the Decision root. The one accepted delta therefore
occurs only after the separate attributed Decision.

### Replay, remap, and stale Target: PASS

The first Target packet remains exactly
`sha256:6d1a2ca87851deb1fa2133f4f6cf7edb28ee843cb0eef57ea09e826b3fdca63b`
at all four retained stages. The Target-index roots change across the trace,
including from pre-run
`sha256:2e77609a3ce670abd1bf653a19c40d585f8f87d54ef6f21724b7b99470571372`
to post-Decision
`sha256:e5a0c40cfcc6817215ae2fd81b2f4ae64cf1dcab74c1cb4d861baac11788aa8d`.
The frozen post-Decision record therefore correctly retains a stale packet as
a product defect rather than presenting it as fresh work.

The frozen pre-run, post-Verification, projection, key-free Decision packet,
and post-Decision remap bytes agree with the reconstructed Git histories. The
post-Verification candidate is not activated; the Decision packet is
`prepared_not_invoked`; and the post-Decision record reports exactly one
accepted delta.

### Controller and materializer boundary: PASS

The Submission provenance is `source_system: canopus`, producer
`agent:canopus-local`. Its agent authority record contains no semantic approval
and no Event. The scoped verifier also has no semantic authority. Manual and
AST inspection of the exact materializer finds only these Vela surfaces:
`--version`, `status`, `review show`, and `next`, including clean-clone reads.
It reads no authority-agent socket and contains no Git push. Its projection
invocation is the frozen dry-run path. The artifact itself performs only
offline reads and temporary reconstruction.

## Claim ceiling

The exact bytes support one bounded internal first-party systems trace:
controller activity, scoped Verification, and a read-only map leave accepted
Standing unchanged; a later human Decision admits one bounded Claim; replay and
remap expose that delta and the stale Target packet.

They do not establish the bounded Claim's scientific correctness beyond the
Decision's recorded scope, original producer/verifier organizational
independence, controller quality, general productivity, adoption, external
reproduction, or a general causal effect of Vela. The review performed no new
authority, Decision, Standing, source, deployment, inference, publication, or
merge action.
