# Producer quickstart

A producer creates evidence. Vela retains an authenticated Submission and
opens a Proposal. Neither act verifies or accepts the Claim.

## Inspect the Frontier

```bash
git clone <frontier-url>
cd <frontier>
vela status . --json
vela check . --json
vela next . --limit 1 --json
```

Read the returned Offer. It binds the Target, packet, expected outputs,
verifier profile, and exact next command.

## Read the exact Target briefing

```bash
vela start <target> --frontier . --json
```

This optional command revalidates the Target and packet, then prints a
write-free briefing and direct Submission template. Run the bounded work in
the native agent, workbench, proof assistant, notebook, or laboratory system.
Retain exact frontier-relative Artifacts. A failed or negative result is useful
only when its scope, search space, algorithm, and limits are explicit.

## Submit the result

```bash
vela submit --frontier . \
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
configured. Submission intake fails closed until `vela authority init`
has established the repository boundary. Routine Submission and Verification
intake authenticates the producer or verifier record itself and does not read a
repository-authority key or require the caller's local trust pin. The
independently distributed sequence-one trust root remains required for strict
consumer verification and later authority actions.
