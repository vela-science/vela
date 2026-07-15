# Vela

Git-native, authority-scoped state for scientific work. Evidence enters as
content-addressed receipts, verification is derived from explicit checks, and
accepted state changes only through a human key or a policy that key already
authorized. This repository is the open protocol and reference implementation.

Sixty seconds, no trust required:

```sh
cargo build --release
./target/release/vela reproduce examples/sidon-a309370
```

That command re-verifies every stored witness from scratch with the declared
exact verifier. Given the same tracked bytes and a supported execution
environment, the verifier deterministically returns the same scoped result. It
does not establish scientific truth, indefinite artifact availability, or
compatibility with every future machine. No model or reputation score decides
the verifier outcome.

Vela compiles research artifacts (papers, notes, runs, proofs) into a versioned
*frontier*: a content-addressed, replayable record of what a named authority has
accepted for one bounded scope at an exact root. Other authorities may disagree
or fork. The unit Vela tracks is the *change* to that scoped state, not the
document that triggered it.

Two things are separate here, and the separation is the point:

- **Log integrity and replay are mechanically checkable.** Authority-bearing
  changes are signed over content-addressed bytes, and conforming readers given
  the same valid log derive the same declared state. This is necessary, and it
  is not sufficient.
- **A claim only becomes *verified* by passing the gate.** Not by a proposer's
  say-so, not by an LLM judge, not by a single confirming run. The gate wants at
  least two independent matched verifier attachments — by different method and
  solver, each bound to the exact claim — plus one surviving adversarial probe.
  With zero attachments a claim sits at `needs_verification`, even after a
  reviewer accepts it.

The gate is what kept the Erdős dogfooding from banking 47 "verified" records
that carried an empty verification field. Nine Sidon-set records that did pass
it were later approved into [OEIS A309370](https://oeis.org/A309370) by an
outside editor. That is external publication of the witness data, not
independent adoption, interoperability, or validation of Vela itself.

This repository contains the open Vela protocol, reference reducer, CLI, hub,
and conformance suite. It is dual-licensed under Apache-2.0 OR MIT.

## The verification gate

A claim is `draft` by default and reaches `verified` only through
`verifier_attachment::derive_gate_status`, a pure function of the attachments
with no setter. The status cannot drift out of sync with the evidence because it
is never stored, only derived — the discipline `status_provenance` already
applies to Belnap polarity. Four conditions, each tied to a real failure it
would have caught:

- **G1 independence** — ≥2 matched attachments by *different* method/solver,
  mutually declaring `independent_of`. One self-confirmed run never suffices.
- **G2 claim-match** — every passing attachment is bound to the current claim
  digest. A proof of a *different* statement is `passed_but_unmatched` and counts
  for nothing.
- **G3 adversarial** — at least one probe present and none refuted. A refuted
  probe drives the status to `refuted`.
- **G4 well-formed** — attachments are structurally valid and content-addressed
  (`vva_…`).

Alongside it, `deliverable_grade` blocks solve-language ("resolves #647", "first
to solve") in a claim's text unless the grade is an actual solve. A bound
improvement may not call itself a resolution.

```sh
vela gate vocab                      # the grade / method / probe vocabularies
vela gate grade --claim "..." --grade improved_published_bound
vela gate check --claim "..." --attachments attachments.json
vela reproduce examples/sidon-a309370   # re-verify every stored witness from scratch
```

Verification status is orthogonal to the human review verdict and to Bayesian
confidence. A finding can be reviewer-accepted and still gate
`needs_verification`; that gap is information the substrate used to hide.

## What is here

| Path | What it is |
|------|------------|
| `crates/vela-protocol` | The reference reducer — the normative state-transition function. |
| `crates/vela-cli` | The `vela` command-line tool. |
| `crates/vela-verify` | Frozen, independent exact verifiers (Sidon, Golomb, cap, B_h, covering, constant-weight, Costas, linear codes, and the Erdős certificate kinds: interval-product #1056, CRT partial cover #203, Kummer no-carry #684, min-binom-gcd #700, binomial deficiency #1093, exception enumeration #1094) — the reference verifier set behind the gate and `vela reproduce`. |
| `crates/vela-hub` | The hub: a read-only index over strictly verified Git history. Operators select source repositories in a versioned catalog; the Hub accepts no frontier-state or source-registration writes. |
| `clients/python` | A repository-local independent replay implementation used by conformance; it is not an alternate write API or distribution. |
| `conformance/` | The cross-implementation test-vector suite (reducer fixtures + gate reject-vectors). |
| `examples/sidon-a309370` | Current verifier example: nine OEIS A309370 witness files you can re-check with `vela reproduce`; not an accepted-state fixture. |
| `examples/erdos-formalization` | Historical signed-event replay fixture; retained for immutable-byte compatibility, not as a current authoring template. |
| `frontiers/` | Read-only discovery catalogs derived from standalone frontier state; these directories are not themselves Vela frontiers. |
| `lean/` | Machine-checked proofs of the governance-soundness theorems, plus `SidonCertificate.lean` (a kernel-checked vcert). |
| `schema/` | Current portable packet and finding schemas. |

## Build

```sh
cargo build --release
./target/release/vela --help
vela completions zsh > ~/.zfunc/_vela   # shell completions (bash/zsh/fish)
```

Or install a prebuilt binary on Apple Silicon macOS or Linux x86_64. Other
platforms must build from source:

```sh
curl -fsSL https://raw.githubusercontent.com/vela-science/vela/v0.800.11/install.sh \
  | VELA_VERSION=v0.800.11 bash
```

## The working loop

The full porcelain reference is [docs/CLI.md](docs/CLI.md). If you are
submitting one bounded result to a frontier you do not maintain, start with
the [producer quickstart](docs/PRODUCER_QUICKSTART.md).

```sh
vela next <frontier> --json
vela work <target> --frontier <frontier> --as agent:<you> --json
vela land --frontier <frontier> --work <target> --claim <result> \
  --type computational --replayability exact --artifact <path>:<kind> \
  --caveat <limit> --as agent:<you> --json
vela reproduce <frontier>
vela sign                       # key-holding human only
```

An agent may land; only a key-holding human signs. Failed and negative work can
be retained as scoped receipts instead of disappearing from the next briefing.

The Rust reducer is the reference implementation; the repository-local Python
reader tracks its declared subset against the conformance vectors in
`conformance/`.

## Contribute to a live frontier

Producers do not need a human key. They claim a prepared target and land a
receipt under an `agent:` identity:

```sh
vela next . --json
vela work <target> --frontier . --as agent:<your-handle> --json
# Run the verifier named by the work briefing, then:
vela land --frontier . --work <target> --claim <result> \
  --type computational --replayability exact --artifact <path>:<kind> \
  --caveat <limit> --as agent:<your-handle> --json
```

The frontier's signed policy routes the receipt. Permit can admit a narrowly
pre-authorized class; Defer leaves the proposal for a key-holding human. A
producer cannot sign, accept, reject, or finalize it. See the
[producer quickstart](docs/PRODUCER_QUICKSTART.md) for the exact workflow,
result classes, Git publication check, and offline path.

## Project links

- Repository: https://github.com/vela-science/vela
- Releases: https://github.com/vela-science/vela/releases
- Protocol: [docs/PROTOCOL.md](docs/PROTOCOL.md)

## License

Dual-licensed under Apache-2.0 OR MIT, at your option.
