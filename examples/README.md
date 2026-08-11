# Vela reference flows

These examples exercise the current Protocol 1 release-candidate boundary. They
are small on purpose and add no protocol object, SDK, hosted service, or
authority path.

- [`formal-math/`](formal-math/) replays the current Math Repository and checks
  the merged Erdős 321 terminal-variant evidence without treating that evidence
  as Verification, Decision, or Standing.
- [`computational-science/`](computational-science/) runs a bounded exact
  computation and emits a signed Submission with the independent Python or
  JavaScript reference producer. It does not accept the Claim.
- [`correction-inheritance/`](correction-inheritance/) verifies the retained
  real Math authority chain, then reproduces the separate synthetic diamond
  cascade with Rust and Python. The distinction is explicit because no real
  accepted dependency cascade exists yet.

For the shortest first experience, start with the [flagship quickstart](../docs/QUICKSTART.md).
