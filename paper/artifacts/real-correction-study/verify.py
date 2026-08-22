#!/usr/bin/env python3
"""Deterministically qualify the open real-correction fixtures.

This verifier performs no network, provider, scoring, authority, or protected-key
operation. It checks retained bytes, Git blob identities, bounded dependency
inventories, the historical Erdős 264 authority signature, common arm atoms,
and the public non-confirmatory discrimination cases.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey


class QualificationError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise QualificationError(message)


def canonical(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def git_blob(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def read_json(path: Path) -> object:
    raw = path.read_bytes()
    return json.loads(raw)


def declaration_blocks(source: str) -> dict[str, str]:
    pattern = re.compile(
        r"(?m)^(?:noncomputable\s+)?(?:def|theorem|lemma)\s+([A-Za-z0-9_.]+)"
    )
    matches = list(pattern.finditer(source))
    blocks: dict[str, str] = {}
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(source)
        blocks[match.group(1)] = source[match.start() : end]
    return blocks


def full_index_diff(predecessor: bytes, successor: bytes, source_path: str) -> bytes:
    with tempfile.TemporaryDirectory() as directory:
        temporary = Path(directory)
        old = temporary / "old" / source_path
        new = temporary / "new" / source_path
        old.parent.mkdir(parents=True)
        new.parent.mkdir(parents=True)
        old.write_bytes(predecessor)
        new.write_bytes(successor)
        completed = subprocess.run(
            [
                "git",
                "diff",
                "--no-index",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--full-index",
                "--",
                f"old/{source_path}",
                f"new/{source_path}",
            ],
            cwd=temporary,
            check=False,
            capture_output=True,
        )
        require(completed.returncode == 1 and not completed.stderr, "Git diff reconstruction")
        return completed.stdout.replace(b"a/old/", b"a/").replace(b"b/new/", b"b/")


def verify_source(root: Path, fixture: dict[str, object]) -> tuple[str, str]:
    source = fixture["source"]
    assert isinstance(source, dict)
    data_by_side: dict[str, bytes] = {}
    texts: dict[str, str] = {}
    roots: list[dict[str, object]] = []
    for side in ("predecessor", "successor"):
        binding = source[side]
        assert isinstance(binding, dict)
        path = root / str(binding["retained_path"])
        data = path.read_bytes()
        require(len(data) == binding["bytes"], f"{fixture['id']} {side} byte count")
        require(sha256(data) == binding["sha256"], f"{fixture['id']} {side} SHA-256")
        require(git_blob(data) == binding["blob"], f"{fixture['id']} {side} Git blob")
        data_by_side[side] = data
        texts[side] = data.decode("utf-8")
        roots.append(
            {
                "side": side,
                "commit": binding["commit"],
                "tree": binding["tree"],
                "blob": binding["blob"],
                "sha256": binding["sha256"],
            }
        )
    require(texts["predecessor"] != texts["successor"], f"{fixture['id']} no transition")
    reconstructed = full_index_diff(
        data_by_side["predecessor"], data_by_side["successor"], str(source["path"])
    )
    require(
        sha256(reconstructed) == source["upstream_full_index_diff_sha256"],
        f"{fixture['id']} full-index diff",
    )
    return texts["predecessor"], texts["successor"]


def verify_bounded_scope(fixture: dict[str, object], predecessor: str, successor: str) -> None:
    fixture_id = fixture["id"]
    ground = fixture["bounded_ground_truth"]
    assert isinstance(ground, dict)
    require(ground["complete"] is True, f"{fixture_id} incomplete scope")
    consequences = ground["consequences"]
    assert isinstance(consequences, list)
    consequence_ids = [str(row["id"]) for row in consequences]
    require(len(consequence_ids) == len(set(consequence_ids)), f"{fixture_id} duplicate consequence")
    blocks = declaration_blocks(successor)

    if fixture_id == "erdos-264-integer-perturbation":
        require("∀ b : ℕ → ℕ" in predecessor, "Erdos 264 predecessor type")
        require("∀ b : ℕ → ℤ" in successor, "Erdos 264 successor type")
        require("BddBelow (Set.range b)" in successor, "Erdos 264 lower bound")
        consumers = {
            name
            for name, block in blocks.items()
            if name != "IsIrrationalitySequence"
            and name.startswith("erdos_264")
            and "IsIrrationalitySequence" in block
        }
        expected = {
            item.removeprefix("Erdos264.")
            for item in consequence_ids
            if item.startswith("Erdos264.erdos_264")
        }
        require(consumers == expected, "Erdos 264 direct consumer closure")
    elif fixture_id == "snake-induced-path":
        require("G'.verts = {v | v ∈ P.support}" in predecessor, "snake predecessor relation")
        require("G' = P.toSubgraph" in successor, "snake successor relation")
        expected = {
            item.removeprefix("SnakeInBox.") for item in consequence_ids
        }
        require(set(blocks) == expected, "snake file-local declaration closure")
    elif fixture_id == "erdos-1055-exclusive-prime-class":
        exclusion = "(∀ (m : ℕ+) (hm : m ≤ n), ¬ H m hm p) ∧"
        require(exclusion not in predecessor, "Erdos 1055 predecessor exclusion")
        require(exclusion in successor, "Erdos 1055 successor exclusion")
        expected = {
            item.removeprefix("Erdos1055.")
            for item in consequence_ids
            if item != "Erdos1055.class-one-base-clause"
        }
        require(set(blocks) == expected, "Erdos 1055 file-local declaration closure")
    else:
        raise QualificationError(f"unknown fixture {fixture_id}")


def pae(payload_type: str, payload: bytes) -> bytes:
    encoded_type = payload_type.encode("utf-8")
    return b" ".join(
        [
            b"DSSEv1",
            str(len(encoded_type)).encode("ascii"),
            encoded_type,
            str(len(payload)).encode("ascii"),
            payload,
        ]
    )


def verify_erdos_264_authority(root: Path, fixture: dict[str, object]) -> None:
    scenario = fixture["authority_scenario"]
    assert isinstance(scenario, dict)
    require(scenario["regime"] == "independently_authorized_acceptance", "264 regime")
    paths = [root / str(path) for path in scenario["evidence_paths"]]
    by_name = {path.name: path for path in paths}
    keyset_path = next(path for path in paths if "keysets" in path.parts)
    record_path = next(path for path in paths if "records" in path.parts)
    keyset = read_json(keyset_path)
    envelope = read_json(record_path)
    assert isinstance(keyset, dict) and isinstance(envelope, dict)
    require(sha256(canonical(keyset)) == "sha256:" + keyset_path.stem, "authority keyset root")
    require(keyset["threshold"] == 1 and len(keyset["keys"]) == 1, "authority threshold")
    payload = base64.b64decode(envelope["payload"], validate=True)
    record = json.loads(payload)
    require(canonical(record) == payload, "authority payload canonical")
    require(record["schema"] == "vela.authority-record.v1", "authority record schema")
    content = record["content"]
    require(content["authority_keyset_root"] == sha256(canonical(keyset)), "record keyset binding")
    require(content["authorization"]["evaluation"]["decision"] == "allow", "authorization allow")
    require(content["authorization"]["evaluation"]["valid"] is True, "authorization valid")
    require(
        any(item["action"] == "review_accept" for item in content["semantic_approvals"]),
        "semantic approval",
    )
    require(set(content["event_ids"]) == {"vev_0325f467077ed92e", "vev_2e265868d5a76496"}, "event set")
    key = keyset["keys"][0]
    signature = next(item for item in envelope["signatures"] if item["keyid"] == key["key_id"])
    try:
        Ed25519PublicKey.from_public_bytes(bytes.fromhex(key["public_key"])).verify(
            base64.b64decode(signature["sig"], validate=True),
            pae(envelope["payloadType"], payload),
        )
    except (InvalidSignature, ValueError) as error:
        raise QualificationError("authority signature") from error
    delta = {row["path"]: row["after_root"] for row in content["object_delta"]}
    for event_id in content["event_ids"]:
        event_path = by_name[f"{event_id}.json"]
        event = read_json(event_path)
        assert isinstance(event, dict)
        require(event["id"] == event_id, "authority event identity")
        relative = f".vela/authority/events/{event_id}.json"
        require(sha256(event_path.read_bytes()) == delta[relative], "authority event delta")
    require(not any("private" in key.lower() for key in keyset), "private key field retained")
    fidelity_path = by_name["erdos-264-source-transition.v1.json"]
    target_path = by_name["erdos-264-parts-i-proof-repair.json"]
    fidelity = read_json(fidelity_path)
    target = read_json(target_path)
    assert isinstance(fidelity, dict) and isinstance(target, dict)
    require(
        sha256(fidelity_path.read_bytes())
        == "sha256:4443284e9856a2df1902dd81fb443f4042fb28b510278bfa2fe23ef935be3173",
        "fidelity artifact root",
    )
    require(
        sha256(target_path.read_bytes())
        == "sha256:112931d7959a3f9201ea4c8402daef3d91ae25410aba1c8fc6765ce69888e3de",
        "proof-repair target root",
    )
    source = fixture["source"]
    assert isinstance(source, dict)
    require(fidelity["predecessor"]["commit"] == source["predecessor"]["commit"], "fidelity predecessor")
    require(fidelity["successor"]["commit"] == source["successor"]["commit"], "fidelity successor")
    direct = {
        row["symbol"] for row in fidelity["direct_consumer_scope"]["consumers"]
    }
    ground = fixture["bounded_ground_truth"]
    assert isinstance(ground, dict)
    expected_direct = {
        row["id"]
        for row in ground["consequences"]
        if row["id"].startswith("Erdos264.erdos_264")
    }
    require(direct == expected_direct, "first-party direct consumer closure")
    require(target["target"]["state"] == "available_after_accepted_correction", "proof target state")


def verify_discrimination(root: Path) -> dict[str, object]:
    data = read_json(root / "discrimination-cases.json")
    assert isinstance(data, dict)
    cases = data["cases"]
    assert isinstance(cases, list)
    regimes = [row["authority_regime"] for row in cases]
    actions = [row["safe_next_action"] for row in cases]
    require(len(cases) == 3, "discrimination case count")
    require(len(set(regimes)) == 3, "discrimination regimes")
    require(len(set(actions)) == 3, "discrimination actions")
    expected = dict(zip(regimes, actions, strict=True))
    resolver = {
        "no_authorized_acceptance_action": "prepare_submission_no_status_change",
        "independently_authorized_acceptance": "reassess_dependents_without_new_decision",
        "authorization_presently_unprovable": "withhold_status_change_request_authority_chain",
    }
    require(resolver == expected, "authority resolver contract")
    baseline = data["fact_only_baseline"]["constant_action"]
    baseline_exact = sum(action == baseline for action in actions)
    require(baseline_exact == 1, "fact-only baseline must be non-ceiling")
    return {
        "cases": len(cases),
        "fact_only_exact": baseline_exact,
        "authority_aware_exact": len(cases),
        "source_fact_extraction_is_sufficient": False,
    }


def qualify(root: Path) -> dict[str, object]:
    packet = read_json(root / "fixture-qualification.json")
    arms = read_json(root / "arm-contract.json")
    assert isinstance(packet, dict) and isinstance(arms, dict)
    require(packet["status"] == "open_qualification_only", "qualification status")
    fixtures = packet["fixtures"]
    assert isinstance(fixtures, list)
    require(len(fixtures) == 3, "fixture count")
    regimes: set[str] = set()
    fixture_roots: list[dict[str, object]] = []
    for fixture in fixtures:
        assert isinstance(fixture, dict)
        predecessor, successor = verify_source(root, fixture)
        verify_bounded_scope(fixture, predecessor, successor)
        qualification = fixture["qualification"]
        assert isinstance(qualification, dict)
        require(all(value is True for value in qualification.values()), f"{fixture['id']} qualification")
        scenario = fixture["authority_scenario"]
        assert isinstance(scenario, dict)
        regimes.add(str(scenario["regime"]))
        semantic_atoms = {
            "source": fixture["source"],
            "correction": fixture["correction"],
            "authority_scenario": scenario,
            "bounded_ground_truth": fixture["bounded_ground_truth"],
        }
        fixture_roots.append({"id": fixture["id"], "atomic_facts_root": sha256(canonical(semantic_atoms))})
        if fixture["id"] == "erdos-264-integer-perturbation":
            verify_erdos_264_authority(root, fixture)
    require(
        regimes
        == {
            "no_authorized_acceptance_action",
            "independently_authorized_acceptance",
            "authorization_presently_unprovable",
        },
        "prospective authority regime coverage",
    )
    require(packet["research_boundary"]["protected_final_key_created"] is False, "protected key boundary")
    require(len(arms["arms"]) == 3, "arm count")
    require(
        [arm["id"] for arm in arms["arms"]]
        == ["git-documents", "structured-state", "vela"],
        "arm order",
    )
    discrimination = verify_discrimination(root)
    result = {
        "schema": "vela.real-correction-qualification-result.v1",
        "status": "qualified_for_open_method_development",
        "fixture_count": len(fixtures),
        "fixtures": fixture_roots,
        "authority_regimes": sorted(regimes),
        "discrimination": discrimination,
        "comparison_arms": [arm["id"] for arm in arms["arms"]],
        "identical_semantic_atoms_required": True,
        "protected_final_key_created": False,
        "confirmatory_freeze_allowed": False,
        "confirmatory_blockers": [
            "Git/documents arm non-ceiling has not been demonstrated with independent open-pilot observations.",
            "Fresh held-out real correction families have not been selected.",
            "Independent methodological review has not accepted the gates and custody plan."
        ],
        "authority_effect": "none"
    }
    result["qualification_root"] = sha256(canonical(result))
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    try:
        result = qualify(args.root.resolve())
    except (OSError, json.JSONDecodeError, QualificationError, KeyError, TypeError) as error:
        print(canonical({"status": "fail", "error": str(error)}).decode("utf-8"))
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
