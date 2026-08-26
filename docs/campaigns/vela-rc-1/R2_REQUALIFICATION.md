# VELA-RC-1 R2 clean-install and replay requalification

Recorded: 2026-08-26, America/Toronto.

## Verdict

`PASS WITH DOCUMENTED PLATFORM LIMITATIONS`

The exact repaired candidate passed a clean source build and source install,
the complete governed operator loop, the independently pinned public Math read,
the frozen neutral clean-clone replay, and the required authority-pin and
Artifact fail-closed matrix. The limitation is platform coverage: clean Linux
x86-64 was exercised under emulation on an Apple-silicon host; a disposable
macOS Apple-silicon guest was not available and was not tested.

This is only the R2 clean-install and replay verdict. It does not accept the
candidate for release, adjudicate R1 or R3-R7, identify a released binary,
authorize a version change, or authorize a tag, push, publication, signing, or
release.

## Binding and starting state

| Field | Exact value |
| --- | --- |
| Starting freeze commit | `6750eb79fbe83ab106ad575357ea0f1775b38146` |
| Starting freeze tree | `94f0fc73d98918cdddc9021d7df9d4b5c23e4d46` |
| Repaired product commit | `ad2a4516078525025d05bd461b550ed5b8e35971` |
| Repaired product tree | `e08112922efbe59ef3b042d0a8f6b0f9557761ea` |
| Freeze parent | repaired product commit above |
| Vela version reported | `0.977.4` |
| Starting checkout | detached `HEAD`, clean |

`git diff ad2a451..6750eb79` changed only the four append-only campaign
records `DECISIONS.md`, `FAILURES.md`, `QUALIFICATION.md`, and `STATE.md`.
No product, protocol, fixture, build, or release-facing instruction byte
differs between the repaired candidate and the freeze commit.

The freeze ref was exported as a complete Git bundle solely to transport exact
Git objects into the isolated build guest. The bundle SHA-256 was
`dea5db2fa37d30d0a6a3e5728ec4a27e93d1833a8b7f7d9a86eefb624966a7fb`.
The guest checked out the repaired commit detached and asserted both its commit
and tree before compiling. The candidate checkout was clean before and after
the build and install.

## Isolation and exact environments

### Host and Docker boundary

```text
host: macOS 27.0 (26A5378n), Darwin 27.0.0, arm64
Docker client: 29.2.1, darwin/arm64
Docker engine: 28.3.2, linux/arm64
Docker storage: overlayfs
guest platform request: linux/amd64
guest kernel: Linux 6.10.14-linuxkit, x86_64
```

Every guest used a new writable container layer. No host Cargo registry,
`target/`, Git configuration, home directory, Vela trust directory, SSH agent,
authority key, machine ID, campaign Repository, or generated scientific state
was mounted. The build guest received only the read-only freeze bundle. The
main consumer received the installed candidate binary and release-facing
example inputs read-only. The final independent fixture guest received only
the candidate binary and `examples/neutral-replay/` read-only.

### Build guest

```text
image: rust:1.97.1-bookworm
image digest: sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
guest: Debian GNU/Linux 12 (bookworm), x86_64
rustc: 1.97.1 (8bab26f4f 2026-07-14), LLVM 22.1.6
cargo: 1.97.1 (c980f4866 2026-06-30)
Git: 2.39.5
initial Cargo registry: absent
initial Vela trust state: absent
```

### Consumer and independent replay guests

```text
image: debian:bookworm-slim
image digest: sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171
guest: Debian GNU/Linux 12 (bookworm), x86_64
Git: 2.39.5
OpenSSH client in main consumer: 9.2p1
jq: 1.6
initial machine ID: absent
initial Vela trust state: absent
initial SSH state: absent
initial work state: empty
```

The final replay-only guest did not install `openssh-client`, had no machine
ID or `SSH_AUTH_SOCK`, and had no campaign checkout or campaign state. Its
release-facing fixture check passed and removed the trust pin it created.

## Exact build and install

The build guest ran:

```bash
git clone /input/vela-freeze.bundle /work/vela
git -C /work/vela checkout --detach \
  ad2a4516078525025d05bd461b550ed5b8e35971
test "$(git -C /work/vela rev-parse HEAD)" = \
  ad2a4516078525025d05bd461b550ed5b8e35971
test "$(git -C /work/vela rev-parse 'HEAD^{tree}')" = \
  e08112922efbe59ef3b042d0a8f6b0f9557761ea

cd /work/vela
cargo build --locked --release
./target/release/vela --version
CARGO_TARGET_DIR=/work/vela/target \
  cargo install --locked --path crates/vela-cli --root /opt/vela-candidate
cmp ./target/release/vela /opt/vela-candidate/bin/vela
```

Exact output:

```text
vela 0.977.4
sha256:a9cdc6a0cd3d26b3b9c3b37d55c317dafdb5938f017880eabd6de627fca37e9d
installed_matches_built=yes
source_worktree_clean=yes
```

The installed binary was copied, without rebuilding, into `/usr/local/bin/vela`
in the new consumer. Its digest remained exactly
`a9cdc6a0cd3d26b3b9c3b37d55c317dafdb5938f017880eabd6de627fca37e9d`.
That digest identifies only this exact source build. It is not claimed to be a
signed-release digest or byte-reproducible across build paths, hosts, or Rust
toolchains.

The published `install.sh` was not used because it installs the already signed
`v0.977.4` release rather than this unreleased repaired commit. Using it would
have tested different product bytes. The release-facing exact-source path was
used instead: locked release build, Cargo source install, and installation of
those exact bytes into a pristine consumer.

## Public Math read with independent trust

The clean consumer followed the public `README.md` identity and trust values:

```bash
git clone https://github.com/vela-science/math.git /work/math
git -C /work/math checkout --detach \
  5de716c896065c03c0a470d015ba2a328a527f73

vela status /work/math --json

vela authority trust pin /work/math \
  --record-root sha256:efae3e02b5be6dfccf6701ebe26f87f00bb64f5b4372674e572a633844d95469 \
  --json
vela status /work/math --json
vela replay /work/math --json
```

The first, deliberately unpinned `status` exited `1`, returned
`vela.error.v1`, emitted no Standing, and directed the consumer to install an
independently qualified sequence-one root. After the documented pin was
installed, the read reproduced:

| Field | Exact result |
| --- | --- |
| Math commit | `5de716c896065c03c0a470d015ba2a328a527f73` |
| Math tree | `56e37a5058c80e69f3c343b8ae624c08b5417229` |
| Repository root | `sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4` |
| Strict integrity | `pass`; zero blockers |
| Counts | 3 accepted Claims, 0 pending, 6 Submissions, 6 Verifications, 10 Artifacts |

The Math checkout remained clean. Pinning changed only OS-account-local public
trust configuration and reported `authority_granted: false` and no Repository
writes.

## Governed operator loop

The main clean consumer created one stable container-local machine ID, started
one standard OpenSSH agent, and generated one disposable Ed25519 authority key.
The key, agent, and machine ID never left the guest. The tracked Review Method
was the release-facing `examples/review-methods/computational-formal.json`, root
`sha256:736ca630b8e3cb3154ffbeca833b64b543c2aa2b51355403e0c2534fc7708fae`.

The exercised command sequence was:

```bash
vela init /work/operator/repository \
  --name "R2 neutral arithmetic requalification" \
  --scope "Does the retained finite arithmetic calculation match its stated result?" \
  --check --json
vela init /work/operator/repository \
  --name "R2 neutral arithmetic requalification" \
  --scope "Does the retained finite arithmetic calculation match its stated result?" \
  --json

git -C /work/operator/repository add -- \
  evidence/arithmetic-mean.txt verification/method.json
git -C /work/operator/repository commit \
  -m "Retain R2 arithmetic evidence and review method"

vela submit --repo /work/operator/repository \
  --claim "The arithmetic mean of the retained values 1, 2, 3, and 4 is 2.5." \
  --type computational --replayability exact \
  --artifact evidence/arithmetic-mean.txt:calculation \
  --caveat "This establishes only the stated arithmetic over the retained four values." \
  --requires-verification computational_or_formal_check \
  --as agent:fixture-producer --json

vela verification check \
  /work/operator/repository/verification/method.json \
  --profile computational-formal-verification-v1 \
  --property computational_or_formal_check \
  --as verifier:exact-checker \
  --does-not-establish \
    "Statement faithfulness, empirical applicability, novelty, significance, scientific acceptance, or Standing." \
  --json

vela verification record /work/operator/repository \
  vpr_9fdc493bce2ea514 \
  --profile computational-formal-verification-v1 \
  --method verification/method.json \
  --property computational_or_formal_check --outcome pass \
  --does-not-establish \
    "Statement faithfulness, empirical applicability, novelty, significance, scientific acceptance, or Standing." \
  --independent-of agent:fixture-producer \
  --as verifier:exact-checker --json

vela replay /work/operator/repository --json
vela review inbox /work/operator/repository --json
vela review accept /work/operator/repository \
  vpr_9fdc493bce2ea514 \
  --reason "The retained arithmetic and exact scoped check support this bounded result." \
  --if-entry-root \
    sha256:8c7704f256090b2b24aaa807daee7378bd9ed8eeeeebc4a5d109e9a92c104a42 \
  --as human:fixture-reviewer --json

vela status /work/operator/repository --json
vela replay /work/operator/repository --json
vela claims /work/operator/repository --json
vela why /work/operator/repository \
  vcl_441ec7c52a27c33d0832134f43f02c9e020206c5490509985a797005e0555232 \
  --json
vela log /work/operator/repository --json
git -C /work/operator/repository fsck --strict
```

Exact salient results:

| Boundary | Exact result |
| --- | --- |
| Repository | `52cac794-0a1a-42ce-b261-40d5c4e7dae8` |
| Sequence-one authority record | `sha256:0401e34c44706d18cacfac1c6d647aa8a7b412681ba59abb7e7f11500e0bbad7` |
| Origin | `sha256:7071a722620bbb0cdfdf51c49c92edd1a39893bc651f53c7f3d8efb689e82057` |
| Submission | `vsb_800151c4cbae0bf3`; accepted Event delta `0` |
| Proposal | `vpr_9fdc493bce2ea514`; accepted Event delta `0` |
| Claim | `vcl_441ec7c52a27c33d0832134f43f02c9e020206c5490509985a797005e0555232` |
| Verification | `vvr_eaac3cd4f77e12cd`, root `sha256:eaac3cd4f77e12cd7f5d89167050a8ea414f9911abf534430fd340ce5b809849`, outcome `pass`; accepted Event delta `0` |
| Before Decision | root `sha256:791c8af7547d17b855320300815dc186c72e2262ccc7e9fa289d0796003a7e0b`; 0 accepted, 1 pending |
| Decision | authority record `sha256:1fbd3a0ede1814f4595eae1b199e675c347b17c3b9b11cdcd74cb9e21ee0b288`; `scientific_state_changed: true` |
| Decision Events | `vev_1dd7430027157160`, `vev_98bffbdcbd8d04f4` |
| Terminal Repository root | `sha256:c704450d5b38957b4937dbb899ef817647d618bb748feca99e3f500f20ff297e` |
| Terminal Git | commit `3b09d328edb9e820aab51a09aa2b27e99e26a0fa`; tree `1af7fd7df3bb805a977c47ab0c61641fb8886236` |
| Terminal Standing | 1 accepted Claim, 0 pending; strict `pass`, zero blockers |

`git fsck --strict` passed, the Repository working tree was clean, and `why`
kept Submission, Verification, Decision, and Standing meanings separate.

## Frozen neutral replay and exact digest

The candidate's release-facing bundle reproduced its checked-in identity:

```text
bundle: examples/neutral-replay/neutral-replay.git.bundle
SHA-256: 7edc8297e79c995864b7a3e02bb046fdb47aed11a767a003c14395f5eaf4131c
valid: 0bd019a846902c8e3e7802d6150063b475f144dc
corrupt-artifact: 1712f8189c66d49415ab3ab54a8ae96e605e505c
```

After installing the independently documented sequence-one root
`sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753`,
the clean clone reproduced:

| Field | Exact result |
| --- | --- |
| Valid Git commit | `0bd019a846902c8e3e7802d6150063b475f144dc` |
| Valid Git tree | `0983f52ac18e11897225087cf7aa919d459823cd` |
| Repository root | `sha256:6e7c2d797352a70b9d102f79baa9f3431631aa6ca240233f3dcd37d13f938e6a` |
| Counts | 1 Submission, 1 Verification, 1 accepted Claim, 0 pending |
| Strict integrity | `pass`; zero blockers |
| Fixture-local accepted-set commitment | `sha256:87e6791ebd481d977a0789b71f5fe523a1fe2799fb1015eb852f1f57da79ace1` |

The final pristine replay-only guest then ran exactly:

```bash
VELA_BIN=/usr/local/bin/vela /input/neutral-replay/check.sh
```

It returned `neutral replay fixture: ok`, removed the trust pin it had created,
and still had no `/etc/machine-id` or SSH agent. This confirms that replay needs
no authority key, agent, machine identity, campaign checkout, campaign-local
state, campaign script, or network service once Vela, Git, `jq`, the public
fixture, and a SHA-256 utility are present.

## Fail-closed matrix

Every negative replay exited `1`, returned `vela.error.v1` with `ok: false`,
and emitted no `repository_root` or partial Standing.

| Case | Exact result |
| --- | --- |
| Missing authority pin | `trusted repository read requires an independent sequence-one pin ...` |
| Malformed authority pin | refused parsing `{malformed` and directed restoration of valid `vela.authority-trust-anchor.v1` evidence |
| Mismatched authority pin | refused installed all-zero root because it did not equal the verified sequence-one authority record |
| Missing Artifact | `read object records/artifacts/sha256/39feb3b6928d9d1ccf52fb14ad584c45d515cc3800f011388e7ca77c3dc6e1cb: No such file or directory (os error 2)` |
| Corrupt Artifact bytes | `object records/artifacts/sha256/39feb3b6928d9d1ccf52fb14ad584c45d515cc3800f011388e7ca77c3dc6e1cb does not match its declared root` |

The missing-Artifact result came from the frozen public `corrupt-artifact`
branch. The corrupt-byte result came from a disposable clone of `valid` in
which only the retained Artifact bytes were changed and committed as ordinary
Git state. Its audit-only commit/tree were
`76db1a3a978c771708d584ffb3489ea4b87c2c53` /
`743dbc6c25f2d7a3bcad3bd3ff1dceb1f8373444`. Restoring the exact valid pin
immediately restored the frozen valid replay root.

The authority-pin diagnostics were actionable but carried `error.code: null`.
That is an ergonomic/stability observation for later documentation or product
review, not a failure of the required fail-closed behavior.

## Timing

Times are guest wall time under Linux x86-64 emulation and are descriptive,
not performance claims.

| Step | Elapsed |
| --- | ---: |
| Build guest `apt-get update` | 3.222 s |
| Build guest prerequisites | 37.301 s |
| Exact source bundle clone | 1.732 s |
| Locked release build | 55.620 s |
| Cargo source install using built target | 0.822 s |
| Main consumer `apt-get update` | 2.814 s |
| Main consumer prerequisites | 31.982 s |
| Public Math clone | 3.467 s |
| Public Math unpinned refusal | 4.826 s |
| Public Math pin | 5.782 s |
| Public Math status | 9.879 s |
| Public Math replay | 6.174 s |
| `init --check` | 0.045 s |
| `init` | 3.875 s |
| `submit` | 10.764 s |
| `verification check` | 0.042 s |
| `verification record` | 20.630 s |
| Pre-Decision replay | 2.807 s |
| Decision Inbox | 1.524 s |
| Authorized accept | 13.222 s |
| Terminal status | 3.740 s |
| Terminal replay | 2.924 s |
| Terminal claims | 2.464 s |
| Terminal why | 1.736 s |
| Terminal log | 1.704 s |
| Frozen valid clone | 0.166 s |
| Frozen valid replay | 2.849 s |
| Missing-pin refusal | 2.530 s |
| Malformed-pin refusal | 2.451 s |
| Mismatched-pin refusal | 2.532 s |
| Missing-Artifact replay refusal | 0.058 s |
| Corrupt-byte replay refusal | 0.057 s |
| Independent replay-guest prerequisites | 34.182 s |
| Independent release-facing `check.sh` | 10.650 s |

## Cache, network, and prerequisite assumptions

- Docker reported both pinned image digests as already present. Image-layer
  caching was therefore used for immutable base-image transport, but no
  writable runtime cache or volume was shared with any guest.
- The build guest began without a Cargo registry. `cargo build --locked`
  reached `static.rust-lang.org` to sync the pinned toolchain and fetch the
  declared `clippy` component, and reached crates.io to download locked crates.
- Debian package setup required network access to `deb.debian.org` and
  `deb.debian.org/debian-security`. The public Math read required GitHub network
  access. The frozen neutral replay itself required no network after its named
  prerequisites were installed.
- Building from source assumes the pinned Rust toolchain, a system linker and C
  build environment, Git, CA roots, and network access for missing toolchain
  components and locked crates. The official Rust image supplied these.
- Governed writes require a stable local platform identity. Linux containers
  need an explicitly provisioned container-local 32-hex `/etc/machine-id`.
  They also need the standard OpenSSH agent and one suitable Ed25519 key.
- The evidence/Method preparation commit required a local Git author identity.
  The quickstart shows a Git commit but still does not explicitly name
  `user.name` and `user.email`; R2 configured both locally.
- `jq` and a SHA-256 utility were audit/assertion prerequisites, not Vela runtime
  prerequisites. The final replay-only guest confirmed no OpenSSH client,
  machine ID, or agent is needed for the public read-only fixture.

## Harness observations

Two audit-harness mistakes occurred and are retained here; neither invoked a
failing Vela product path.

1. An attempted `docker exec` targeted the successfully completed, stopped
   build container. Docker refused before starting any operator command. The
   operator loop was then run in the intended new pristine consumer.
2. After a successful Math replay, an assertion queried
   `.integrity.strict` from replay JSON. That field belongs to status JSON.
   The harness exited after the successful replay output; the corrected
   assertions checked replay `ok`, root, and counts plus status strictness and
   blockers, and passed.

No install, init, submit, Verification, Decision, governed status, valid
replay, public Math read after pinning, frozen fixture assertion, Git integrity
check, or required fail-closed product behavior failed.

## Limitations and disposition

- Clean Linux x86-64 was tested under Docker emulation on Apple silicon.
  Disposable macOS Apple-silicon installation and replay remain untested.
- The exact candidate is intentionally unpublished. No signed candidate archive
  or hosted exact-candidate install existed, and creating one was outside this
  audit's authority. Source build/install therefore qualifies the exact commit;
  it does not qualify later release packaging or signatures.
- This audit used the Rust implementation and exact public fixture. It did not
  test a second compiler implementation or claim cross-host binary equality.
- The arithmetic fixture tests lifecycle and replay integrity. It is not a
  scientific-utility, external-adoption, performance, or Protocol 1.0 result.

Subject to those platform and unpublished-candidate limitations, the repaired
candidate is independently requalified for the R2 clean-install and replay
gate as `PASS WITH DOCUMENTED PLATFORM LIMITATIONS`.
