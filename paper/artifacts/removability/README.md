# Removability and hosted-service-loss check

This experiment asks whether canonical Erdős replay and proposal inspection
still work when optional producers, websites, read databases, hosted APIs, the
original agent session, and repository-authority credentials are absent.

The plan is frozen before execution in `plan.v1.json`. The test runs the pinned
Vela binary against a fresh exact clone with an empty home directory and
network access denied. It is first-party removability evidence for benchmark
families B5 and B6 only.
