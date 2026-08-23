#!/usr/bin/env bash
# Verification gate for the proof index.
#
# Fails if:
#   1. any tracked source contains a literal `sorry` or `admit`, or
#   2. `Audit.lean` reports `sorryAx` for any headline theorem, or
#   3. any headline theorem depends on an axiom outside the allowed kernel set.
#
# Run locally with the Mathlib cache primed:
#   lake exe cache get && lake build && bash scripts/check_axioms.sh
set -euo pipefail

ALLOWED='[propext, Classical.choice, Quot.sound]'
fail() { echo "FAIL: $*" >&2; exit 1; }

# 1. No literal sorry / admit in the hosted proofs. Each problem's directory
#    also holds its computational provenance and plans, whose prose may use
#    the word; only Lean sources are proofs.
for proof_dir in ErdosProblems lean/Erdos699/Proved; do
  if [ -d "$proof_dir" ] && grep -rnoE --include='*.lean' '\b(sorry|admit)\b' "$proof_dir" ; then
    fail "literal sorry/admit in $proof_dir/"
  fi
done

# 2/3. Build the audit and inspect the axiom report.
report="$(lake env lean Audit.lean 2>&1)"
echo "$report"

echo "$report" | grep -qiE '(^| )error' && fail "Audit.lean did not compile cleanly"
echo "$report" | grep -q 'sorryAx'        && fail "a headline theorem depends on sorryAx"

# Every "depends on axioms:" report must be a subset of the allowed kernel set.
# Lean may wrap the axiom list across several lines for long theorem names, so
# collect continuation lines until the closing bracket appears.
seen=0
while IFS= read -r line; do
  if [[ "$line" == *"depends on axioms:"* ]]; then
    seen=$((seen+1))
    axioms="${line#*depends on axioms: }"
    while [[ "$axioms" != *"]"* ]]; do
      IFS= read -r continuation || fail "unterminated axiom report -> $line"
      axioms+="$continuation"
    done
    compact="$(printf '%s' "$axioms" | tr -d '[][:space:]')"
    if [ -n "$compact" ]; then
      IFS=',' read -r -a axiom_names <<< "$compact"
      for axiom in "${axiom_names[@]}"; do
        case "$axiom" in
          propext|Classical.choice|Quot.sound) ;;
          *) fail "unexpected axiom '$axiom' -> $line" ;;
        esac
      done
    fi
  fi
done <<< "$report"

[ "$seen" -gt 0 ] || fail "no axiom report produced (is Audit.lean wired up?)"
echo "PASS: $seen headline theorem(s) clean, axioms subset of $ALLOWED"
