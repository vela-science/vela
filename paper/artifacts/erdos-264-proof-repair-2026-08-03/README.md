# Erdős 264 proof-repair study

This is the retained result of one preregistered, matched Harbor study over a
real correction-continuation task. Both arms saw the same exact Formal
Conjectures checkout, predecessor proof, native Lean verifier, model, custody,
and single-attempt rule. The only treatment difference was ordinary Git/file
inspection versus the read-only `vela next` and `vela start` path.

The headline result is **0/1 exact native passes in each arm**. This is a null
result for the preregistered endpoint, not evidence of Vela lift.

The failure modes differ:

- the Git/files arm retained an incomplete Lean-invalid candidate;
- the Vela-guided arm selected the exact current Target and retained a
  source-preserving proof that compiles with unlimited heartbeats, but it still
  times out under the frozen native-default verifier contract.

The Vela-guided arm used less observed agent time and model cost, but a failed
exactness gate cannot be rescued by secondary metrics. With one attempt per arm,
those differences are descriptive only.

Harbor could not issue a reward for either trial. After each model had already
finished and its artifact had been retained, Docker Desktop exhausted its
storage while unpacking the separately built, network-denied Formal
Conjectures verifier image. The artifacts are therefore scientifically
classifiable from their retained native Lean traces, while the Harbor trial
status remains an infrastructure error. The study does not turn either error
into a verifier score.

[`result.json`](result.json) binds the frozen plan, fixture, Harbor job,
trial results, trajectories, artifacts, timings, token counts, model costs,
failure classifications, and claim limits. Raw Harbor outputs remain in the
user-local cache named by the study; they are not copied into Git.

The registered study's exact next obligation was to optimize or restructure
the guided proof until the source-local verifier passed, or to demonstrate a
verifier-contract defect. That work is post-study engineering and may not be
retroactively counted as a benchmark retry. A repair remains evidence until a
separate Verification and an attributed human Decision change Standing.

That post-study diagnosis is now complete. The registered contract required a
native Lean pass but did not declare an elaboration heartbeat budget. The
source-local verifier had therefore inherited Lean's implicit 200,000-heartbeat
enclosing-command limit. Making the contract explicit with
`-DmaxHeartbeats=0` verifies the unchanged guided proof; a trailing-whitespace
normalization was then rechecked byte-for-byte as
`sha256:9ba4b0c8aa144985aac8df40ee070c0ffe4ab7b59915d9b44eb90b42f96935e8`.
This is a separate repair episode and does not rescore the frozen 0/0 study.

The normalized artifact is retained in Erdős Frontier. Submission
`vsb_d0af649f7155e0ed` produced Proposal `vpr_69b5b3e26d39acbe`.
Complementary scoped Verification
`vvr_47f1732ee550cfd7` records the exact proof observation; requirement-satisfying
Verification `vvr_3c05f6340fee38be` makes Decision Inbox entry
`sha256:badec76bae5a1141ce213408f5f1e5d77b1a85102789b2580a14796346321e2d`
protocol-ready with accepted-event delta zero. An attributed human accepted the
exact packet in Frontier commit `ea44055f33ec04509385454228fd6cba8fcfe562`.
Clean-clone replay reproduced repository root `sha256:f53da5…`; a different
context-free producer then recovered `erdos:203:finite-cover`, its exact packet,
verifier, authority ceiling, and first useful action in 98 seconds without
private predecessor context. The agent did not make the Decision.

This closes the action-complete correction and handoff case. It does not rescue
the frozen matched result: exact pass@1 remains `0/1` for both arms, so this
study demonstrates workflow closure but not causal Vela lift.
