"""Generate ``sources.lock.json`` from ``sources.yaml``.

The lock records the exact content root of every source a Frontier acquires, so
accepted state is traceable to fixed bytes rather than to a floating `main` or to
whatever a publisher happens to serve today.

The rule this module exists to enforce: **every hash here is computed from bytes
this code actually fetched or read.** A declared hash is never copied through
from ``sources.yaml``. Where a source declares one, the declaration is treated as
an assertion to check, and a mismatch fails the run rather than being retained
under the same commit. Where no content hash can be computed at all, the entry
says so in ``unlocked`` and gives the reason. A hash nobody computed is worse
than no hash at all, and a source silently dropped is worse still.

Every entry therefore carries exactly one of:

  ``sha256``       a content root computed here from the bytes named by
                   ``url`` or ``path``;
  ``exact_roots``  per-file content roots computed here, for a repository
                   pinned at a commit whose individual files are the retained
                   evidence;
  ``unlocked``     a sentence saying why no content hash exists for this entry.

``error`` marks a source that should have been lockable and was not. It is
written into the lock so the gap is visible, and the run then exits non-zero.

Nothing in the lock comes from the clock. The file used to open with a
``generated_at`` stamp, which made two runs over identical inputs produce
different bytes and so made the obvious check — re-resolve, then
``git diff --exit-code`` — impossible to write. A lock that cannot be diffed
against a re-run is a lock nobody can audit, which is the whole of what it is
for. When the lock was made is a question Git answers, and answers better: the
commit dates the record, while a stamp only dated a process that may have run
days before anyone committed its output. Whether the lock is *stale* is a
different question again, and ``--check --refetch`` is what asks it.

That invariant is stated once more, machine-readably, in
``schemas/sources-lock.schema.json``, and this module validates its own output
against it before returning. A resolver that trusts its own output is a resolver
whose bugs reach the lock.
"""

from __future__ import annotations

import hashlib
import json
import os
import urllib.error
import urllib.request
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from . import schema

DECLARATION_FILE = "sources.yaml"
LOCK_FILE = "sources.lock.json"

UA = {"User-Agent": "vela-source-manifest"}
TIMEOUT = 90

Fetch = Callable[[str, Mapping[str, str] | None], bytes]


# Locators and repository identity are copied through to the lock verbatim. They
# are recorded outside the fetch branches on purpose: a routine refresh that
# cannot reach the network must still leave the inventory provenance intact
# rather than quietly narrowing the lock to whatever it could reach.
PASSTHROUGH = (
    "repo",
    "ref",
    "path",
    "paths",
    "commit",
    "tree",
    "home",
    "homepage",
    "pages_commit",
    "pages_commit_resolved",
)


def urlopen_fetch(url: str, headers: Mapping[str, str] | None = None) -> bytes:
    request = urllib.request.Request(url, headers={**UA, **(headers or {})})
    with urllib.request.urlopen(request, timeout=TIMEOUT) as response:
        return response.read()


def github_headers() -> dict[str, str]:
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    return {"Authorization": f"Bearer {token}"} if token else {}


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def is_repository_landing_page(url: str, repo: str | None) -> bool:
    """True when `url` is a repository's front page rather than a locator for
    bytes. Fetching one yields rendered HTML, and recording a hash of that HTML
    as the content root would be a false pin that looks exactly like a real one.
    """
    if not repo:
        return False
    return url.rstrip("/") in (
        f"https://github.com/{repo}",
        f"http://github.com/{repo}",
    )


@dataclass
class Resolution:
    """The lock a run produced and every reason the run failed.

    Problems are accumulated rather than raised so the whole inventory is
    attempted and the lock still gets written. The caller decides the exit code;
    library code that calls `sys.exit` cannot be used by a caller that wanted to
    look at the result first.
    """

    payload: dict[str, Any]
    problems: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.problems


def read_declaration(root: Path) -> tuple[dict[str, Any], list[str]]:
    """Parse and validate a Frontier's `sources.yaml`.

    A declaration that parses but says the wrong thing fails here, at the
    producer, rather than downstream in whatever consumes the lock.

    `safe_load` resolves an unquoted `2026-08-05T21:22:46Z` to a datetime, which
    the declaration schema then rejects as not a string. That rejection is the
    behavior wanted: the two timestamps a Frontier declares are quoted, and the
    lock copies them through verbatim, so an unquoted one has to stop the run
    rather than be coerced into a value the source never wrote.
    """
    origin = root / DECLARATION_FILE
    document = yaml.safe_load(origin.read_text(encoding="utf-8")) or {}
    problems = schema.validate(document, schema.DECLARATION_SCHEMA, DECLARATION_FILE)
    return document, problems


def read_lock(root: Path) -> tuple[dict[str, Any], list[str]]:
    origin = root / LOCK_FILE
    document = json.loads(origin.read_text(encoding="utf-8"))
    problems = schema.validate(document, schema.LOCK_SCHEMA, LOCK_FILE)
    return document, problems


def resolve_commit(repo: str, rev: str, fetch: Fetch) -> tuple[str, str]:
    """Return the (commit, tree) GitHub reports for `rev`, read from the API
    response rather than from what sources.yaml claims they are.
    """
    payload = json.loads(
        fetch(f"https://api.github.com/repos/{repo}/commits/{rev}", github_headers())
    )
    return payload["sha"], payload["commit"]["tree"]["sha"]


def _check_declared_revision(
    name: str, spec: Mapping[str, Any], observed_commit: str, observed_tree: str, problems: list[str]
) -> None:
    """Hold the declaration to what GitHub actually serves.

    A moved pin is the failure this whole file exists to catch: the same commit
    id in sources.yaml resolving to different bytes upstream is precisely the
    floating-`main` problem wearing a hash.
    """
    declared_commit = spec.get("commit")
    if declared_commit and declared_commit != observed_commit:
        problems.append(
            f"{name}: sources.yaml pins commit {declared_commit}, "
            f"but GitHub resolved it to {observed_commit}"
        )
    if spec.get("tree") and spec["tree"] != observed_tree:
        problems.append(
            f"{name}: sources.yaml declares tree {spec['tree']}, "
            f"but commit {observed_commit} has tree {observed_tree}"
        )


def lock_exact_roots(
    entry: dict[str, Any],
    spec: Mapping[str, Any],
    name: str,
    fetch: Fetch,
    problems: list[str],
) -> None:
    """Lock a repository pinned at a commit by recomputing the content root of
    each file the acquisition treats as exact. The declared hashes in
    sources.yaml are assertions to check, never values to copy.
    """
    repo, commit = spec["repo"], spec["commit"]
    observed_commit, observed_tree = resolve_commit(repo, commit, fetch)
    _check_declared_revision(name, spec, observed_commit, observed_tree, problems)
    entry["commit"] = observed_commit
    entry["tree"] = observed_tree

    roots: dict[str, dict[str, str]] = {}
    for key, declared in sorted(spec["exact_roots"].items()):
        path = declared["path"]
        url = f"https://raw.githubusercontent.com/{repo}/{observed_commit}/{path}"
        computed = sha256(fetch(url, None))
        roots[key] = {"path": path, "url": url, "sha256": computed}
        if declared.get("sha256") and declared["sha256"] != computed:
            problems.append(
                f"{name}/{key}: sources.yaml declares {declared['sha256']} for {path}, "
                f"but {observed_commit} serves {computed}"
            )
    entry["exact_roots"] = roots


def lock_entry(
    root: Path, name: str, spec: Mapping[str, Any], fetch: Fetch, problems: list[str]
) -> dict[str, Any]:
    entry: dict[str, Any] = {"kind": spec.get("kind")}
    for field_name in PASSTHROUGH:
        if spec.get(field_name) is not None:
            entry[field_name] = spec[field_name]
    url = spec.get("url")
    if url is not None:
        entry["url"] = url

    # Cited, not acquired. Another Frontier holds the bytes; its url is a landing
    # page, so the declared commit and tree are the whole of the pin.
    if spec.get("acquired_by"):
        entry["acquired_by"] = spec["acquired_by"]
        entry["unlocked"] = (
            f"cited, not acquired: the bytes are acquired by the {spec['acquired_by']} "
            "frontier and are not retained here, so the pin is the declared commit and tree"
        )
        return entry

    # Consulted as a reference. No bytes are retained, so there is nothing to
    # hash, and fetching the page to manufacture a root would misrepresent a
    # consultation as an acquisition.
    if spec.get("kind") == "reference_only":
        entry["unlocked"] = (
            "reference only: the frontier records that this was consulted, not what "
            "it said, so no bytes are retained and there is no content root to compute"
        )
        return entry

    try:
        if spec.get("exact_roots"):
            lock_exact_roots(entry, spec, name, fetch, problems)
            return entry

        if spec.get("path") is not None:
            target = root / spec["path"]
            if target.is_file():
                if url is not None:
                    # `path` carries two meanings across the Frontiers. On an
                    # entry with no url it names bytes retained in this
                    # repository (sources/fidelity_cache.json). On a url-backed
                    # entry it names the file's location in the *upstream*
                    # repository (data/problems.yaml in teorth/erdosproblems),
                    # and the url is the locator. Today the two never collide,
                    # because no upstream path also exists locally. If one ever
                    # did — a vendored copy, a coincidence of names — silently
                    # hashing it would switch the pin from upstream bytes to
                    # local bytes under an entry that still names the url, and
                    # the lock would read exactly as it does now. Refuse rather
                    # than pick: only the author knows which is the acquisition.
                    entry["error"] = (
                        f"ambiguous declaration: {spec['path']} exists in this repository "
                        f"and {url} is declared as well, so which of the two holds the "
                        "acquired bytes is not stated. Declare one, or move the upstream "
                        "path into the url"
                    )
                    problems.append(f"{name}: {entry['error']}")
                    return entry
                entry["sha256"] = sha256(target.read_bytes())
                return entry
            if url is None:
                entry["error"] = f"declared path {spec['path']} does not exist in this repository"
                problems.append(f"{name}: {entry['error']}")
                return entry

        if url is not None:
            if is_repository_landing_page(url, spec.get("repo")):
                observed_commit, observed_tree = resolve_commit(spec["repo"], spec["commit"], fetch)
                _check_declared_revision(name, spec, observed_commit, observed_tree, problems)
                entry["commit"], entry["tree"] = observed_commit, observed_tree
                entry["unlocked"] = (
                    "no content locator: the url is the repository landing page, not bytes, "
                    "and this frontier retains no snapshot. The pin is the commit and tree "
                    "above, both read from the GitHub API at generation time"
                )
                return entry

            entry["sha256"] = sha256(fetch(url, None))
            if spec.get("repo") and spec.get("ref"):
                # The content root is already computed and is the durable pin;
                # the commit only records which revision the ref pointed at when
                # those bytes were read. So a GitHub API failure here is reported
                # and fails the run, but it does not discard a root that was
                # computed from bytes we actually hold. Losing a good hash to an
                # unrelated rate limit is a worse outcome than an incomplete
                # entry that says so.
                try:
                    entry["commit"], entry["tree"] = resolve_commit(spec["repo"], spec["ref"], fetch)
                except (urllib.error.URLError, OSError, json.JSONDecodeError, KeyError) as exc:
                    problems.append(
                        f"{name}: locked a content root, but could not resolve "
                        f"{spec['repo']}@{spec['ref']} to a commit ({type(exc).__name__}: {exc})"
                    )
            return entry

        entry["unlocked"] = (
            "no url and no in-repository path: nothing to compute a content root from"
        )
        return entry

    except (urllib.error.URLError, OSError, json.JSONDecodeError, KeyError) as exc:
        entry["error"] = f"{type(exc).__name__}: {exc}"
        problems.append(f"{name}: could not lock ({entry['error']})")
        return entry


def resolve(root: str | Path, fetch: Fetch | None = None) -> Resolution:
    """Build the lock payload for the Frontier rooted at `root`. Writes nothing.

    `fetch` defaults to `urlopen_fetch`, resolved here rather than bound as a
    default argument so a test can substitute one without the network.
    """
    fetch = fetch or urlopen_fetch
    root = Path(root)
    document, problems = read_declaration(root)

    # A declaration that fails its own schema is not an inventory yet: there is
    # nothing here worth fetching, and nothing a lock could honestly record a
    # gap against. The empty declaration is one of these — the schema requires
    # `sources` and requires it non-empty — and so is a value of the wrong type,
    # which is why stopping here also keeps a refused value away from the JSON
    # encoder. An unquoted timestamp arrives from YAML as a datetime, and
    # copying it through would raise out of `render` with a traceback where the
    # reason belongs.
    if problems:
        return Resolution(payload={"sources": {}}, problems=problems)

    registry = document["sources"]
    locked = {
        name: lock_entry(root, name, spec, fetch, problems) for name, spec in registry.items()
    }
    payload = {"sources": locked}

    # The output is held to the same schema a consumer will hold it to. A
    # resolver bug that produces an entry with both a sha256 and an unlocked
    # reason should fail here, not in whatever reads the lock next month.
    if locked:
        problems.extend(schema.validate(payload, schema.LOCK_SCHEMA, LOCK_FILE))
    return Resolution(payload=payload, problems=problems)


def render(payload: Mapping[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def write_sources_lock(root: str | Path = ".", fetch: Fetch | None = None) -> Resolution:
    """Resolve and write `sources.lock.json`.

    The lock is written even when the run failed, so the gap is on the record
    rather than only in a terminal that has since scrolled away. That holds for
    a source the run could not pin: the entry says so, and the file carries it.

    It does not hold for a declaration that never became an inventory. The
    schema requires at least one source, so an empty result means the run never
    read one, and writing it would replace a lock full of computed roots with a
    file recording nothing. There is no gap to preserve, only the previous
    record to destroy, so the lock on disk is left where it is.
    """
    root = Path(root)
    resolution = resolve(root, fetch)
    if resolution.payload["sources"]:
        (root / LOCK_FILE).write_text(render(resolution.payload), encoding="utf-8")
    return resolution
