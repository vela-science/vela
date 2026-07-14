from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .core import (
    dump_json,
    emit_receipt,
    load_json,
    strict_json_loads,
    validate_receipt,
)


def _verifier_run(spec: str) -> dict[str, str]:
    parts = spec.split(":", 3)
    if len(parts) < 2:
        raise argparse.ArgumentTypeError("verifier run must be method:outcome[:log[:solver]]")
    run = {"method": parts[0], "outcome": parts[1]}
    if len(parts) > 2:
        run["log"] = parts[2]
    if len(parts) > 3:
        run["solver"] = parts[3]
    return run


def _first_eval_claim(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.lower().startswith("claim:"):
            return stripped.split(":", 1)[1].strip()
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("#"):
            return stripped.lstrip("#").strip()
    for line in text.splitlines():
        stripped = line.strip()
        if stripped:
            return stripped
    raise ValueError(f"{path} does not contain a claim")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="vela-receipt-v0")
    sub = parser.add_subparsers(dest="command", required=True)

    emit = sub.add_parser("emit", help="emit a vela.receipt.v1 JSON file")
    emit.add_argument("--claim", required=True)
    emit.add_argument("--type", default="computational", dest="claim_type")
    emit.add_argument("--replayability", default="unknown")
    emit.add_argument("--artifact", action="append", default=[])
    emit.add_argument("--caveat", action="append", default=[])
    emit.add_argument("--verifier-run", type=_verifier_run, action="append", default=[])
    emit.add_argument("--generated-by", default="vela-receipt-v0")
    emit.add_argument("--submitter-actor")
    emit.add_argument("--source-system")
    emit.add_argument("--source-uri")
    emit.add_argument("--run-id")
    emit.add_argument("--base-dir", default=".")
    emit.add_argument("--condition", action="append", default=[])
    emit.add_argument("--verification-requirement", action="append", default=[])
    emit.add_argument("--state-diff-json")
    emit.add_argument("--no-intoto", action="store_true", help="omit the in-toto attestation extension")
    emit.add_argument("--out")

    validate = sub.add_parser("validate", help="validate a vela.receipt.v1 JSON file")
    validate.add_argument("receipt")

    orx = sub.add_parser("emit-openresearch", help="emit from an OpenResearch-shaped EVAL.md plus diff")
    orx.add_argument("--eval", required=True, dest="eval_path")
    orx.add_argument("--diff", required=True, dest="diff_path")
    orx.add_argument("--artifact", action="append", default=[])
    orx.add_argument("--claim")
    orx.add_argument("--replayability", default="bounded")
    orx.add_argument("--caveat", action="append", default=[])
    orx.add_argument("--verifier-run", type=_verifier_run, action="append", default=[])
    orx.add_argument("--source-uri")
    orx.add_argument("--run-id", default="openresearch-shaped-run")
    orx.add_argument("--submitter-actor")
    orx.add_argument("--base-dir", default=".")
    orx.add_argument("--out")

    args = parser.parse_args(argv)
    try:
        if args.command == "emit":
            state_diff = strict_json_loads(args.state_diff_json) if args.state_diff_json else {}
            receipt = emit_receipt(
                claim=args.claim,
                artifacts=args.artifact,
                caveats=args.caveat,
                claim_type=args.claim_type,
                replayability=args.replayability,
                verifier_runs=args.verifier_run,
                generated_by=args.generated_by,
                submitter_actor=args.submitter_actor,
                source_system=args.source_system,
                source_uri=args.source_uri,
                run_id=args.run_id,
                base_dir=Path(args.base_dir),
                conditions=args.condition,
                verification_requirements=args.verification_requirement,
                state_diff=state_diff,
                include_intoto=not args.no_intoto,
            )
            dump_json(receipt, args.out)
            return 0
        if args.command == "validate":
            validate_receipt(load_json(args.receipt))
            print("OK receipt validates")
            return 0
        if args.command == "emit-openresearch":
            base_dir = Path(args.base_dir)
            eval_path = Path(args.eval_path)
            diff_path = Path(args.diff_path)
            claim = args.claim or _first_eval_claim(eval_path)
            caveats = args.caveat or [
                "OpenResearch-shaped producer output is activity. Vela landing, verifier re-derivation, and human acceptance are separate."
            ]
            artifacts = [f"{eval_path}:eval", f"{diff_path}:code_diff", *args.artifact]
            receipt = emit_receipt(
                claim=claim,
                artifacts=artifacts,
                caveats=caveats,
                claim_type="computational",
                replayability=args.replayability,
                verifier_runs=args.verifier_run or [{
                    "method": "producer eval",
                    "outcome": "unknown",
                    "log": "OpenResearch-shaped EVAL.md imported as a claim, not a Vela verdict.",
                }],
                generated_by="vela-receipt-v0/openresearch",
                submitter_actor=args.submitter_actor,
                source_system="OpenResearch-shaped",
                source_uri=args.source_uri,
                run_id=args.run_id,
                base_dir=base_dir,
                conditions=["EVAL.md and diff were read as data, not executed."],
                verification_requirements=["A Vela verifier registry entry must independently re-derive the claim before acceptance."],
                state_diff={
                    "frontier": "producer-selected",
                    "source_claim_id": args.run_id,
                    "artifact_shape": "openresearch_eval_diff",
                },
            )
            dump_json(receipt, args.out)
            return 0
    except Exception as e:
        print(f"ERROR {e}", file=sys.stderr)
        return 1
    return 2
