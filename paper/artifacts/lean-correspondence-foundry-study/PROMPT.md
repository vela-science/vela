# Frozen participant prompt

The same text is used in Stage A and Stage B. Bracketed values are replaced by
the prelaunch-frozen assignment manifest; no free-form prompt editing is
allowed after the arm bytes are committed.

## Common prompt

```text
You are reviewing one proposed correspondence between exact Lean source and
target states. Work only in the read-only assignment directory. Network access,
external memory, other assignments, and authority actions are unavailable.

Assignment: [assignment_id]
Family: [opaque_family_id]
Time limit: 1200 seconds

Determine, at the supplied commits and environments:

1. Is the claimed source-target relation valid, invalid, or unprovable from the
   supplied evidence?
2. Does the observed state represent semantic change, environment drift, both,
   neither, or an unprovable state?
3. What is the complete in-scope downstream impact closure? For every allowed
   item id, select exactly one disposition and cite one or more supplied
   evidence ids. Include no item outside the supplied closed scope.
4. What, if anything, is evidenced about Repository-local authority and about
   scientific status? Keep a Lean build, witness, Git commit, signature,
   Verification, packet, foreign Decision, local Decision, Standing, and
   scientific truth distinct.
5. List material uncertainty that prevents a stronger conclusion.

Use local shell/file tools as needed. Do not write to a source repository,
contact a provider or person, create a key, sign, submit, verify, accept, reject,
or change Standing. Return exactly one JSON object conforming to the supplied
response schema. Do not add prose outside the JSON object.
```

## Arm preambles

Exactly one preamble is inserted before the common prompt.

`raw-source`:

```text
The assignment contains the exact source and target repositories, bounded Git
history, environment files, witness source, evidence inventory, and closed
impact-item vocabulary. No correspondence record or derived current-validity,
change, or impact answer is supplied.
```

`correspondence-assisted`:

```text
The assignment contains the same semantic atoms as the raw-source condition,
plus a rooted Lean Correspondence relation record, witness receipt, recheck
state, and explicit impact artifact. Treat each as evidence to verify, not as an
answer key, authority action, scientific acceptance, or guarantee of validity.
```

`structured-unwitnessed` (only after its independent admission gate passes):

```text
The assignment contains the same semantic atoms and closed structure as the
correspondence-assisted condition, but executable witnesses, inheritance edges,
recheck state, and the current-validity/change/impact fields derived from them
are withheld. No answer key or authority action is supplied.
```

## Frozen substitution rules

- `[assignment_id]` is the prederived session assignment id.
- `[opaque_family_id]` reveals no correctness or change label.
- The time limit is always `1200`.
- Paths, allowed impact ids, and evidence ids are supplied in the assignment
  manifest, not interpolated into prose.
- Case names visible in Stage A remain visible because Stage A is open. Stage B
  uses opaque ids until all captures freeze.
- Arm names are retained in custody metadata but not exposed in the participant
  directory or prompt.
- No participant-specific encouragement, repair hint, retry instruction, or
  result-dependent text is permitted.
