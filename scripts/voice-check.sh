#!/usr/bin/env bash
#
# voice-check.sh — enforce Vela's design voice on every markdown doc.
#
# Exits non-zero if any doc contains banned hype words, emoji, or title-case
# h3 headings. Run automatically from scripts/release-check.sh.
#
# The canon this enforces lives in docs/BRAND.md.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO_ROOT"

fail=0
report() {
  echo "  · voice-check: $1"
  fail=1
}

# Which docs to check. Covers the public-facing markdown surface.
# The Astro site (`site/`) renders these directly, so anything here
# becomes user-visible.
MD_PATHS=(
  "README.md"
  "CHANGELOG.md"
  "CONTRIBUTING.md"
  "SECURITY.md"
  "AGENTS.md"
  "docs"
  "demo"
  "essays"
  "examples"
)

md_files() {
  for p in "${MD_PATHS[@]}"; do
    if [[ -f $p ]]; then echo "$p"; fi
    if [[ -d $p ]]; then find "$p" -type f -name '*.md' -print; fi
  done
}

# Ban-list — pure hype words. `seamless` is allowed in strict technical use
# (type-theoretic contexts) because MATH.md uses it for sheaf-gluing language.
BAN_RX='\b(unlock|supercharge|AI-powered|revolutionize|blazing|next-generation|game-changing|cutting-edge)\b'

# Title-case heuristic: two or more consecutive title-case words after the
# first in an h2/h3/h4. Single-word headings pass. Proper nouns + acronyms
# embedded in otherwise sentence-case headings pass.
H3_TITLECASE_RX='^##+ [A-Za-z]+ [A-Z][a-z]+( [A-Z][a-z]+)+( |$)'

# Emoji range (BMP + supplementary pictographs).
EMOJI_RX=$'[\U0001F300-\U0001FAFF\U00002600-\U000027BF]'

while read -r file; do
  # BRAND.md is the ban-list canon itself — exempt from the ban word scan.
  if [[ $file != "docs/BRAND.md" ]]; then
    # Use Python to strip inline-code spans (single, double, triple backticks)
    # before testing for ban-list words. Lets PRODUCT.md / AGENTS.md / DESIGN.md
    # cite the ban-list as documentation without triggering itself.
    if ! python3 -c "
import sys, re
ban = re.compile(r'\b(unlock|supercharge|AI-powered|revolutionize|blazing|next-generation|game-changing|cutting-edge)\b')
in_fence = False
hits = []
for i, line in enumerate(open(sys.argv[1], encoding='utf-8'), 1):
    stripped = line.rstrip('\n')
    if stripped.lstrip().startswith('\`\`\`'):
        in_fence = not in_fence
        continue
    if in_fence:
        continue
    # strip inline-code spans
    cleaned = re.sub(r'\`[^\`]*\`', '', stripped)
    if ban.search(cleaned):
        hits.append(f'{sys.argv[1]}:{i}: {stripped}')
if hits:
    for h in hits: print(h)
    sys.exit(1)
" "$file" >/tmp/voice_check_ban_$$.out 2>/dev/null; then
      report "banned hype word in $file:"
      while IFS= read -r line; do echo "      $line"; done </tmp/voice_check_ban_$$.out
    fi
    rm -f /tmp/voice_check_ban_$$.out
  fi

  if grep -nE "$H3_TITLECASE_RX" "$file" >/dev/null 2>&1; then
    hits=$(grep -nE "$H3_TITLECASE_RX" "$file")
    report "title-case heading in $file (use sentence case):"
    while IFS= read -r line; do echo "      $line"; done <<<"$hits"
  fi

  if python3 -c "
import sys, re
s = open(sys.argv[1], encoding='utf-8').read()
# Tally / arrow / editorial dingbats are allowed: ✓ ✗ ✔ ✘ ★ → ↗ ↘ etc.
ALLOW = {'✓','✗','✔','✘','★','☆',
         '→','←','↑','↓','↗','↘','↙','↖',
         '—','–','…'}
pat = re.compile(r'[\U0001F300-\U0001FAFF\U00002600-\U000027BF]')
hits = []
for i, line in enumerate(s.splitlines(), 1):
    found = [c for c in line if pat.match(c) and c not in ALLOW]
    if found:
        hits.append((i, line))
if hits:
    for n, l in hits: print(f'{sys.argv[1]}:{n}: {l}')
    sys.exit(1)
" "$file" >/tmp/voice_check_emoji_$$.out 2>/dev/null; then
    :
  else
    report "emoji in $file:"
    while IFS= read -r line; do echo "      $line"; done </tmp/voice_check_emoji_$$.out
  fi
  rm -f /tmp/voice_check_emoji_$$.out
done < <(md_files)

if [[ $fail -ne 0 ]]; then
  echo ""
  echo "  voice-check failed. see docs/BRAND.md for the canon."
  exit 1
fi

echo "  · voice-check: ok"
