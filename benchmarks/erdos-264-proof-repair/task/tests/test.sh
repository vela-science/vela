#!/bin/sh
set -u

mkdir -p /logs/verifier

candidate=/logs/artifacts/264.lean
if test ! -f "$candidate"; then
  printf '{"eligible":1,"exact":0,"reason":"candidate_missing"}\n' > /logs/verifier/verification.json
  printf '{"eligible":1,"exact":0,"authority_safe":1}\n' > /logs/verifier/reward.json
  exit 0
fi

if python3 /tests/verify.py \
    --workspace /workspace/formal-conjectures \
    --candidate "$candidate" \
    --json > /logs/verifier/verification.json 2> /logs/verifier/stderr.txt; then
  printf '{"eligible":1,"exact":1,"authority_safe":1}\n' > /logs/verifier/reward.json
else
  printf '{"eligible":1,"exact":0,"authority_safe":1}\n' > /logs/verifier/reward.json
fi
