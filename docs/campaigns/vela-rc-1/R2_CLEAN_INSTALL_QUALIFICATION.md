# VELA-RC-1 R2 clean-install and replay qualification

Recorded: 2026-08-26, America/Toronto.

## Verdict

`PASS WITH DOCUMENTED PLATFORM LIMITATIONS` for the R2 installation and byte-
replay gate.

This is not a campaign release verdict. R1 separately reached
`HOLD — SEMANTIC BLOCKER`: shipped replay/read accepts a Repository without
enforcing an independently installed sequence-one authority root. R2 observed
the same behavior in a pristine consumer container. The valid bytes replayed
exactly, but the unpinned success is too permissive under Protocol 1 and blocks
the RC independently of R2 reproducibility. R2 does not fix or narrow that
semantic issue.

## Binding and scope

| Field | Exact value |
| --- | --- |
| Parent control commit | `6d680eebb4a17813e72b55685aa2eec6b34e5fae` |
| Parent control tree | `9273cdaea323859fcd26beebeccc3f7b7fb1acfe` |
| Parent branch | `campaign/vela-rc-1-supervisor` |
| R2 branch | `campaign/vela-rc-1-r2-clean-install` |
| Vela version | `0.977.4` |
| Protocol | Vela Protocol 1 release candidate |
| Public release | signed `v0.977.4` |
| Fixture domain | neutral finite arithmetic |
| Lean | not selected; it adds no evidence for this fixture |

The exact control commit was not advertised by `git ls-remote origin` on any
RC-1 branch when tested. That is consistent with the campaign's no-push rule,
but means an exact candidate clone could not honestly be attributed to public
hosting. R2 exported the complete control ref as a Git bundle, mounted it read-
only, and cloned it into a new container filesystem. No supervisor working tree,
target directory, campaign-generated Repository, Cargo cache, SSH agent, trust
pin, or authority key entered the operational container.

The supervisor later reported that the full locked Core union and
`cargo clippy --locked --workspace --all-targets -- -D warnings` passed at the
same control commit, recorded separately at `339db00cb93001440f1768e4e1d56d6dd0b2dc98`.
That is inherited regression evidence, not an R2 clean-install result.

## Exact environments

### Host and isolation

```text
host: macOS 27.0 (26A5378n), Darwin 27.0.0, arm64
Docker client: 29.2.1, darwin/arm64
Docker Desktop: 4.43.2 (199162)
Docker engine: 28.3.2, linux/arm64
guest platform request: linux/amd64
```

The clean guests ran under x86-64 emulation on Apple silicon. No Docker cache
or volume was mounted for Cargo, Git, Vela state, `/root`, or SSH. Image layers
and downloaded source archives were transport inputs, not writable runtime
caches shared between guests.

### Signed-release consumer

```text
image: debian:bookworm-slim
image digest: sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171
guest: Debian GNU/Linux 12 (bookworm), x86_64
Git: 2.39.5
OpenSSH client: 9.2p1
jq: 1.6
installed Vela: 0.977.4
installed binary SHA-256: f73e2378770406efdce486a7a8003170fd55b60a6c69bfb137d429f5d9703c64
initial /etc/machine-id: absent
```

### Exact-candidate source build

```text
image: rust:1.97.1-bookworm
image digest: sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
guest: Debian GNU/Linux 12 (bookworm), x86_64
rustc: 1.97.1 (8bab26f4f 2026-07-14), LLVM 22.1.6
cargo: 1.97.1 (c980f4866 2026-06-30)
Git: 2.39.5
container-local machine ID: 0123456789abcdef0123456789abcdef
source commit: 6d680eebb4a17813e72b55685aa2eec6b34e5fae
source tree: 9273cdaea323859fcd26beebeccc3f7b7fb1acfe
source-built binary SHA-256: 8625ecd174cc9bc79de3cebbd476305cecd6fd2fb0cad31019d4895d7bab7bf8
```

The source-built digest identifies only that build. It is not the accepted
signed release digest, and no cross-path or cross-rustc byte reproducibility is
claimed.

## Commands exercised

### Signed installation and hosted read path

The first clean consumer installed only the tools named or directly required
by the release-facing commands:

```bash
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  ca-certificates curl git openssh-client jq

curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.977.4/install.sh | \
  VELA_VERSION=v0.977.4 VELA_REQUIRE_SIGNED_MANIFEST=1 bash

git clone https://github.com/vela-science/math.git /tmp/math
git -C /tmp/math checkout 5de716c896065c03c0a470d015ba2a328a527f73
vela status /tmp/math --json
vela replay /tmp/math --json
```

Exact salient output:

```text
Verified by: signed release manifest (provider-independent)
vela 0.977.4
binary sha256: f73e2378770406efdce486a7a8003170fd55b60a6c69bfb137d429f5d9703c64
Math commit: 5de716c896065c03c0a470d015ba2a328a527f73
Math tree: 56e37a5058c80e69f3c343b8ae624c08b5417229
Math repository root: sha256:a956b84c437202e5a02cc9e036a621bd14a302b34a75758115730bdbb77c52a4
accepted Claims: 3
strict: pass
```

### Exact-candidate source installation

The complete control ref was exported from Git, verified as a complete bundle,
mounted read-only, and cloned into the clean source guest:

```bash
git clone -b campaign/vela-rc-1-r2-clean-install \
  /input/vela.bundle /work/vela
cd /work/vela
test "$(git rev-parse HEAD)" = \
  6d680eebb4a17813e72b55685aa2eec6b34e5fae
cargo build --release
./target/release/vela --version
sha256sum ./target/release/vela
```

This follows the source path published in `README.md`. The clone began with no
`target/`, Cargo registry, authority state, trust state, or generated campaign
Repository.

### Operator loop

The disposable container used one standard OpenSSH agent and one dedicated
ephemeral Ed25519 key. The key and agent never left the guest. The Review
Method and arithmetic bytes were tracked and clean before Verification.

```bash
printf '%s\n' 0123456789abcdef0123456789abcdef > /etc/machine-id
eval "$(ssh-agent -s)"
ssh-keygen -q -t ed25519 -a 64 -N '' \
  -f /work/authority/vela-authority \
  -C 'Vela neutral replay fixture authority'
ssh-add -t 8h /work/authority/vela-authority

/work/vela/target/release/vela init /work/operator/repository \
  --name 'Neutral arithmetic fixture' \
  --scope 'Does the retained finite arithmetic calculation match its stated result?' \
  --check --json
/work/vela/target/release/vela init /work/operator/repository \
  --name 'Neutral arithmetic fixture' \
  --scope 'Does the retained finite arithmetic calculation match its stated result?' \
  --json

git -C /work/operator/repository config user.name 'Vela Fixture'
git -C /work/operator/repository config user.email fixture@vela.invalid
git -C /work/operator/repository add -- \
  evidence/arithmetic-mean.txt verification/method.json
git -C /work/operator/repository commit \
  -m 'Retain neutral calculation and review method'

vela submit --repo . \
  --claim 'The arithmetic mean of the retained values 1, 2, 3, and 4 is 2.5.' \
  --type computational --replayability exact \
  --artifact evidence/arithmetic-mean.txt:calculation \
  --caveat 'This establishes only the stated arithmetic over the retained four values.' \
  --requires-verification computational_or_formal_check \
  --as agent:fixture-producer --json

vela verification check verification/method.json \
  --profile computational-formal-verification-v1 \
  --property computational_or_formal_check \
  --as verifier:exact-checker \
  --does-not-establish \
    'Statement faithfulness, empirical applicability, novelty, significance, scientific acceptance, or Standing.' \
  --json

vela verification record . vpr_af87deb2a3f1fc1c \
  --profile computational-formal-verification-v1 \
  --method verification/method.json \
  --property computational_or_formal_check --outcome pass \
  --does-not-establish \
    'Statement faithfulness, empirical applicability, novelty, significance, scientific acceptance, or Standing.' \
  --independent-of agent:fixture-producer \
  --as verifier:exact-checker --json

vela replay . --json
vela review inbox . --json
vela review accept . vpr_af87deb2a3f1fc1c \
  --reason 'The retained arithmetic and exact scoped check support this bounded result.' \
  --if-entry-root \
    sha256:58863f0398a1b9d18d151da2fcb900948cfbc363fc0afbdfa2f07bba6e58ec3f \
  --as human:fixture-reviewer --json
vela status . --json
vela replay . --json
vela claims . --json
vela why . \
  vcl_24df07004f63ce0c92a4fe12b06a08d0b777714642f4e9d613a92d8b3bdbb94b \
  --json
vela log . --json
```

## Exact operator outputs

| Boundary | Exact result |
| --- | --- |
| Genesis | Repository `ced420cb-454a-42fb-b7d2-d62422c794b7`; origin `sha256:58bd9e59e8b6fdb8382c450c8acc95d988621f067341754949aa340bd84452b5` |
| Sequence one | `sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753` |
| Submission | `vsb_bd6d2a308e91645f`; Proposal `vpr_af87deb2a3f1fc1c`; accepted Event delta `0` |
| Claim | `vcl_24df07004f63ce0c92a4fe12b06a08d0b777714642f4e9d613a92d8b3bdbb94b`; root `sha256:08681bd2703e40313a741ec43db197c68c0f50783ab4c956978eb19184dcab5c` |
| Verification | `vvr_b1037c94d49c849d`; `pass`; accepted Event delta `0` |
| Before Decision | Repository root `sha256:1ccbed8858de6d049dab94c0a0898979f7c1482f90aa83ae47104bfb7a3d012f`; accepted `0`; pending `1` |
| Decision | authority record `sha256:8227d323019e13339afebc56d900d951a74dd319e9ca61d95fa48ccff1ee96c8`; `scientific_state_changed: true` |
| Events | `review.accepted` `vev_4ecce2945b20bd8c`; `claim.asserted` `vev_f34be749c818c098` |
| Terminal Git | commit `0bd019a846902c8e3e7802d6150063b475f144dc`; tree `0983f52ac18e11897225087cf7aa919d459823cd` |
| Terminal replay | Repository root `sha256:6e7c2d797352a70b9d102f79baa9f3431631aa6ca240233f3dcd37d13f938e6a`; accepted `1`; pending `0`; strict `pass` |
| Fixture Standing commitment | `sha256:87e6791ebd481d977a0789b71f5fe523a1fe2799fb1015eb852f1f57da79ace1` over canonical `accepted_claims`; informative test commitment, not a Protocol root |

The frozen public artifact is
`examples/neutral-replay/neutral-replay.git.bundle`, SHA-256
`7edc8297e79c995864b7a3e02bb046fdb47aed11a767a003c14395f5eaf4131c`.
It contains complete `valid` and `corrupt-artifact` histories.

## Clean-clone replay and integrity failure

A second pristine Debian guest installed the signed release again. It received
only the read-only fixture bundle. No source checkout, authority key, agent,
machine ID, trust directory, or other first-guest state was copied.

```bash
git clone -b valid /fixture/neutral-replay.git.bundle /work/valid
vela replay /work/valid --json
vela status /work/valid --json

git clone -b corrupt-artifact \
  /fixture/neutral-replay.git.bundle /work/corrupt
vela replay /work/corrupt --json
```

The valid branch reproduced the exact terminal commit, tree, Repository root,
counts, and fixture Standing commitment. It also replayed before any independent
sequence-one pin was installed. That unpinned success is the independently
observed semantic blocker recorded above; the checked-in public instructions
therefore include the exact `authority trust pin` command even though this
shipped binary does not enforce it on read.

The corrupt branch is an ordinary clean Git commit
`1712f8189c66d49415ab3ab54a8ae96e605e505c` that removes required Artifact
`records/artifacts/sha256/39feb3b6928d9d1ccf52fb14ad584c45d515cc3800f011388e7ca77c3dc6e1cb`.
Replay exited `1` and returned exactly:

```json
{
  "schema": "vela.error.v1",
  "ok": false,
  "command": "replay",
  "error": {
    "kind": "domain",
    "code": null,
    "message": "read object records/artifacts/sha256/39feb3b6928d9d1ccf52fb14ad584c45d515cc3800f011388e7ca77c3dc6e1cb: No such file or directory (os error 2)",
    "hint": null
  }
}
```

No partial Standing was emitted.

## Elapsed setup and command friction

All times are guest wall time under x86-64 emulation and are descriptive, not
performance claims.

| Step | Elapsed |
| --- | ---: |
| Debian prerequisite installation, first read guest | 29.771 s |
| Signed Vela installation | 2.587 s |
| Hosted Math clone | 1.184 s |
| Hosted Math status | 9.336 s |
| Hosted Math replay | 5.875 s |
| Source-guest jq installation | 3.935 s |
| Complete control-bundle clone | 1.828 s |
| Clean release source build | 48.477 s |
| `init --check` | 0.035 s |
| `init` | 3.524 s |
| `submit` | 10.743 s |
| `verification check` | 0.037 s |
| `verification record` | 19.956 s |
| replay before Decision | 2.664 s |
| Decision Inbox | 1.478 s |
| authorized accept | 12.995 s |
| terminal status | 3.634 s |
| terminal replay | 2.874 s |
| terminal claims | 2.446 s |
| terminal why | 1.646 s |
| terminal log | 1.635 s |
| second clean bundle clone | 0.132 s |
| second clean replay | 2.587 s |
| corrupt bundle clone | 0.137 s |

## Failures and prerequisites

### Expected product failure

- The missing required Artifact failed closed with exit `1` and the exact
  `vela.error.v1` above. This is passing integrity evidence.

### Release blocker observed independently

- A clean consumer with no local trust pin replayed the valid history. Protocol
  1 requires an independently selected sequence-one root. This is reproducible
  but semantically invalid and agrees with R1's accepted HOLD.

### Undocumented or under-documented prerequisites

- The write journey needs a usable Git author identity. Without one, user and
  Vela commits may remain unpublished locally; the quickstart shows `git
  commit` but does not explicitly call out `user.name` and `user.email` setup.
- The source-build sentence assumes a system linker/build toolchain and network
  access for the pinned Rust components and locked crates unless already
  retained locally. The official Rust image supplied the compiler/linker, then
  `rust-toolchain.toml` triggered a channel sync and downloaded the pinned
  `clippy` component during the first `cargo build --release`.
- The signed installer command assumes `curl`, archive extraction, a SHA-256
  utility, `ssh-keygen`, and a writable install prefix or `sudo`. The installer
  diagnoses the security-critical verifier, but the quickstart has no package
  prerequisite table.
- `jq` is not a Vela runtime prerequisite. It was R2's JSON assertion tool and
  is explicitly listed for the checked-in fixture verifier.

The container-local 32-hex `/etc/machine-id`, standard OpenSSH agent/key,
tracked clean Review Method, and independently supplied authority root are
already documented. R2 provisioned them explicitly rather than relying on host
state.

### Harness failures retained, not product failures

- The first Debian-slim attempt used `/usr/bin/time`, which the image did not
  contain. Bash's built-in timer replaced it.
- `bash -lc` reset the official Rust image's `PATH`, hiding
  `/usr/local/cargo/bin`; `bash -c` preserved the image-declared path.
- `git bundle verify` was first invoked outside a Git repository and rejected
  the harness command. Subsequent verification ran inside a new empty Git
  repository, and clean `git clone` independently validated the bundle.

No Vela install, init, submit, Verification, Decision, status, claims, why,
log, or valid replay command failed in the corrected clean path.

## Limitations

- Linux x86-64 was tested in a clean Debian guest under emulation. The macOS
  Apple-silicon signed archive was not retested in a disposable macOS VM.
- The exact RC control commit was local-only by campaign policy. Public hosting
  of prospective RC source or bytes was neither available nor authorized.
- R2 compared exact output roots across clean clones; it did not run an
  independent compiler implementation or claim cross-host binary equality.
- The neutral arithmetic fixture exercises Vela's lifecycle, not Lean or a
  domain-native scientific checker.

Subject to those platform limitations, clean signed installation, exact source
build, the complete operator loop, clean-clone replay, and required-Artifact
fail-closed behavior passed. The separate unpinned-read semantic blocker keeps
the overall release campaign on HOLD.
