# Independent re-review: inherited correction benchmark repair

## Verdict

**PASS**, scoped to the amended benchmark implementation at producer commit
`8336b584a8303026e747ce4a7c7257e3efb1db2c`, tree
`996bbb8417bf807cb0230b0294e1d4d845e6e3b9`.

This re-review resolves the prior commit-bound findings F01 and F02 from review
commit `c377e99e2c982211be7ecb9cba8998c848cf18c2`. It qualifies the immutable
artifact as a deterministic preregistration, information-matched packet builder,
closed scorer, and fixed-denominator capture/custody implementation suitable for
separate consideration of an experimental authorization.

It **does not authorize** the 16-session experiment. It does not approve paid
inference, human participation, scientific validation, credentials, budget,
participant configuration, assignment seed, authority or Standing actions, a
merge, or a positive result. `result.json` remains `not_run` with 0/16 sessions.

The original benchmark base remains
`1a2e0328620b4e8c4584c3d4baf257adb11f3d45`. Live `origin/main` independently
advanced to `2b8d43ed50a9639dfc18c5f6f21677021f70a4b2`; this re-review makes no
merge-compatibility or current-main certification claim.

## Prior findings

### F01 — resolved

Responses and consequence items now have exact closed field sets. Free-text
actions are replaced by four enumerated action codes, and scoring compares the
selected code exactly with the protected Claim-to-action mapping.

Independent reproduction confirmed:

- adding the four prior negated unsafe sentences as extra fields fails with
  `response_consequence_fields_invalid`; and
- selecting a valid but wrong action code makes `action_exact: false` and
  `exact_success: false`.

The prior 17/17 negation exploit no longer exists.

### F02 — resolved

Each run now retains exact authorization bytes. Freeze independently validates
the closed run shape, directory/run identity, preregistration root, actual
packet bytes and condition root, authorization fields/root/assignment,
participant configuration, attempt, timeout, authorization time, timestamps,
finite nonnegative duration, duration/status consistency, nonnegative integer
tool count, and closed response. One authorization root and participant
configuration must cover the complete 16-run denominator. Scored results bind
the capture and adjudication roots.

Independent reproduction constructed a valid 16-run capture, then confirmed
fail-closed rejection of:

- forged registration, packet, authorization, and configuration roots;
- packet-byte drift and unauthorized participant assignment;
- attempt `99` and timeout drift;
- negative and NaN duration;
- timestamp/duration mismatch;
- negative and boolean tool counts; and
- the unfilled, not-authorized run template.

The valid control capture and score still pass and bind their exact capture and
adjudication roots. The prior forged positive-gate exploit no longer exists.

## Amended registration and immutable bytes

The prospective amendment retains the previous registration root, previous
producer commit, prior independent BLOCKED review, reason for repair, and zero
observed sessions/results. Independent recomputation matched:

- producer commit `8336b584a8303026e747ce4a7c7257e3efb1db2c`;
- producer tree `996bbb8417bf807cb0230b0294e1d4d845e6e3b9`;
- current registration root
  `sha256:7391c3c6adb74633886fd9fb2d35a257e7501bd37153acfb3e19ac850d0e9157`;
- preregistration bytes
  `sha256:f700be9e487e60cdda17c25d35b5c271ffe225b1fd9202741cf844ad12fc0784`;
- amendment bytes
  `sha256:c130706e3242abaf36e735f46dacc5171b1326d05ffad597c0f3e5eb450c4e21`;
- amendment canonical root
  `sha256:e8fc90aad7b0d7d0f1aad120e87d7f972c0f537f1ffdc6ca9ce58fe67c4fa1e8`;
- artifact manifest bytes
  `sha256:463e4cf7622bba617d273b3cdf559311778a3266b5666d3568e1c8dd2831ad62`;
- implementation bytes
  `sha256:8ae77954fe43e98d0e884e0cac79c9d16639fe0699453bb9ba0e8285c7980c1c`;
- test bytes
  `sha256:976d55e62713f21feb048576c1ca24b995ea13ad15ef097362cdd0933396069e`;
- adjudication root
  `sha256:6b2e94c7bfce7c41353eb48cd4962243e3f177fdaccb8c7da48567d99dfca557`;
- response-template root
  `sha256:78d4d59eedf87b612be8c7845546baebd2a9c85d56aa92f7084c255550c4972b`;
- authorization-template root
  `sha256:2c0600b3f4c09b75544e6293a692c96ddf06d485faab40c1703481ada5d8c440`;
- input-equivalence root
  `sha256:649cec9ed991172ac4303a78e76e893750a250c2392e93e0a45e8cc44440e014`;
- Git/documents packet root
  `sha256:bdda8e39a17e50607a4587993dc7fe855fae9408dad2dd0ae11dc47ee281cb6e`;
- Vela packet root
  `sha256:2bc904703cfd47419846e0a9771c5e9c3933dba5465ec9f48440d1850ace4c97`;
- unchanged public-facts root
  `sha256:fe8b3363ec9a8305743ca55144a59885a73623b712a32fe0c9050227350bac2a`;
  and
- unchanged replay-chain root
  `sha256:ae39c3c4ff623deb5be261fad654afa6ac19c44d364b18e5aad5899b8b9c0d52`.

Every manifest entry matched. The same six source/evidence files remain
byte-identical in the fixture and both arms, and protected classification and
Claim-to-action mappings remain absent from both packets.

## Deterministic checks

The exact producer commit was fetched into a new clean clone. Each nominal
check was run twice and passed both times:

- benchmark `verify`;
- 15 benchmark tests;
- Ruff check and format check;
- Python correction-impact verification at
  `sha256:935e084f8c5c45bcee234d2e9752062ba54493aa1b14f731e0efbbb1ecc01df6`;
- 5 Rust correction-impact reducer tests;
- full Protocol 1 conformance at
  `sha256:e014259269ea34452bb5a583a29ee478bec53e67128ec9eafa6d099a883fc24c`;
  and
- `git diff --check` from the original base through the repair commit.

The repair diff touches only 16 paths under
`paper/artifacts/inherited-correction-benchmark/`. No producer byte was edited
during review, and no paid model, human study, scientific validation, Decision,
authority action, Standing mutation, or merge was performed.

## Remaining external gate

Before any session, a separate decision must supply and review the exact
authorization, participant/model configuration, assignments and seed
commitment, budget and credential custody, and validation protocol. This PASS
establishes benchmark-package readiness only; it is not that decision.
