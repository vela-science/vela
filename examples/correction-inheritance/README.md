# Correction and inheritance: real Decision, separate cascade proof

This reference flow keeps two facts separate.

First, the retained Math authority fixture is real signed Repository history.
The independent reader verifies its four-record authority chain from a separate
sequence-one trust anchor and reaches one accepted corrected Claim:

```bash
uv run --project conformance --locked \
  python conformance/verify_authority_chain.py
```

Second, the downstream diamond is a frozen synthetic conformance vector. It
exercises hard dependencies, lost and surviving support routes, repair
obligations, bounds, and fail-closed mutations in both implementations:

```bash
cargo test --locked -p vela-edge --test correction_impact
uv run --project conformance --locked \
  python conformance/verify_correction_impact.py
```

Both must derive projection root
`sha256:935e084f…cc01df6`; the input and output roots are frozen in
[`flow.json`](flow.json).

The real Math correction currently has no producer-authored dependency edges,
so its downstream cascade is empty. The diamond proves implementation
agreement, not a real accepted dependency cascade. Vela 1.0 keeps that limit
visible rather than relabeling synthetic conformance as scientific use.
