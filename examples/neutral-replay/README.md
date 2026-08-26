# Neutral clean-clone replay fixture

This informative fixture is one minimal, non-domain-specific Repository
history. It retains a four-value arithmetic calculation and demonstrates the
current operator chain:

```text
genesis -> Submission -> scoped Verification -> attributed Decision
        -> admitted Events -> accepted Standing -> strict replay
```

It adds no protocol object or authority path. The calculation is only a small
public replay target; it is not evidence of scientific utility, external
adoption, or a Protocol 1.0 release.

## Prerequisites

The replay path needs only:

- Vela `0.977.4`, installed from the signed release or built from an exact
  source checkout;
- a complete Git client; and
- a supported platform: Linux x86-64 or macOS Apple silicon.

The optional [`check.sh`](check.sh) assertions also need `jq` and either
`sha256sum` or `shasum`. They need no authority key, SSH agent, machine ID,
pre-existing local trust state, network service, campaign checkout, or private
setup. The script installs the exact public sequence-one trust pin and removes
it afterward only when the script created it.

Install the pinned signed release on a supported host:

```bash
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.977.4/install.sh | \
  VELA_VERSION=v0.977.4 VELA_REQUIRE_SIGNED_MANIFEST=1 bash
```

## Replay the valid history

From a Vela source checkout:

```bash
git clone -b valid \
  examples/neutral-replay/neutral-replay.git.bundle neutral-replay-valid

vela authority trust pin neutral-replay-valid \
  --record-root sha256:317226ded44506c4010ebe073889d816eabd522b8f0870a83d02e01f93cc3753 \
  --json
vela replay neutral-replay-valid --json
vela status neutral-replay-valid --json
vela why neutral-replay-valid \
  vcl_24df07004f63ce0c92a4fe12b06a08d0b777714642f4e9d613a92d8b3bdbb94b \
  --json
vela log neutral-replay-valid --json
```

The clean clone must reproduce:

- Git commit `0bd019a846902c8e3e7802d6150063b475f144dc`;
- Git tree `0983f52ac18e11897225087cf7aa919d459823cd`;
- Repository root
  `sha256:6e7c2d797352a70b9d102f79baa9f3431631aa6ca240233f3dcd37d13f938e6a`;
- one authenticated Submission, one passing scoped Verification Record, one
  accepted Claim, and no pending Claim;
- the accepted Claim at root
  `sha256:08681bd2703e40313a741ec43db197c68c0f50783ab4c956978eb19184dcab5c`;
  and
- `review.accepted` and `claim.asserted` Events admitted by the attributed
  Decision.

The sequence-one root is fixture metadata obtained independently of the cloned
Repository history. Pinning it changes only OS-account-local public trust
configuration; it grants no authority and changes no Repository byte.

Vela intentionally publishes no standalone protocol `standing_root`.
[`expected.json`](expected.json) therefore names a fixture-local regression
commitment over the RFC 8785 canonical `accepted_claims` slice:

```text
sha256:87e6791ebd481d977a0789b71f5fe523a1fe2799fb1015eb852f1f57da79ace1
```

That value proves this frozen fixture's accepted Standing did not drift. It is
not a Repository root or a new public protocol root.

## Exercise the integrity failure

The same bundle has a `corrupt-artifact` branch whose final ordinary Git commit
removes the required content-addressed Artifact while leaving the reference in
the Repository manifest:

```bash
git clone -b corrupt-artifact \
  examples/neutral-replay/neutral-replay.git.bundle neutral-replay-corrupt

vela replay neutral-replay-corrupt --json
echo $?
```

The command exits `1`, returns `vela.error.v1`, and reports the exact missing
Artifact path. It does not return a partial Standing or silently drop the
evidence.

Run every frozen assertion with the installed binary:

```bash
examples/neutral-replay/check.sh
```

Or bind an exact source-built binary by absolute path:

```bash
VELA_BIN="$PWD/target/release/vela" examples/neutral-replay/check.sh
```

[`flow.json`](flow.json) records the original public CLI commands that created
the valid history. [`expected.json`](expected.json) binds the bundle, branches,
roots, identifiers, Standing commitment, and fail-closed error.
