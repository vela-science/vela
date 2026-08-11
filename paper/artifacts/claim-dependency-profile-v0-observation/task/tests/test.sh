#!/bin/sh
set -eu

mkdir -p /logs/verifier
python3 /tests/scorer.py \
  --answer /logs/artifacts/answer.json \
  --answer-key /tests/answer-key.json \
  --input-manifest /tests/input-manifest.json \
  --input-root /tests/input \
  --verification-output /logs/verifier/verification.json \
  --reward-output /logs/verifier/reward.json
