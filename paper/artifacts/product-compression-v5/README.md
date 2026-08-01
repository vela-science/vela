# Product-compression v5 diagnostic

This run is invalid for the registered comparison. Harbor completed all four
trials without retries, but `git-files-02` exceeded the frozen five-minute
agent timeout by roughly five seconds and Harbor recorded an
`AgentTimeoutError`.

Both Vela-guided trials were exact and both Git/files trials were not exact.
That is useful directional evidence, but the timed-out session prevents product
lift credit.

The raw Harbor job remains local at
`jobs/product-compression-v5/runs/vela-product-compression-v5-native-20260731`
and is bound by the recorded job root. The successor plan changes only the
execution safety timeout to 15 minutes. It does not change the v5 answer
contract, fixture, model, arms, or comparison rule.
