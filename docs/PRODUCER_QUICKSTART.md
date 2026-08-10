# Producer quickstart

A producer creates evidence. Vela retains an authenticated Submission and
opens a Proposal. Neither act verifies or accepts the Claim.

## Inspect the repository

```bash
git clone <repository-url>
cd <repository>
vela status . --json
vela replay . --json
```

The source-owning Repository or a read product may separately expose an exact
next obligation, packet, expected outputs, and verifier profile. Validate that
projection under its owning contract.

## Orient bounded work

Vela core owns no Target catalogue and publishes no `next`/`start` command
pair. Work selection and packet briefing stay in the source Repository, read
product, or native workbench. Run the bounded work in the native agent,
workbench, proof assistant, notebook, or laboratory system.
Retain exact repository-relative Artifacts. A failed or negative result is
useful only when its scope, search space, algorithm, and limits are explicit.

## Submit the result

```bash
vela submit --repo . \
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

The result names the immutable Submission and pending Proposal. The
accepted-event delta is zero. Producer-reported
`--check` values remain producer claims and never become Verification Records.

A producer from another workbench may pass a complete signed
`vela.submission.v2` file to the same command:

```bash
vela submit submission.json --repo . --json
```

## Inspect and reproduce

```bash
vela show . <vsb_id> --json
vela review show . <vpr_id> --json
vela reproduce .
```

## Authority boundary

Agents and producers do not run `vela review accept` or
`vela review reject`, access repository-authority credentials, mint a
Verification Record for their own output, or describe Git publication as
scientific acceptance.

`vela init` creates the structural Profile and establishes the repository
boundary in one command. A failed signing attempt remains resumable by rerunning
`vela init`; Submission intake fails closed until it completes. Routine Submission and Verification
intake authenticates the producer or verifier record itself and does not read a
repository-authority key or require the caller's local trust pin. The
independently distributed sequence-one trust root remains required for strict
consumer verification and later authority actions.
