# Cold-reader operator guide

Use this script unchanged for every participant. Do not explain either fixture,
identify relevant files, or interpret an answer during a task.

## Orientation script

“This study compares two ways of reading a scientific record. You are not being
asked to judge the science or make a decision. You will inspect two different
records, one through GitHub and one through the Vela Observatory. For each task,
answer only from the assigned surface. Do not use a search engine, AI assistant,
CLI, paper, memo, private context, or the other surface. It is valid to report
that a requested record or next action does not exist.”

Read this glossary verbatim:

- **Claim:** a bounded scientific assertion recorded for review.
- **Verification:** scoped evidence about whether a Claim or its artifacts pass
  stated checks. Verification is not acceptance.
- **Decision:** an attributed authorized accept, reject, or cancel action.
- **Standing:** the current replayed state produced by authorized Decisions.
- **Frontier:** one named repository with its own history and authority.
- **Observatory:** a read-only projection of exact Frontier records; it is not
  the authority or writer.

Ask the participant to confirm the restrictions and start the assigned task.

## Task procedure

1. Open the assigned URL and question sheet without revealing another route.
2. Start one monotonic task clock when both become visible.
3. Record elapsed seconds when the participant first supplies:
   - decisive evidence recognized by the answer key;
   - the correct Decision and local Standing;
   - the exact next valid action or explicit absence of an offer.
4. Do not confirm correctness, redirect navigation, or answer questions about
   the record. Repeat only the glossary or task restrictions verbatim.
5. Stop the clock when the participant submits the final response. At 720
   seconds, stop the task and retain the incomplete response without coaching.
6. After the two-minute reset, repeat with the assigned second fixture.

Unmet milestones and total time at timeout are recorded as 720 seconds with
`censored=true`. Timeouts and wrong answers remain eligible outcomes.

## Scoring and custody

Store only the de-identified participant code and the covariates frozen in the
plan. Give each scorer an answer copy without interface labels or timings.
Two scorers work independently, then record disagreements and adjudication.
Do not alter the plan, answer key, glossary, timing rules, or scoring formula
after participant one views evidence.
