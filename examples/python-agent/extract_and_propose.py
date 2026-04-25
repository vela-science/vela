#!/usr/bin/env python3
"""Phase T (v0.5): canonical paper → claim → frontier example.

Reads a paper text file, extracts candidate claims via the Anthropic API
(Claude Sonnet 4.6 by default), and proposes them as `finding.note`
proposals against a running `vela serve --http` instance.

This is the canonical "how do I use Vela from Python" example. The
target audience is anyone building AI-driven scientific extraction
agents on top of an MCP-aware substrate.

Prerequisites:

    pip install requests cryptography anthropic

    # Frontier set up:
    vela actor add frontier.json reviewer:my-agent --pubkey <hex>
    vela serve frontier.json --http 3848

    # Environment:
    export ANTHROPIC_API_KEY=...
    export VELA_AGENT_KEY=/path/to/private.key

Usage:

    python extract_and_propose.py --paper alz.txt --frontier-url http://localhost:3848

Without --paper, runs against a small canned snippet so the loop is
exercisable without an actual paper folder.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

# Allow running from a repo checkout without `pip install -e`.
SDK_PATH = Path(__file__).resolve().parents[2] / "bindings" / "python"
sys.path.insert(0, str(SDK_PATH))

from vela import Actor, Frontier  # noqa: E402

CANNED_PAPER_SNIPPET = """\
Title: Monovalent Brain Shuttle module engineered into an anti-Aβ antibody
Abstract: A monovalent binding mode to the transferrin receptor (TfR)
increases β-Amyloid target engagement in an Alzheimer's disease mouse
model by 55-fold compared to the parent antibody.
"""

EXTRACTION_PROMPT = """\
You are an extraction agent. Read the following paper snippet and
identify up to 3 atomic scientific claims, each suitable as a Vela
`finding.note` annotation. Return JSON only, in the shape:

{"claims": [
  {"text": "<one-sentence claim>", "scope": "<species/system>", "reason": "<why this is a distinct claim>"},
  ...
]}

Rules:
- Each claim must be a single sentence.
- Use exact wording from the paper where possible; do not paraphrase
  away the conditions (species, model, intervention).
- If the snippet has fewer than 3 distinct claims, return fewer.
- Output JSON only — no preamble, no explanation.

Paper snippet:
"""


def extract_claims_with_claude(paper_text: str) -> list[dict]:
    """Call Anthropic Sonnet 4.6 to extract claims. Falls back to a
    deterministic stub if no API key is set, so the example is
    runnable end-to-end in CI."""
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        # Deterministic fallback for CI / no-API-key runs.
        return [
            {
                "text": "A monovalent Brain Shuttle module increases β-Amyloid target engagement 55-fold in a mouse Alzheimer's model.",
                "scope": "Mus musculus, anti-Aβ antibody + TfR Brain Shuttle",
                "reason": "Quantitative target-engagement effect in animal model.",
            }
        ]
    try:
        import anthropic
    except ImportError:
        sys.stderr.write(
            "anthropic SDK not installed; install with `pip install anthropic`\n"
        )
        sys.exit(1)
    client = anthropic.Anthropic()
    msg = client.messages.create(
        model="claude-sonnet-4-6",
        max_tokens=2048,
        messages=[
            {
                "role": "user",
                "content": EXTRACTION_PROMPT + paper_text,
            }
        ],
    )
    raw = msg.content[0].text.strip()
    # Strip code-fence wrapping if present.
    if raw.startswith("```"):
        raw = "\n".join(raw.split("\n")[1:-1])
    data = json.loads(raw)
    return data.get("claims", [])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "--frontier-url",
        default="http://localhost:3848",
        help="URL of vela serve --http",
    )
    parser.add_argument(
        "--paper",
        type=Path,
        default=None,
        help="Path to a paper text file; falls back to a canned snippet",
    )
    parser.add_argument(
        "--actor",
        default=os.environ.get("VELA_AGENT_ID", "reviewer:agent-test"),
        help="Stable actor id registered via `vela actor add`",
    )
    parser.add_argument(
        "--key",
        default=os.environ.get("VELA_AGENT_KEY"),
        help="Path to actor's Ed25519 private key (hex)",
    )
    args = parser.parse_args()

    if not args.key:
        sys.stderr.write(
            "Provide --key <path> or set VELA_AGENT_KEY environment variable.\n"
        )
        return 1
    actor = Actor.from_hex_key(
        actor_id=args.actor,
        private_key_hex=Path(os.path.expanduser(args.key)).read_text().strip(),
    )

    paper_text = args.paper.read_text() if args.paper else CANNED_PAPER_SNIPPET
    print(f"== extracting claims from {args.paper or '<canned snippet>'} ==")
    claims = extract_claims_with_claude(paper_text)
    print(f"   {len(claims)} claim(s) extracted")
    for i, c in enumerate(claims):
        print(f"   [{i}] {c['text'][:80]}")

    print(f"\n== connecting to {args.frontier_url} ==")
    f = Frontier.connect(args.frontier_url)

    # Pick a target finding for the notes — first finding in the frontier.
    findings = f.list_findings(limit=1)
    if not findings:
        sys.stderr.write("frontier has no findings; cannot attach notes.\n")
        return 1
    target_id = findings[0]["id"]
    print(f"   target finding: {target_id}")

    print("\n== proposing notes ==")
    for c in claims:
        text = c["text"]
        reason = c.get("reason", "extracted by agent")
        proposal = f.propose_note(
            finding_id=target_id,
            text=text,
            reason=reason,
            actor=actor,
        )
        print(f"   {proposal.id}  status={proposal.status}")

    print("\n== events_since(start) ==")
    cursor = None
    for event in f.events_since(cursor):
        print(f"   {event.id} kind={event.kind} target={event.target.id}")
        cursor = event.id

    print("\nDone. Review and accept queued proposals via the Workbench:")
    print(f"   open {args.frontier_url}/previews/proposals.html")
    return 0


if __name__ == "__main__":
    sys.exit(main())
