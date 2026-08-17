# Optional review-method checklists

These four canonical `vela.review-method.v1` examples are small starting
points, not universal gates. Copy only the review that matches the scientific
question, replace its example identity and procedure with what actually ran,
retain the canonical file in Git, and bind it through `vela verification
record --method`.

The examples separate four questions that are often collapsed:

- `semantic-source-adequacy.json`: source identity, scope, and statement fit;
- `mathematical-reasoning.json`: the stated mathematical argument;
- `computational-formal.json`: an exact computation or formal checker run;
- `meta-authority-independence.json`: performer attribution, shared
  dependencies, independence, and the Repository authority boundary.

Human, AI-model, organization, and deterministic-tool reviewers use the same
Verification semantics. Their kind supplies provenance, not evidentiary rank.
Weight comes from the exact method, inputs, outputs, scope, limitations, and
independence or shared dependencies. A pass remains non-authoritative; only an
authorized Decision changes Standing.

One ordinary flow is:

```bash
git add -- verification/method.json
git commit -m "Retain exact review method"

vela verification record . <vpr_id> \
  --profile <profile-from-method> \
  --method verification/method.json \
  --outcome pass \
  --does-not-establish "<copy each method nonclaim exactly>" \
  --independent-of <producer-actor-id> \
  --as <attested_by_actor_id> \
  --json
```

Use multiple `--does-not-establish` flags when the method lists multiple
nonclaims. Declare `--independent-of` only when that independence is true;
otherwise record the check as complementary and disclose shared dependencies
in retained output. Separate reviewers produce separate Verification Records.
