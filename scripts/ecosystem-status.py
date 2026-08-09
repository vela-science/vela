#!/usr/bin/env python3
"""Generate and check `ecosystem-status.json`, the ecosystem's truthfulness file.

This is not a database and it holds no authority. It exists because prose about
the ecosystem drifted away from the ecosystem without anything noticing: four
repositories documented as archived were still `archived=false` on GitHub, and
`docs/ECOSYSTEM.md` carried a list of five things "not done" that were all done,
including an arrow whose two sides had been rewritten to the same word.

Both failures share one shape. A sentence asserted something checkable, and
nothing checked it. So this file records the assertions in machine-readable
form, splits them by what can actually be observed, and fails when an assertion
and an observation disagree.

Two blocks, and the difference between them is the whole design:

  `local`     derived from this checkout's tracked content alone. Deterministic:
              no timestamps, no Git metadata, no network. `--check` recomputes
              it and requires an exact match, so a change that moves a named
              surface fails until someone regenerates and looks at the diff.

  `observed`  facts about other repositories, each stamped with when and how it
              was observed. `--check` never recomputes these — it cannot reach
              the network from conformance — but it does hold them to the
              declaration (a repository documented as frozen must not be
              observed live) and to an age limit.

A declared repository with no observation is reported as `not_observed`. That
is a true statement about our knowledge and is treated as such: it does not fail
the check, and it does not get to look like a pass either.

  scripts/ecosystem-status.py                       regenerate `local`, keep `observed`
  scripts/ecosystem-status.py --check               verify without writing
  scripts/ecosystem-status.py --checkout math=../math --observe-remote
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import re
import shlex
import subprocess
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parent.parent
ARTIFACT = ROOT / "ecosystem-status.json"
SCHEMA = "vela.ecosystem-status.v1"
DEFAULT_MAX_OBSERVATION_AGE_DAYS = 180
# The manifest shape `observe_projection` knows how to read. vela-web pins the
# same literal at `packages/frontier-data/src/projection-contract.ts`, and the
# live manifest at app.vela.space serves it. A rename there must stop this
# script rather than let it parse an unfamiliar document and report what it
# failed to find as a fact.
PROJECTION_SCHEMA = "vela.observatory-release-manifest"


# One row per repository the ecosystem names.
#
# `responsibility` is the single thing that repository owns; if two rows could
# carry the same sentence, one of them should not exist
# (`docs/REPOSITORY_BOUNDARIES.md`).
#
# `active_writer` and `read_replicas` are the continuity rule written down.
# `docs/CONTINUITY.md` §3 fixes exactly one remote as the writer and makes every
# other one a read replica until a human promotes it, and says the record lives
# here. An observed checkout pointing somewhere not on this row is either a
# promotion nobody wrote down or a mistake, and the check does not try to tell
# them apart: it stops and asks.
#
# The codeberg.org entries are replicas that exist and are verified, not
# aspirations. `vela-web`'s `mirror-replicas.yml` pushes to them twice daily and
# then reads them back over the public URL with no credential, failing if they
# hold different refs from the primary. Listing a replica that nothing refreshes
# would be worse than listing none: it answers the continuity question with a
# URL instead of a copy.
DECLARED_REPOSITORIES: dict[str, dict[str, object]] = {
    "vela-science/vela": {
        "active_writer": "https://github.com/vela-science/vela.git",
        "visibility": "public",
        "read_replicas": ["https://codeberg.org/vela-science/vela.git"],
        "responsibility": "Protocol semantics, the vela CLI, wire schemas, conformance, and releases",
        "state": "active",
    },
    "vela-science/vela-web": {
        "active_writer": "https://github.com/vela-science/vela-web.git",
        "visibility": "private",
        "read_replicas": [],
        "responsibility": "Root-bound read projections, the Observatory, and the editorial site",
        "state": "active",
    },
    "vela-science/.github": {
        "active_writer": "https://github.com/vela-science/.github.git",
        "visibility": "public",
        "read_replicas": [],
        "responsibility": "Organization profile, shared workflows, and security policy",
        "state": "active",
    },
    "vela-science/math": {
        "active_writer": "https://github.com/vela-science/math.git",
        "visibility": "public",
        "read_replicas": ["https://codeberg.org/vela-science/math.git"],
        "responsibility": "The one live mathematics authority: sources, Claims, Decisions, replay state",
        "state": "active",
    },
    "vela-science/erdos-frontier": {
        "active_writer": "https://github.com/vela-science/erdos-frontier.git",
        "visibility": "public",
        "read_replicas": [],
        "responsibility": "Historical epoch-1 repository, preserved for its signed history",
        "state": "frozen",
    },
    "vela-science/sidon-frontier": {
        "active_writer": "https://github.com/vela-science/sidon-frontier.git",
        "visibility": "public",
        "read_replicas": [],
        "responsibility": "Historical epoch-1 repository, preserved for its signed history",
        "state": "frozen",
    },
    "vela-science/quantum-codes-frontier": {
        "active_writer": "https://github.com/vela-science/quantum-codes-frontier.git",
        "visibility": "public",
        "read_replicas": [],
        "responsibility": "Historical epoch-1 repository, preserved for its signed history",
        "state": "frozen",
    },
    "vela-science/formal-conjectures-frontier": {
        "active_writer": "https://github.com/vela-science/formal-conjectures-frontier.git",
        "visibility": "public",
        "read_replicas": [],
        "responsibility": "Historical epoch-1 repository, preserved for its signed history; dissolved into erdos-frontier",
        "state": "frozen",
    },
}


# Surfaces this repository's own documentation names. `expected` is what the
# documentation says is true today, so a row flipping is either a real change or
# a documentation error, and either way it needs a human to look at it.
DECLARED_SURFACES: dict[str, bool] = {
    ".github/release/check-sbom.py": True,
    ".github/release/smoke-bundle.sh": True,
    ".github/workflows/conformance.yml": True,
    ".github/workflows/release.yml": True,
    "action.yml": True,
    "conformance/canonical-hashing.json": True,
    "conformance/emitters/javascript.mjs": True,
    "conformance/emitters/python.py": True,
    "conformance/fixtures/correction": True,
    "conformance/readers/python": True,
    "conformance/verify.py": True,
    "crates/vela-edge/src/analysis/correction_impact.rs": True,
    "crates/vela-protocol/src/objects/repository.rs": True,
    "crates/vela-protocol/src/wire_schema.rs": True,
    # The four documents `docs/CONTINUITY.md` cites. A citation whose target
    # moved is the same failure as a surface that moved, one indirection out.
    "docs/CONTINUITY.md": True,
    "docs/PUBLISHING.md": True,
    "docs/REPOSITORY_BOUNDARIES.md": True,
    "docs/SIGNING.md": True,
    "docs/THREAT_MODEL.md": True,
    "install.sh": True,
    "packages/vela-source-manifest": True,
    "scripts/ecosystem-status.py": True,
    "scripts/release.sh": True,
    "scripts/sign-published-release.sh": True,
    ".github/workflows/ecosystem-status.yml": True,
    "scripts/release_manifest.py": True,
    # Declared absent. A declared-absent surface is worth as much as a
    # declared-present one: it is how "not built" stops being a claim nobody
    # rechecks. The two are absent for different reasons, and the reason is the
    # part worth writing down. No JavaScript reader has been built. `epoch1/`
    # was built, verified against all four checkouts, and then deleted when ADR
    # 0039's same-day amendment withdrew §8 — so this row guards a decision
    # rather than tracking a gap, and a directory reappearing here is the
    # epoch-1 branch coming back.
    "conformance/readers/javascript": False,
    "crates/vela-protocol/src/epoch1": False,
}


# The identifier spellings ADR 0039 retired, and the trees they must be absent
# from. `docs/ROOTS.md` keeps `vfr_` bound to epoch 1 on purpose, so the prose
# that names the retired prefix is not a violation; these trees are code.
RETIRED_SPELLINGS = ("vfr_", "frontier_id", "frontier.toml")
RETIRED_SPELLING_TREES = ("crates", "schemas", "packages")


class Failure(Exception):
    """A checked assertion disagreed with what is on disk."""


def run(arguments: list[str], cwd: pathlib.Path | None = None) -> str | None:
    try:
        completed = subprocess.run(
            arguments,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    return completed.stdout.strip() or None


def now() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


# --------------------------------------------------------------------------
# `local`: deterministic, offline, derived from tracked content only.
# --------------------------------------------------------------------------


def workspace_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as source:
        return tomllib.load(source)["workspace"]["package"]["version"]


def workspace_crates() -> list[str]:
    with (ROOT / "Cargo.toml").open("rb") as source:
        members = tomllib.load(source)["workspace"]["members"]
    return sorted(pathlib.PurePosixPath(member).name for member in members)


def toolchain_channel() -> str:
    with (ROOT / "rust-toolchain.toml").open("rb") as source:
        return tomllib.load(source)["toolchain"]["channel"]


def repository_id_contract() -> str:
    """Read the current repository-id contract out of the protocol source.

    Taking it from the source rather than from a constant here is the point: a
    wire cut that changes identity and forgets this file produces a diff, not a
    quiet agreement between two copies of the same string.
    """
    text = (ROOT / "crates/vela-protocol/src/shape.rs").read_text(
        encoding="utf-8"
    )
    matches = set(
        re.findall(r'REPOSITORY_ID_CONTRACT: &str = "([a-z0-9-]+)"', text)
    )
    if len(matches) != 1:
        raise Failure(
            "shape.rs does not pin exactly one repository-id contract: "
            f"{sorted(matches) or 'none found'}"
        )
    return matches.pop()


def retired_spelling_sites() -> list[str]:
    """Every file under the code trees that still writes a retired spelling.

    A count would be brittle and a boolean would be false. `docs/ECOSYSTEM.md`
    claims these are at zero "and the occurrences that remain are deliberate
    references to the retired spelling" — the retired-path predicate and the
    tests that hold the new wording in place. A list is the only form of that
    claim a reader can check: a new path appearing is drift, and a path
    disappearing means a test stopped guarding something.
    """
    sites: list[str] = []
    for tree in RETIRED_SPELLING_TREES:
        directory = ROOT / tree
        if not directory.is_dir():
            continue
        for path in sorted(directory.rglob("*")):
            if not path.is_file() or "__pycache__" in path.parts:
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            if any(spelling in text for spelling in RETIRED_SPELLINGS):
                sites.append(path.relative_to(ROOT).as_posix())
    return sorted(sites)


def local_block() -> dict[str, object]:
    schemas = sorted(path.name for path in (ROOT / "schemas").glob("*.schema.json"))
    workflows = sorted(path.name for path in (ROOT / ".github/workflows").glob("*.yml"))
    surfaces = {}
    for name in sorted(DECLARED_SURFACES):
        present = (ROOT / name).exists()
        surfaces[name] = {"declared": DECLARED_SURFACES[name], "present": present}
    return {
        "epoch": {
            "repository_id_contract": repository_id_contract(),
            "retired_spelling_sites": retired_spelling_sites(),
            "retired_spellings": list(RETIRED_SPELLINGS),
            "retired_spelling_trees": list(RETIRED_SPELLING_TREES),
        },
        "published_schemas": schemas,
        "release": {
            "toolchain_channel": toolchain_channel(),
            "version": workspace_version(),
        },
        "repository": "vela-science/vela",
        "surfaces": surfaces,
        "workflows": workflows,
        "workspace_crates": workspace_crates(),
    }


# --------------------------------------------------------------------------
# `observed`: everything that needs a checkout, a binary, or a network.
# --------------------------------------------------------------------------


def observe_checkout(path: pathlib.Path, vela_bin: str | None) -> dict[str, object]:
    """Observe one Vela repository from a local clone.

    Deliberately records no filesystem path. This file is published; where a
    maintainer keeps their clones is not ecosystem state.
    """
    observation: dict[str, object] = {
        "method": "local-checkout",
        "observed_at": now(),
    }
    profile = path / "vela.toml"
    if profile.is_file():
        with profile.open("rb") as source:
            declared = tomllib.load(source)
        observation["repository_id"] = declared.get("repository_id")

    origin = path / ".vela/origin.json"
    if origin.is_file():
        document = json.loads(origin.read_text(encoding="utf-8"))
        observation["origin"] = {
            "generation": document.get("generation"),
            "kind": document.get("kind"),
            "origin_id": document.get("origin_id"),
        }

    state = path / ".vela/repository.json"
    if state.is_file():
        document = json.loads(state.read_text(encoding="utf-8"))
        observation["counts"] = {
            key: len(document[key])
            for key in (
                "accepted_claims",
                "pending_claims",
                "proposals",
                "submissions",
                "verifications",
                "artifacts",
            )
            if isinstance(document.get(key), list)
        }
        observation["origin_root"] = document.get("origin_root")

    lock = path / "sources.lock.json"
    if lock.is_file():
        document = json.loads(lock.read_text(encoding="utf-8"))
        sources = document.get("sources")
        if isinstance(sources, dict):
            observation.setdefault("counts", {})["declared_sources"] = len(sources)

    commit = run(["git", "rev-parse", "HEAD"], cwd=path)
    if commit:
        observation["git"] = {
            "commit": commit,
            "remote": run(["git", "config", "--get", "remote.origin.url"], cwd=path),
            "tree": run(["git", "rev-parse", "HEAD^{tree}"], cwd=path),
        }

    # The repository root is derived by replay, not stored, so it is only
    # available when a binary built from the same commit as the data can be run.
    # `docs/ROOTS.md` is explicit that a stored field is not a substitute.
    if vela_bin and (path / ".vela").is_dir():
        status = run([vela_bin, "status", str(path), "--json"])
        if status:
            try:
                document = json.loads(status)
            except json.JSONDecodeError:
                document = None
            if isinstance(document, dict):
                observation["replay"] = {
                    "integrity": document.get("integrity"),
                    "repository_root": (document.get("repository") or {}).get(
                        "repository_root"
                    ),
                    "reported_by": run([vela_bin, "--version"]),
                }
    return observation


def observe_remote(name: str, gh_bin: str) -> dict[str, object] | None:
    # `--gh-bin` is split as a command line so a wrapper that takes its own
    # arguments works. The wrapper's name and account are not recorded: which
    # local credential helper answered is not ecosystem state.
    command = shlex.split(gh_bin)
    payload = run(
        [*command, "api", f"repos/{name}", "--jq", "{archived,visibility,pushed_at,default_branch}"]
    )
    if payload is None:
        return None
    try:
        document = json.loads(payload)
    except json.JSONDecodeError:
        return None
    return {
        "archived": document.get("archived"),
        "default_branch": document.get("default_branch"),
        "method": f"github-api repos/{name}",
        "observed_at": now(),
        "pushed_at": document.get("pushed_at"),
        "visibility": document.get("visibility"),
    }


def observe_projection(url: str) -> dict[str, object] | None:
    import urllib.error
    import urllib.request

    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            document = json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, json.JSONDecodeError, ValueError):
        return None
    projection = document.get("projection")
    if not isinstance(projection, dict):
        return None
    # The deployed manifest still keys its per-repository rows `source_frontiers`
    # under `vela.observatory-release-manifest`; renaming that key moves a
    # published root, so it is read as it is served rather than corrected here.
    repositories = []
    for entry in projection.get("source_frontiers") or []:
        if not isinstance(entry, dict):
            continue
        repositories.append(
            {
                "commit": entry.get("commit"),
                "counts": {
                    key.removesuffix("_count"): entry[key]
                    for key in sorted(entry)
                    if key.endswith("_count") and isinstance(entry[key], int)
                },
                "origin_root": entry.get("origin_root"),
                "repository_root": entry.get("repository_root"),
                "slug": entry.get("slug"),
                "tree": entry.get("tree"),
            }
        )
    return {
        "activation_time": projection.get("activation_time"),
        "method": "https-get",
        "observed_at": now(),
        "projected_repositories": repositories,
        "projection_schema": projection.get("schema"),
        "release_root": projection.get("release_root"),
        "source": url,
        "vela_version": projection.get("vela_version"),
    }


# --------------------------------------------------------------------------
# Assemble, write, check.
# --------------------------------------------------------------------------


def load_artifact() -> dict[str, object]:
    if not ARTIFACT.is_file():
        return {}
    return json.loads(ARTIFACT.read_text(encoding="utf-8"))


def build(previous: dict[str, object], arguments: argparse.Namespace) -> dict[str, object]:
    observed: dict[str, object] = dict(previous.get("observed") or {})

    for entry in arguments.checkout:
        name, _, raw = entry.partition("=")
        if not name or not raw:
            raise Failure(f"malformed --checkout {entry!r}; expected NAME=PATH")
        qualified = name if "/" in name else f"vela-science/{name}"
        if qualified not in DECLARED_REPOSITORIES:
            raise Failure(f"{qualified} is not a declared repository")
        path = pathlib.Path(raw).expanduser().resolve()
        if not path.is_dir():
            raise Failure(f"no such checkout: {raw}")
        merged = dict(observed.get(qualified) or {})
        merged.update(observe_checkout(path, arguments.vela_bin))
        observed[qualified] = merged

    if arguments.observe_remote:
        for name in DECLARED_REPOSITORIES:
            remote = observe_remote(name, arguments.gh_bin)
            if remote is None:
                print(f"ecosystem-status: could not observe {name} remotely", file=sys.stderr)
                continue
            merged = dict(observed.get(name) or {})
            merged["remote"] = remote
            observed[name] = merged

    projection = previous.get("projection")
    if arguments.observe_projection:
        fresh = observe_projection(arguments.observe_projection)
        if fresh is None:
            print(
                f"ecosystem-status: could not read {arguments.observe_projection}",
                file=sys.stderr,
            )
        else:
            projection = fresh

    return {
        "schema": SCHEMA,
        "declaration": {
            name: dict(row) for name, row in sorted(DECLARED_REPOSITORIES.items())
        },
        "generator": "scripts/ecosystem-status.py",
        "local": local_block(),
        "observed": {name: observed[name] for name in sorted(observed)},
        "projection": projection,
    }


def serialize(document: dict[str, object]) -> str:
    return json.dumps(document, indent=2, sort_keys=True) + "\n"


def drift(stored: object, observed: object, path: str) -> list[str]:
    """Report the leaves that differ, not the subtrees that contain them.

    A whole-block dump is technically a diff and practically unreadable, which
    is how a check that fires gets ignored.
    """
    if isinstance(stored, dict) and isinstance(observed, dict):
        lines: list[str] = []
        for key in sorted(set(stored) | set(observed)):
            if stored.get(key) != observed.get(key):
                lines.extend(drift(stored.get(key), observed.get(key), f"{path}.{key}"))
        return lines
    return [f"{path}: stored {stored!r} / observed {observed!r}"]


def check(arguments: argparse.Namespace) -> list[str]:
    failures: list[str] = []
    committed = load_artifact()
    if not committed:
        return [f"{ARTIFACT.name} is missing; run scripts/ecosystem-status.py"]

    if committed.get("schema") != SCHEMA:
        failures.append(f"schema is {committed.get('schema')!r}, expected {SCHEMA!r}")

    expected_local = local_block()
    if committed.get("local") != expected_local:
        failures.append(
            "the `local` block no longer describes this checkout. "
            "Regenerate with scripts/ecosystem-status.py and read the diff."
        )
        failures.extend(
            f"  {line}" for line in drift(committed.get("local") or {}, expected_local, "local")
        )

    declaration = committed.get("declaration") or {}
    for name in DECLARED_REPOSITORIES:
        if name not in declaration:
            failures.append(f"{name} is declared in the generator but absent from the artifact")

    # Presence was the whole check, so the artifact could disagree with the
    # generator about every field and still pass as long as the keys lined up.
    # It did: `read_replicas` stayed `[]` in the committed artifact after the
    # generator gained the codeberg.org replicas, and the file whose job is to
    # catch documentation drift was the thing that had drifted. Compared in
    # full, the same way the `local` block already is.
    expected_declaration = json.loads(json.dumps(DECLARED_REPOSITORIES, sort_keys=True))
    if declaration != expected_declaration:
        failures.append(
            "the `declaration` block no longer matches DECLARED_REPOSITORIES. "
            "Regenerate with scripts/ecosystem-status.py and read the diff."
        )
        failures.extend(
            f"  {line}" for line in drift(declaration, expected_declaration, "declaration")
        )

    # The assertion, as opposed to the drift check above: a surface the
    # documentation names has to be there, and one it says was never built has
    # to still be absent. Both directions were wrong at once in
    # `docs/ECOSYSTEM.md`, so both directions are checked.
    for name, row in sorted((expected_local.get("surfaces") or {}).items()):
        if row["declared"] and not row["present"]:
            failures.append(f"{name} is named as existing and does not exist")
        if not row["declared"] and row["present"]:
            failures.append(
                f"{name} is documented as absent and now exists; "
                "update the declaration and whatever prose called it missing"
            )

    observed = committed.get("observed") or {}
    horizon = dt.datetime.now(dt.UTC) - dt.timedelta(days=arguments.max_age_days)
    for name, row in sorted(observed.items()):
        declared = DECLARED_REPOSITORIES.get(name, {})
        declared_state = declared.get("state")
        remote = row.get("remote") if isinstance(row, dict) else None

        # `docs/CONTINUITY.md` §3: one writer, and promotion is a human decision
        # with a recorded runbook. A checkout that already points elsewhere is
        # how that decision gets made by accident.
        observed_remote = (row.get("git") or {}).get("remote") if isinstance(row, dict) else None
        if observed_remote:
            permitted = [declared.get("active_writer"), *(declared.get("read_replicas") or [])]
            if observed_remote not in permitted:
                failures.append(
                    f"{name}: the observed checkout pushes to {observed_remote}, which is "
                    f"neither the declared active writer ({declared.get('active_writer')}) "
                    "nor a declared read replica"
                )

        if isinstance(remote, dict):
            archived = remote.get("archived")
            if declared_state == "frozen" and archived is False:
                failures.append(
                    f"{name} is documented as frozen but the host reports archived=false"
                )
            if declared_state == "active" and archived is True:
                failures.append(
                    f"{name} is documented as active but the host reports archived=true"
                )
            # Observed on every repository and asserted on none, so a repository
            # going public — or a public one going private and taking a
            # documented surface offline — was recorded and passed. Declared, a
            # flip in either direction stops and asks, which is the same idiom
            # DECLARED_SURFACES already uses for a surface that is meant to be
            # absent.
            declared_visibility = declared.get("visibility")
            observed_visibility = remote.get("visibility")
            if declared_visibility and observed_visibility and declared_visibility != observed_visibility:
                failures.append(
                    f"{name} is declared {declared_visibility} but the host reports "
                    f"{observed_visibility}"
                )
        # A row is a container until something observed it. Only entries
        # carrying a `method` are observations, and only those owe an instant.
        for label, entry in (("", row), ("remote ", remote)):
            if not isinstance(entry, dict) or "method" not in entry:
                continue
            stamp = entry.get("observed_at")
            if not isinstance(stamp, str):
                failures.append(f"{name}: {label}observation carries no observed_at")
                continue
            try:
                when = dt.datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ").replace(
                    tzinfo=dt.UTC
                )
            except ValueError:
                failures.append(f"{name}: {label}observed_at {stamp!r} is not an instant")
                continue
            if when < horizon:
                # Naming the command matters more here than anywhere else in
                # this file. The fuse fires inside conformance, which has no
                # network and no checkouts and therefore cannot repair what it
                # is refusing — so the message has to carry the repair out to
                # someone who can run it, or it is an alarm with no exit.
                repair = (
                    f"scripts/ecosystem-status.py --checkout {name.split('/')[-1]}=<path> "
                    "--observe-remote"
                    if label == ""
                    else "scripts/ecosystem-status.py --observe-remote"
                )
                failures.append(
                    f"{name}: {label}observation from {stamp} is older than "
                    f"{arguments.max_age_days} days. Re-observe with `{repair}`, "
                    "or drop the row if it can no longer be observed at all"
                )

    # The projection block sits at the top level rather than under `observed`,
    # so the loop above never reached it and nothing checked it at all: it
    # carries `method` and `observed_at` like every other observation, and a
    # three-year-old reading of the deployed projection read as a pass. Same
    # predicate, same horizon.
    projection = committed.get("projection")
    if isinstance(projection, dict) and "method" in projection:
        stamp = projection.get("observed_at")
        if not isinstance(stamp, str):
            failures.append("projection: observation carries no observed_at")
        else:
            try:
                when = dt.datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.UTC)
            except ValueError:
                failures.append(f"projection: observed_at {stamp!r} is not an instant")
            else:
                if when < horizon:
                    failures.append(
                        f"projection: observation from {stamp} is older than "
                        f"{arguments.max_age_days} days; re-observe with --observe-projection"
                    )
        # `observe_projection` reads `source_frontiers` out of the manifest, so
        # it assumes a schema. Recording the id without checking it means a
        # renamed manifest would be parsed as whatever the old shape was and the
        # emptiness reported as fact.
        observed_schema = projection.get("projection_schema")
        if observed_schema is not None and observed_schema != PROJECTION_SCHEMA:
            failures.append(
                f"projection: manifest schema is {observed_schema!r}, and this script "
                f"parses {PROJECTION_SCHEMA!r}"
            )
    return failures


def report_unobserved(committed: dict[str, object]) -> None:
    observed = committed.get("observed") or {}
    unobserved = [name for name in DECLARED_REPOSITORIES if name not in observed]
    if unobserved:
        print("not observed: " + ", ".join(sorted(unobserved)))


def main() -> int:
    parser = argparse.ArgumentParser(prog="ecosystem-status")
    parser.add_argument("--check", action="store_true", help="verify without writing")
    parser.add_argument(
        "--checkout",
        action="append",
        default=[],
        metavar="NAME=PATH",
        help="observe one repository from a local clone; repeat per repository",
    )
    parser.add_argument(
        "--observe-remote",
        action="store_true",
        help="read archived/visibility state for every declared repository",
    )
    parser.add_argument(
        "--observe-projection",
        metavar="URL",
        default=None,
        help="read the deployed site manifest for projected repository roots",
    )
    parser.add_argument("--gh-bin", default=os.environ.get("VELA_GH_BIN", "gh"))
    parser.add_argument("--vela-bin", default=os.environ.get("VELA_BIN"))
    parser.add_argument(
        "--max-age-days",
        type=int,
        default=int(
            os.environ.get(
                "VELA_ECOSYSTEM_MAX_OBSERVATION_AGE_DAYS",
                DEFAULT_MAX_OBSERVATION_AGE_DAYS,
            )
        ),
    )
    arguments = parser.parse_args()

    try:
        if arguments.check:
            failures = check(arguments)
            if failures:
                print("ecosystem-status: FAIL", file=sys.stderr)
                for failure in failures:
                    print(f"  {failure}", file=sys.stderr)
                return 1
            report_unobserved(load_artifact())
            print("ecosystem-status: ok")
            return 0

        document = build(load_artifact(), arguments)
        ARTIFACT.write_text(serialize(document), encoding="utf-8")
        report_unobserved(document)
        print(f"ecosystem-status: wrote {ARTIFACT.name}")
        return 0
    except Failure as error:
        print(f"ecosystem-status: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
