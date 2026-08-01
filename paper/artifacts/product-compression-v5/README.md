# Product-compression v5 diagnostic

This run is invalid for the registered comparison. Harbor completed all four
trials without retries, but `git-files-02` exceeded the frozen five-minute
agent timeout by roughly five seconds and Harbor recorded an
`AgentTimeoutError`.

Both Vela-guided trials were exact and both Git/files trials were not exact.
That is useful directional evidence, but the timed-out session prevents product
lift credit.

The generated Harbor cache was removed after `diagnostic.v1.json` retained its
job root and bounded invalidity conclusion. The successor plan changes only the
execution safety timeout to 15 minutes. It does not change the v5 answer
contract, fixture, model, arms, or comparison rule.
