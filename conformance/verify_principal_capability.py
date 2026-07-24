#!/usr/bin/env python3
"""Independent verifier for Vela principal and capability conformance."""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path


FIXTURE_ROOT = (
    "sha256:67bf660a0733bbc7579a883e8cc2e1b9ae09843e6ecee856794e2c07f1f5ef2d"
)
AUDIENCE = "vela.repository-authority.v1"
ALLOWED_ACTIONS = {
    "artifact_register",
    "artifact_retract_own",
    "proposal_create",
    "proposal_withdraw_own",
    "receipt_land",
    "verifier_attach",
    "work_claim",
}
HUMAN_ONLY_ACTIONS = {
    "authority_migrate",
    "authority_revoke",
    "authority_rotate",
    "bulk_correct",
    "destroy",
    "membership_manage",
    "policy_activate",
    "policy_revoke",
    "policy_rotate",
    "quorum_manage",
    "recovery_approve",
    "review_accept",
    "review_reject",
}
MAX_LIFETIME_SECONDS = 24 * 60 * 60


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


def root(value: object) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def exact_keys(value: dict, expected: set[str], subject: str) -> None:
    if set(value) != expected:
        raise ValueError(f"{subject} has unknown or missing fields")


def require_root(value: object, subject: str) -> str:
    if not isinstance(value, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None:
        raise ValueError(f"{subject} is not a full SHA-256 root")
    return value


def canonical_time(value: object, subject: str) -> datetime:
    if not isinstance(value, str) or re.fullmatch(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value
    ) is None:
        raise ValueError(f"{subject} is not canonical whole-second UTC RFC3339")
    try:
        return datetime.fromisoformat(value.removesuffix("Z") + "+00:00")
    except ValueError as error:
        raise ValueError(f"{subject} is not a valid time") from error


def strictly_sorted(values: object, subject: str, *, key=lambda value: value) -> list:
    if not isinstance(values, list) or not values:
        raise ValueError(f"{subject} must be a non-empty array")
    comparison = [key(value) for value in values]
    if any(
        comparison[index] >= comparison[index + 1]
        for index in range(len(comparison) - 1)
    ):
        raise ValueError(f"{subject} must be strictly sorted and unique")
    return values


def principal_link_id(link: dict) -> str:
    prefixes = {
        "local": "local",
        "oidc": "oidc",
        "orcid": "orcid",
        "spiffe": "spiffe",
        "github_app": "github-app",
    }
    return f"{prefixes[link['kind']]}:{link['issuer']}|{link['subject']}"


def verify_principal(principal: dict) -> str:
    exact_keys(
        principal,
        {
            "schema",
            "principal_id",
            "principal_class",
            "display_name",
            "affiliation",
            "account_links",
        },
        "principal",
    )
    if (
        principal["schema"] != "vela.principal.v1"
        or principal["principal_class"] != "human"
        or not principal["principal_id"]
    ):
        raise ValueError("principal identity is invalid")
    links = principal["account_links"]
    if not isinstance(links, list) or not links:
        raise ValueError("human principal has no account link")
    identifiers = []
    previous = None
    kind_order = {"local": 0, "oidc": 1, "orcid": 2, "spiffe": 3, "github_app": 4}
    for link in links:
        exact_keys(
            link,
            {"kind", "issuer", "subject", "linked_at", "revoked_at"},
            "account link",
        )
        if link["kind"] not in {"local", "oidc", "orcid"}:
            raise ValueError("human account-link kind is invalid")
        if not link["issuer"] or not link["subject"]:
            raise ValueError("account link issuer or subject is empty")
        linked_at = canonical_time(link["linked_at"], "account linked_at")
        if link["revoked_at"] is not None and canonical_time(
            link["revoked_at"], "account revoked_at"
        ) < linked_at:
            raise ValueError("account revocation precedes linkage")
        identifier = principal_link_id(link)
        if identifier in identifiers:
            raise ValueError("account-link identity is duplicated")
        identifiers.append(identifier)
        encoded = (
            kind_order[link["kind"]],
            link["issuer"],
            link["subject"],
            link["linked_at"],
            link["revoked_at"] or "",
        )
        if previous is not None and previous >= encoded:
            raise ValueError("account links are not in canonical order")
        previous = encoded
    if principal["principal_id"] not in identifiers:
        raise ValueError("human principal ID is not an exact issuer-subject link")
    return root(principal)


def capability_id(capability: dict) -> str:
    content = copy.deepcopy(capability)
    content["capability_id"] = ""
    return "vcap_" + hashlib.sha256(canonical_bytes(content)).hexdigest()[:32]


CAPABILITY_KEYS = {
    "schema",
    "capability_id",
    "issuer_principal_id",
    "subject_principal_id",
    "subject_class",
    "current_actor_principal_id",
    "actor_chain",
    "parent_capability_root",
    "delegation_depth",
    "maximum_delegation_depth",
    "audience",
    "frontier_id",
    "actions",
    "resources",
    "execution_bindings",
    "consequence_ceiling",
    "issued_at",
    "not_before",
    "expires_at",
    "token_id",
    "revocation_ref",
}


def verify_capability(
    capability: dict,
    *,
    observed_at: str | None = None,
    revoked_roots: set[str] | None = None,
) -> str:
    exact_keys(capability, CAPABILITY_KEYS, "capability")
    if (
        capability["schema"] != "vela.capability-grant.v1"
        or capability["capability_id"] != capability_id(capability)
        or capability["subject_class"] not in {"agent", "workload"}
        or capability["current_actor_principal_id"]
        != capability["subject_principal_id"]
        or capability["issuer_principal_id"] == capability["subject_principal_id"]
        or capability["audience"] != AUDIENCE
        or not capability["frontier_id"].startswith("vfr_")
        or capability["consequence_ceiling"] not in {"pending_review", "policy_routed"}
        or not capability["token_id"]
    ):
        raise ValueError("capability identity, audience, or class is invalid")
    if capability["subject_class"] == "agent" and not capability[
        "subject_principal_id"
    ].startswith("agent:"):
        raise ValueError("agent capability subject has the wrong namespace")
    if capability["subject_class"] == "workload" and not capability[
        "subject_principal_id"
    ].startswith(("workload:", "oidc:", "spiffe:", "github-app:")):
        raise ValueError("workload capability subject has the wrong namespace")

    chain = capability["actor_chain"]
    if (
        not isinstance(chain, list)
        or len(chain) < 2
        or len(chain) != len(set(chain))
        or chain[-2] != capability["issuer_principal_id"]
        or chain[-1] != capability["current_actor_principal_id"]
    ):
        raise ValueError("capability actor chain is invalid")
    depth = capability["delegation_depth"]
    maximum = capability["maximum_delegation_depth"]
    parent_root = capability["parent_capability_root"]
    if (
        not isinstance(depth, int)
        or not isinstance(maximum, int)
        or maximum < 0
        or maximum > 1
        or depth < 0
        or depth > maximum
        or (depth == 0) != (parent_root is None)
    ):
        raise ValueError("capability delegation depth is invalid")
    if parent_root is not None:
        require_root(parent_root, "parent capability")
    if capability["revocation_ref"] is not None:
        require_root(capability["revocation_ref"], "capability revocation reference")

    actions = strictly_sorted(capability["actions"], "capability actions")
    if set(actions) - ALLOWED_ACTIONS or set(actions) & HUMAN_ONLY_ACTIONS:
        raise ValueError("capability contains an unsupported or human-only action")
    resources = strictly_sorted(
        capability["resources"],
        "capability resources",
        key=lambda value: (
            value.get("resource_type", ""),
            value.get("resource_id", ""),
            value.get("resource_root", ""),
        ),
    )
    identities = set()
    for resource in resources:
        exact_keys(
            resource,
            {"resource_type", "resource_id", "resource_root"},
            "capability resource",
        )
        identity = (resource["resource_type"], resource["resource_id"])
        if (
            not all(identity)
            or identity in identities
            or require_root(resource["resource_root"], "resource root") is None
        ):
            raise ValueError("capability resource is invalid or ambiguous")
        identities.add(identity)
    bindings = strictly_sorted(
        capability["execution_bindings"],
        "capability execution bindings",
        key=lambda value: (
            value.get("binding_type", ""),
            value.get("binding_root", ""),
        ),
    )
    for binding in bindings:
        exact_keys(
            binding, {"binding_type", "binding_root"}, "capability execution binding"
        )
        if not binding["binding_type"]:
            raise ValueError("execution binding type is empty")
        require_root(binding["binding_root"], "execution binding root")

    issued_at = canonical_time(capability["issued_at"], "capability issued_at")
    not_before = canonical_time(capability["not_before"], "capability not_before")
    expires_at = canonical_time(capability["expires_at"], "capability expires_at")
    if (
        not_before < issued_at
        or expires_at <= not_before
        or int((expires_at - issued_at).total_seconds()) > MAX_LIFETIME_SECONDS
    ):
        raise ValueError("capability validity window is invalid")
    capability_root = root(capability)
    if observed_at is not None:
        observed = canonical_time(observed_at, "capability observed_at")
        if observed < not_before or observed >= expires_at:
            raise ValueError("capability is inactive at observation time")
        if revoked_roots and capability_root in revoked_roots:
            raise ValueError("capability is revoked")
    return capability_root


def verify_delegation(child: dict, parent: dict) -> None:
    child_root = verify_capability(child)
    parent_root = verify_capability(parent)
    del child_root
    if (
        child["parent_capability_root"] != parent_root
        or child["issuer_principal_id"] != parent["subject_principal_id"]
        or child["frontier_id"] != parent["frontier_id"]
        or child["audience"] != parent["audience"]
        or child["delegation_depth"] != parent["delegation_depth"] + 1
        or child["maximum_delegation_depth"] > parent["maximum_delegation_depth"]
        or not set(child["actions"]).issubset(parent["actions"])
        or not {
            canonical_bytes(value) for value in child["resources"]
        }.issubset({canonical_bytes(value) for value in parent["resources"]})
        or not {
            canonical_bytes(value) for value in child["execution_bindings"]
        }.issubset({canonical_bytes(value) for value in parent["execution_bindings"]})
        or (
            child["consequence_ceiling"] == "policy_routed"
            and parent["consequence_ceiling"] != "policy_routed"
        )
    ):
        raise ValueError("child capability broadens or detaches from its parent")
    if child["actor_chain"] != parent["actor_chain"] + [child["subject_principal_id"]]:
        raise ValueError("child capability actor chain is invalid")
    child_issued = canonical_time(child["issued_at"], "child issued_at")
    child_nbf = canonical_time(child["not_before"], "child not_before")
    child_exp = canonical_time(child["expires_at"], "child expires_at")
    parent_issued = canonical_time(parent["issued_at"], "parent issued_at")
    parent_nbf = canonical_time(parent["not_before"], "parent not_before")
    parent_exp = canonical_time(parent["expires_at"], "parent expires_at")
    if child_issued < parent_issued or child_nbf < parent_nbf or child_exp > parent_exp:
        raise ValueError("child capability widens its parent time window")


CLAIM_KEYS = {
    "schema",
    "capability_id",
    "capability_root",
    "issuer_principal_id",
    "subject_principal_id",
    "subject_class",
    "current_actor_principal_id",
    "actor_chain",
    "audience",
    "frontier_id",
    "actions",
    "resource_roots",
    "execution_binding_roots",
    "consequence_ceiling",
    "issued_at",
    "expires_at",
    "token_id",
    "revocation_ref",
    "verified_at",
}


def verify_claim(claim: dict, parent: dict) -> str:
    exact_keys(claim, CLAIM_KEYS, "verified capability claim")
    if (
        claim["schema"] != "vela.verified-capability-claim.v1"
        or claim["capability_id"] != parent["capability_id"]
        or claim["capability_root"] != root(parent)
        or claim["issuer_principal_id"] != parent["issuer_principal_id"]
        or claim["subject_principal_id"] != parent["subject_principal_id"]
        or claim["subject_class"] != parent["subject_class"]
        or claim["current_actor_principal_id"] != parent["current_actor_principal_id"]
        or claim["actor_chain"] != parent["actor_chain"]
        or claim["audience"] != parent["audience"]
        or claim["frontier_id"] != parent["frontier_id"]
        or claim["actions"] != parent["actions"]
        or claim["resource_roots"]
        != [resource["resource_root"] for resource in parent["resources"]]
        or claim["execution_binding_roots"]
        != [binding["binding_root"] for binding in parent["execution_bindings"]]
        or claim["consequence_ceiling"] != parent["consequence_ceiling"]
        or claim["issued_at"] != parent["issued_at"]
        or claim["expires_at"] != parent["expires_at"]
        or claim["token_id"] != parent["token_id"]
        or claim["revocation_ref"] != parent["revocation_ref"]
    ):
        raise ValueError("verified capability claim differs from its grant")
    verified_at = canonical_time(claim["verified_at"], "claim verified_at")
    issued_at = canonical_time(claim["issued_at"], "claim issued_at")
    expires_at = canonical_time(claim["expires_at"], "claim expires_at")
    if verified_at < issued_at or verified_at >= expires_at:
        raise ValueError("verified capability claim time is invalid")
    return root(claim)


def reroot_fixture(fixture: dict) -> None:
    body = copy.deepcopy(fixture)
    body.pop("fixture_root", None)
    fixture["fixture_root"] = root(body)


def verify_fixture(fixture: dict) -> dict:
    exact_keys(
        fixture,
        {
            "schema",
            "principal",
            "parent_capability",
            "child_capability",
            "verified_claim",
            "expected",
            "fixture_root",
        },
        "principal-capability fixture",
    )
    body = copy.deepcopy(fixture)
    supplied_root = body.pop("fixture_root")
    if supplied_root != root(body):
        raise ValueError("principal-capability fixture root is invalid")
    if fixture["schema"] != "vela.principal-capability-conformance.v1":
        raise ValueError("principal-capability fixture schema is invalid")

    principal_root = verify_principal(fixture["principal"])
    parent_root = verify_capability(
        fixture["parent_capability"], observed_at="2026-07-24T12:30:00Z"
    )
    child_root = verify_capability(
        fixture["child_capability"], observed_at="2026-07-24T12:30:00Z"
    )
    verify_delegation(fixture["child_capability"], fixture["parent_capability"])
    claim_root = verify_claim(fixture["verified_claim"], fixture["parent_capability"])
    expected = {
        "principal_root": principal_root,
        "parent_capability_id": fixture["parent_capability"]["capability_id"],
        "parent_capability_root": parent_root,
        "child_capability_id": fixture["child_capability"]["capability_id"],
        "child_capability_root": child_root,
        "verified_claim_root": claim_root,
    }
    if expected != fixture["expected"]:
        raise ValueError("independently derived principal-capability report differs")
    return expected


def main() -> int:
    fixture_path = (
        Path(__file__).resolve().parent
        / "fixtures"
        / "principal-capability-v1.json"
    )
    try:
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        if fixture.get("fixture_root") != FIXTURE_ROOT:
            raise ValueError("fixture root differs from the independently pinned root")
        verify_fixture(fixture)

        hostile = []
        email_identity = copy.deepcopy(fixture)
        email_identity["principal"]["principal_id"] = "fixture@example.com"
        reroot_fixture(email_identity)
        hostile.append(("email identity inference", email_identity))

        human_subject = copy.deepcopy(fixture)
        human_subject["parent_capability"]["subject_class"] = "human"
        human_subject["parent_capability"]["capability_id"] = capability_id(
            human_subject["parent_capability"]
        )
        reroot_fixture(human_subject)
        hostile.append(("human capability subject", human_subject))

        human_action = copy.deepcopy(fixture)
        human_action["parent_capability"]["actions"].append("review_accept")
        human_action["parent_capability"]["actions"].sort()
        human_action["parent_capability"]["capability_id"] = capability_id(
            human_action["parent_capability"]
        )
        reroot_fixture(human_action)
        hostile.append(("human-only action", human_action))

        long_lived = copy.deepcopy(fixture)
        long_lived["parent_capability"]["expires_at"] = "2026-07-26T12:00:00Z"
        long_lived["parent_capability"]["capability_id"] = capability_id(
            long_lived["parent_capability"]
        )
        reroot_fixture(long_lived)
        hostile.append(("long-lived capability", long_lived))

        widened_child = copy.deepcopy(fixture)
        widened_child["child_capability"]["actions"].append("verifier_attach")
        widened_child["child_capability"]["actions"].sort()
        widened_child["child_capability"]["capability_id"] = capability_id(
            widened_child["child_capability"]
        )
        reroot_fixture(widened_child)
        hostile.append(("delegation broadening", widened_child))

        parent_substitution = copy.deepcopy(fixture)
        parent_substitution["child_capability"]["parent_capability_root"] = (
            "sha256:" + "0" * 64
        )
        parent_substitution["child_capability"]["capability_id"] = capability_id(
            parent_substitution["child_capability"]
        )
        reroot_fixture(parent_substitution)
        hostile.append(("parent substitution", parent_substitution))

        bearer = copy.deepcopy(fixture)
        bearer["verified_claim"]["bearer_token"] = "must-not-enter-history"
        reroot_fixture(bearer)
        hostile.append(("bearer-token retention", bearer))

        for name, candidate in hostile:
            try:
                verify_fixture(candidate)
            except ValueError:
                continue
            raise ValueError(f"hostile case unexpectedly verified: {name}")

        parent = fixture["parent_capability"]
        try:
            verify_capability(
                parent,
                observed_at="2026-07-24T12:30:00Z",
                revoked_roots={root(parent)},
            )
        except ValueError:
            pass
        else:
            raise ValueError("revoked capability unexpectedly verified")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"principal-capability conformance: FAIL: {error}", file=sys.stderr)
        return 1

    print(
        "principal-capability conformance: ok "
        f"({fixture['fixture_root']}; 8 hostile cases)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
