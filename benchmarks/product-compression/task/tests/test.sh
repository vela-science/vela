#!/bin/sh
set -u

if ! python3 /tests/verify.py > /logs/verifier/test-stdout.txt 2> /logs/verifier/test-stderr.txt; then
  printf '{"eligible":0,"exact":0}\n' > /logs/verifier/reward.json
fi
