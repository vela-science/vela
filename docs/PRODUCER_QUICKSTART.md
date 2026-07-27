# Producer quickstart

A producer creates evidence. Vela registers an authenticated Submission and
opens a Proposal. Neither act verifies or accepts the Claim.

## Inspect the Frontier

```bash
git clone <frontier-url>
cd <frontier>
vela status . --json
vela check . --strict --json
vela next . --limit 1 --json
```

Read the returned Offer. It binds the Target, packet, expected outputs,
verifier profile, lease state, and exact next command.

## Start one Attempt

```bash
vela start <target> --frontier . --as agent:<name> --json
```

Run only the bounded work and checks named by the Attempt. Retain exact
frontier-relative Artifacts. A failed or negative result is useful only when
its scope, search space, algorithm, and limits are explicit.

## Submit the result

```bash
vela submit --frontier . \
  --attempt <vat_id> \
  --claim "<bounded result>" \
  --type computational \
  --condition "<scope condition>" \
  --replayability exact \
  --artifact <path>:<kind> \
  --caveat "<what this does not establish>" \
  --requires-verification "<independent check required>" \
  --as agent:<name> \
  --json
```

The result names the immutable Submission, Vela-issued Registration Record,
and pending Proposal. The accepted-event delta is zero. Producer-reported
`--check` values remain producer claims and never become Verification Records.

A producer from another workbench may pass a complete signed
`vela.submission.v1` file to the same command:

```bash
vela submit submission.json --frontier . --json
```

## Inspect and reproduce

```bash
vela show . <vsb_id> --json
vela show . <vrr_id> --json
vela review show . <vpr_id> --json
vela reproduce .
```

## Authority boundary

Agents and producers do not run `vela review accept` or
`vela review reject`, access repository-authority credentials, mint a
Verification Record for their own output, or describe Git publication as
scientific acceptance.

Fresh `vela init` repositories are structural and report authority as not
configured. Submission registration fails closed until `vela authority init`
has established the repository writer and the consumer has installed its
independently distributed sequence-one trust root.
