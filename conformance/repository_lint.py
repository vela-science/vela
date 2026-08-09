#!/usr/bin/env python3
"""Hold one Repository checkout to the consolidations that already happened.

This is not a protocol check. `vela replay` decides whether a repository's
scientific state reproduces; nothing here can change Standing, and a Repository
that fails every rule below still replays. What this checks is the *shape* a
Repository has after machinery moved out of it — one shared package where there
were four copies of a resolver, a list of retired paths, a lock with a
generator that owns it — because that shape has no other guard and copy-paste
restores it in one commit.

Two roots, and only two. `VELA_ROOT` is the checkout this file ships in.
`repository_root` is the argument. A Repository CI job checks out one repository:
`../vela` is not beside it, and nothing below may assume it is. Everything the
linter needs from Vela it reads through `vela_file`, everything it inspects it
reads through `Repository.file` or `Repository.walk`, and each refuses a path that
escapes its base.

Every rule reads the fact it needs from whatever declares it. The alternative
has already failed twice this week: a check that restates a pinned SHA, a
projection version, or a lock's contents goes stale the day the declaration
moves and reddens a repository for a change that was correct. So the shared
file list comes from the package, the retired paths come from the profile
contract, a lock's generator is named by the package that ships it, and no hash
is typed in here at all.

The same instinct says what does *not* belong here. A lock's shape is settled by
`vela-source-lock --check` against the schema that generator publishes, which
the action runs a step ahead of this file; restating it here would put two
validators on one document. Action pinning is settled by `zizmor`, which the
action runs a step ahead of that: its blanket policy already requires a hash
and it reads the workflow rather than the line, so the rule that used to live
here was one repository's regex standing in for a tool the same repository
already ran.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import re
import sys
import tomllib
import warnings
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterator

VELA_ROOT = Path(__file__).resolve().parent.parent

# The wire identifier of a package-consumer reference. This is the name of the
# thing the dependency rule is about, not a pin: if it ever changes, the rule is
# about a different object and has to be rewritten, which is a louder failure
# than a stale hash and is meant to be.
CONSUMER_REFERENCE_SCHEMA = "vela.package-consumer-reference.v1"

# The wire identifiers of the records that qualify a candidate for consumption.
# A candidate lives where nothing is released, so its location cannot say
# whether depending on it was decided or drifted into; only a retained
# qualification record can, and it is the record — not this file — that carries
# the root, the consumers it was computed over, and the gates still failing.
# A candidate of a new kind brings its own schema and has to be named here,
# which is an edit; until then its consumers report, which is the safe way for
# this to be wrong.
CANDIDATE_QUALIFICATION_SCHEMAS = frozenset({"vela.lean-replay-package-qualification.v1"})

# Where Vela retains those records. Scanned rather than listed for the same
# reason `packages/` is: a second record is covered the day it exists.
QUALIFICATION_TREE = "research"

# Directory names whose contents are, by convention across every repository
# here, not a released surface: a candidate, a sample, or a test input. A
# production dependency that resolves into one of these depends on something
# nobody promised to keep.
NON_PRODUCTION_DIRECTORIES = frozenset(
    {
        "research",
        "examples",
        "example",
        "tests",
        "test",
        "fixtures",
        "benchmarks",
        "scratch",
        "sandbox",
    }
)

# Directories the walk never descends into. Dot-directories are excluded as a
# class except `.github` and `.vela`, which is what keeps `.contract-source/`
# out — a Repository CI job checks a slice of *this* repository out in there, and
# a linter that read it would report Vela's own files as a Repository's copies.
PRUNED_DIRECTORIES = frozenset({"__pycache__", "node_modules", "venv"})
KEPT_DOT_DIRECTORIES = frozenset({".github", ".vela"})

# Anchors the retired-path list in the profile contract. A missing marker is a
# hard error rather than an empty rule: a check that silently stops checking is
# the failure mode this whole file exists to avoid.
RETIRED_PATHS_MARKER = "<!-- repository-lint:retired-paths -->"
PROFILE_CONTRACT = "docs/REPOSITORY_PROFILE.md"

# A line that resolves something from git, in any of the three spellings a
# Repository uses: the `uvx --from git+…` invocation a declaration carries, the
# `git = …` table in a pyproject, and the `rev=` query `uv.lock` writes. Prose
# that merely names a package's path is not one of them, which is what keeps
# this off the sentence explaining where the generator lives.
GIT_DEPENDENCY = re.compile(r"git\+|(?<![\w-])git\s*=|(?<![\w-])rev\s*=")
FULL_SHA_IN_TEXT = re.compile(r"(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])")

# How much of a shared module has to reappear in one Repository file before it is
# a copy rather than a coincidence. Calibrated against the real bytes, not
# guessed: the `scripts/write_sources_lock.py` that three epoch-1 Repositories carried
# until today redefines 11 of `resolver.py`'s 20 top-level names (55%), while
# the highest-scoring innocent file in any Repository today reaches 3 names and
# 15% on the strength of `main`, `sha256` and `UA`. Anything between those is
# arbitrary; these two numbers are where the gap is.
COPY_MIN_SYMBOLS = 4
COPY_MIN_FRACTION = 0.25


class ConfigurationError(RuntimeError):
    """A declaration this linter reads is missing or no longer says what it said.

    Raised rather than reported, because a rule that cannot find its own source
    of truth has not passed — it has stopped running, and the two are only
    distinguishable if one of them is loud.
    """


@dataclass(frozen=True, order=True)
class Finding:
    rule: str
    path: str
    line: int
    message: str

    def render(self) -> str:
        where = f"{self.path}:{self.line}" if self.line else self.path
        return f"{where}: {self.rule}: {self.message}"


def _within(base: Path, candidate: Path) -> Path:
    resolved = (base / candidate).resolve() if not candidate.is_absolute() else candidate.resolve()
    if resolved != base.resolve() and base.resolve() not in resolved.parents:
        raise ConfigurationError(f"refusing to read {resolved}, which is outside {base}")
    return resolved


def vela_file(relative: str) -> Path:
    return _within(VELA_ROOT, Path(relative))


# ---------------------------------------------------------------------------
# What Vela declares
# ---------------------------------------------------------------------------


def _parse(source: str) -> ast.Module:
    """Parse without relaying the parsed file's warnings as this tool's output.

    A Repository script with a stale escape sequence has a problem, but not one
    this linter was asked about, and a `SyntaxWarning` on stderr next to a clean
    run reads as a finding.
    """
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", SyntaxWarning)
        return ast.parse(source)


def _top_level_symbols(tree: ast.Module) -> set[str]:
    """Names a module binds at import time and a copy would have to rebind too.

    Constants count. The resolver's identity is as much `PASSTHROUGH` and
    `TIMEOUT` as it is `lock_entry`, and a copy that kept the data and renamed
    the functions is still a copy.
    """
    names: set[str] = set()
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            names.add(node.name)
        elif isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id.isupper():
                    names.add(target.id)
    return names


def _string_constants(tree: ast.Module) -> dict[str, str]:
    out: dict[str, str] = {}
    for node in tree.body:
        if not isinstance(node, ast.Assign) or not isinstance(node.value, ast.Constant):
            continue
        if not isinstance(node.value.value, str):
            continue
        for target in node.targets:
            if isinstance(target, ast.Name):
                out[target.id] = node.value.value
    return out


def _dunder_all(tree: ast.Module) -> set[str]:
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id == "__all__" for t in node.targets
        ):
            if isinstance(node.value, (ast.List, ast.Tuple)):
                return {
                    element.value
                    for element in node.value.elts
                    if isinstance(element, ast.Constant) and isinstance(element.value, str)
                }
    return set()


def _distinctive(name: str) -> bool:
    """Whether redefining this name anywhere else is evidence of anything.

    `check` and `resolve` are English verbs and one Repository already defines the
    first for its own reasons; `write_sources_lock` and `read_declaration` are
    not words anyone reaches for by accident. The test is syntactic — compound
    or not all-lowercase — so it needs no list of blessed words to maintain.
    """
    return "_" in name or not name.islower()


@dataclass(frozen=True)
class SharedPackage:
    name: str
    root: Path
    modules: dict[str, frozenset[str]] = field(default_factory=dict)
    digests: dict[str, str] = field(default_factory=dict)
    exported: frozenset[str] = frozenset()
    constants: dict[str, str] = field(default_factory=dict)
    console_scripts: frozenset[str] = frozenset()


def shared_packages() -> list[SharedPackage]:
    """Every package under `packages/`, described by the files it really has.

    Enumerated rather than listed, so a second shared package is covered by the
    copy rule the moment it exists and without an edit here.
    """
    container = vela_file("packages")
    if not container.is_dir():
        raise ConfigurationError(f"{container} is missing; there is no shared package to compare to")
    packages: list[SharedPackage] = []
    for candidate in sorted(container.iterdir()):
        manifest = candidate / "pyproject.toml"
        if not candidate.is_dir() or not manifest.is_file():
            continue
        source = candidate / "src"
        if not source.is_dir():
            continue
        modules: dict[str, frozenset[str]] = {}
        digests: dict[str, str] = {}
        constants: dict[str, str] = {}
        exported: set[str] = set()
        for path in sorted(source.rglob("*")):
            if not path.is_file() or "__pycache__" in path.parts:
                continue
            relative = path.relative_to(candidate).as_posix()
            digests[hashlib.sha256(path.read_bytes()).hexdigest()] = relative
            if path.suffix != ".py":
                continue
            tree = _parse(path.read_text(encoding="utf-8"))
            symbols = _top_level_symbols(tree)
            if symbols:
                modules[relative] = frozenset(symbols)
            constants.update(_string_constants(tree))
            exported |= _dunder_all(tree)
        declared = tomllib.loads(manifest.read_text(encoding="utf-8"))
        packages.append(
            SharedPackage(
                name=declared.get("project", {}).get("name", candidate.name),
                root=candidate,
                modules=modules,
                digests=digests,
                exported=frozenset(name for name in exported if _distinctive(name)),
                constants=constants,
                console_scripts=frozenset(declared.get("project", {}).get("scripts", {})),
            )
        )
    if not packages:
        raise ConfigurationError(f"no shared package found under {container}")
    return packages


def retired_paths() -> list[str]:
    """The retired-but-unenforced paths, read from the contract that retires them.

    `vela replay` already fails on the paths it owns; restating those here would
    make two owners of one rule that can disagree. This list is the other half —
    paths the profile calls retired and replay deliberately does not reject.
    """
    document = vela_file(PROFILE_CONTRACT).read_text(encoding="utf-8")
    marker = document.find(RETIRED_PATHS_MARKER)
    if marker < 0:
        raise ConfigurationError(
            f"{PROFILE_CONTRACT} no longer carries {RETIRED_PATHS_MARKER}; "
            "the retired-path rule has lost its declaration"
        )
    lines = document[marker:].splitlines()[1:]
    opened = False
    entries: list[str] = []
    for line in lines:
        if line.startswith("```"):
            if opened:
                break
            opened = True
            continue
        if not opened:
            if line.strip():
                raise ConfigurationError(
                    f"{PROFILE_CONTRACT}: {RETIRED_PATHS_MARKER} is not immediately "
                    "followed by the fenced retired-path list"
                )
            continue
        entry = line.strip()
        if entry:
            entries.append(entry)
    if not opened or not entries:
        raise ConfigurationError(f"{PROFILE_CONTRACT}: the retired-path list is empty")
    return entries


def _repository_name(url: str) -> str:
    """The bare repository name in a clone URL, which is what a consumer calls itself."""
    return url.rstrip("/").rsplit("/", 1)[-1].removesuffix(".git")


@dataclass(frozen=True)
class QualifiedCandidate:
    package_id: str
    root: str
    source_path: str
    consumers: frozenset[str]


def qualified_candidates() -> list[QualifiedCandidate]:
    """Candidate packages Vela has qualified, read from the records that qualify them.

    The dependency rule below cannot ask a Repository to stop depending on a
    candidate that Vela's own retained evidence says is a promotion no-go: that
    demand is unsatisfiable from either side, and a rule nobody can satisfy is
    one everybody learns to ignore. What the rule can ask is that the
    dependency be *recorded* — same package, same root, same unreleased path,
    and this repository named among the consumers the record was computed over.

    That is a narrower rule, not a softer one. It fires on a reference no record
    names, on a root that has drifted from the record's, and on a fifth
    repository that copies a reference without being qualified for it. If the
    tree of records disappears, every reference fires at once.
    """
    container = VELA_ROOT / QUALIFICATION_TREE
    if not container.is_dir():
        return []
    candidates: list[QualifiedCandidate] = []
    for path in sorted(container.rglob("*.json")):
        if "__pycache__" in path.parts:
            continue
        try:
            document = json.loads(path.read_bytes())
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if not isinstance(document, dict):
            continue
        if document.get("schema") not in CANDIDATE_QUALIFICATION_SCHEMAS:
            continue
        package = document.get("package")
        consumers = document.get("consumers")
        if not isinstance(package, dict) or not isinstance(consumers, list):
            raise ConfigurationError(
                f"{path.relative_to(VELA_ROOT)} carries a qualification schema but no "
                "package and consumer list; the dependency rule cannot read it"
            )
        identifier = package.get("id")
        root = package.get("root")
        source_path = package.get("source_path")
        if not all(isinstance(value, str) for value in (identifier, root, source_path)):
            raise ConfigurationError(
                f"{path.relative_to(VELA_ROOT)} no longer names one package id, root and "
                "source path; the dependency rule cannot read it"
            )
        named = {
            _repository_name(entry["repository"])
            for entry in consumers
            if isinstance(entry, dict) and isinstance(entry.get("repository"), str)
        }
        candidates.append(
            QualifiedCandidate(
                package_id=identifier,
                root=root,
                source_path=source_path.strip("/"),
                consumers=frozenset(named),
            )
        )
    return candidates


# ---------------------------------------------------------------------------
# What one Repository has
# ---------------------------------------------------------------------------


class Repository:
    def __init__(self, root: Path) -> None:
        self.root = root.resolve()
        if not self.root.is_dir():
            raise ConfigurationError(f"{self.root} is not a directory")

    def file(self, relative: str) -> Path:
        return _within(self.root, Path(relative))

    def relative(self, path: Path) -> str:
        return path.relative_to(self.root).as_posix()

    def walk(self) -> Iterator[Path]:
        """A worktree walk, matching what `vela replay` sees.

        Untracked files included, on purpose: a resolver pasted back in is a
        copy the moment it is on disk, and waiting for it to be committed would
        make the linter useless in the one place it is cheapest to obey.
        """
        stack = [self.root]
        while stack:
            current = stack.pop()
            for entry in sorted(current.iterdir()):
                if entry.is_symlink():
                    continue
                if entry.is_dir():
                    name = entry.name
                    if name in PRUNED_DIRECTORIES:
                        continue
                    if name.startswith(".") and name not in KEPT_DOT_DIRECTORIES:
                        continue
                    stack.append(entry)
                elif entry.is_file():
                    yield entry


# ---------------------------------------------------------------------------
# Rules
# ---------------------------------------------------------------------------


def rule_shared_package_copy(repository: Repository, packages: list[SharedPackage]) -> list[Finding]:
    """A Repository must not carry machinery that now lives in a shared package.

    Three independent signals, because the two copies removed today failed
    differently: three epoch-1 Repositories held byte-identical files, and the fourth held
    a resolver of its own that shared only the entry point's name.
    """
    findings: list[Finding] = []
    for path in repository.walk():
        relative = repository.relative(path)
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        for package in packages:
            if digest in package.digests:
                findings.append(
                    Finding(
                        "shared-package-copy",
                        relative,
                        0,
                        f"byte-identical to {package.name}/{package.digests[digest]}; "
                        f"consume {package.name} instead of carrying it",
                    )
                )
        if path.suffix != ".py":
            continue
        try:
            tree = _parse(data.decode("utf-8", errors="replace"))
        except SyntaxError:
            continue
        symbols = _top_level_symbols(tree)
        if not symbols:
            continue
        for package in packages:
            reexported = sorted(symbols & package.exported)
            if reexported:
                findings.append(
                    Finding(
                        "shared-package-copy",
                        relative,
                        0,
                        f"defines {', '.join(reexported)}, which {package.name} exports; "
                        "one name, two implementations",
                    )
                )
            for module, module_symbols in sorted(package.modules.items()):
                overlap = symbols & module_symbols
                fraction = len(overlap) / len(module_symbols)
                if len(overlap) >= COPY_MIN_SYMBOLS and fraction >= COPY_MIN_FRACTION:
                    findings.append(
                        Finding(
                            "shared-package-copy",
                            relative,
                            0,
                            f"redefines {len(overlap)} of {len(module_symbols)} names "
                            f"({fraction:.0%}) from {package.name}/{module}: "
                            f"{', '.join(sorted(overlap))}",
                        )
                    )
    return findings


def _qualifies(
    document: dict[str, Any], declared: str, candidates: list[QualifiedCandidate]
) -> bool:
    """Whether one retained qualification record covers exactly this reference.

    Every part is compared, because the parts are what make it the same
    dependency: a different package, a different root, or a repository the
    record was not computed over is a dependency nobody qualified.
    """
    package = document.get("package")
    identifier = package.get("id") if isinstance(package, dict) else None
    root = document.get("package_root")
    consumer = document.get("consumer")
    if not all(isinstance(value, str) for value in (identifier, root, consumer)):
        return False
    repository = consumer.split("/")[0]
    return any(
        candidate.package_id == identifier
        and candidate.root == root
        and candidate.source_path == declared.strip("/")
        and repository in candidate.consumers
        for candidate in candidates
    )


def rule_non_production_dependency(
    repository: Repository, candidates: list[QualifiedCandidate]
) -> list[Finding]:
    """No dependency may resolve into a non-released tree unqualified.

    A consumer reference names the exact commit and path it binds. When that
    path opens on `research/` or `examples/`, the Repository is depending on a
    candidate: nothing promises it will be there next release, and the package
    under it usually says so itself.

    One thing answers for such a dependency, and it is not this file: a retained
    qualification record naming the same package, the same root, the same path,
    and this repository. Where that record exists the dependency was decided;
    where it does not, it drifted in, and that is the case here.
    """
    findings: list[Finding] = []
    needle = CONSUMER_REFERENCE_SCHEMA.encode("utf-8")
    for path in repository.walk():
        relative = repository.relative(path)
        if path.suffix == ".json":
            data = path.read_bytes()
            if needle not in data:
                continue
            try:
                document = json.loads(data)
            except json.JSONDecodeError:
                continue
            if not isinstance(document, dict):
                continue
            if document.get("schema") != CONSUMER_REFERENCE_SCHEMA:
                continue
            source = document.get("source")
            declared = source.get("path") if isinstance(source, dict) else None
            if not isinstance(declared, str):
                continue
            head = declared.strip("/").split("/")[0]
            if head in NON_PRODUCTION_DIRECTORIES:
                if _qualifies(document, declared, candidates):
                    continue
                package = document.get("package", {})
                identifier = package.get("id") if isinstance(package, dict) else declared
                findings.append(
                    Finding(
                        "non-production-dependency",
                        relative,
                        0,
                        f"depends on {identifier} at {declared}, inside another "
                        f"repository's {head}/ tree",
                    )
                )
        elif path.name == "pyproject.toml":
            for number, line in enumerate(
                path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1
            ):
                for target in re.findall(r'subdirectory\s*=\s*"([^"]+)"', line):
                    head = target.strip("/").split("/")[0]
                    if head in NON_PRODUCTION_DIRECTORIES:
                        findings.append(
                            Finding(
                                "non-production-dependency",
                                relative,
                                number,
                                f"resolves a dependency from {target}, inside another "
                                f"repository's {head}/ tree",
                            )
                        )
    return findings


def rule_generator_pin(repository: Repository, packages: list[SharedPackage]) -> list[Finding]:
    """A shared package a Repository depends on is named at one immutable commit.

    The lock is only as reproducible as the generator that writes it, and the
    generator is not on an index: a Repository reaches it as a git dependency.
    One of the four declared that dependency in a manifest, where `uv` resolves
    and locks the rev; the other three carry the same `uvx --from git+…@rev`
    invocation in the declaration and nothing at all read it, so `@main` there
    would have looked exactly like a pin and regenerated the lock with whatever
    the branch held that morning.

    Shape and agreement, offline, and nothing else. Which commit is right is not
    a fact this checkout can settle for a Repository it was not shipped with — the
    rule that compared a value would go red for a repository whose pin is
    correct and simply newer. What it can settle is that the reference names a
    40-character commit rather than a moving ref, and that a Repository restating
    it does not restate it differently: erdos-frontier names the same rev in a
    declaration comment, a manifest, a lock and a module docstring, and four
    copies of one commit is four things that can disagree.
    """
    findings: list[Finding] = []
    locators = {}
    for package in packages:
        relative = package.root.relative_to(VELA_ROOT).as_posix()
        locators[package.name] = (relative, relative.replace("/", "%2F"))
    seen: dict[str, dict[str, list[str]]] = {name: {} for name in locators}
    for path in repository.walk():
        if path.suffix in {".png", ".jpg", ".webp", ".woff2", ".zip", ".gz"}:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        relative_path = repository.relative(path)
        for number, line in enumerate(text.splitlines(), start=1):
            if not GIT_DEPENDENCY.search(line):
                continue
            for name, (plain, encoded) in locators.items():
                if plain not in line and encoded not in line:
                    continue
                revisions = sorted(set(FULL_SHA_IN_TEXT.findall(line)))
                if not revisions:
                    findings.append(
                        Finding(
                            "generator-pin",
                            relative_path,
                            number,
                            f"resolves {name} from git without naming a 40-character "
                            "commit, so what it installs is whatever the ref holds today",
                        )
                    )
                    continue
                for revision in revisions:
                    seen[name].setdefault(revision, []).append(f"{relative_path}:{number}")
    for name, revisions in seen.items():
        if len(revisions) > 1:
            where = "; ".join(
                f"{revision[:12]} at {', '.join(places)}"
                for revision, places in sorted(revisions.items())
            )
            findings.append(
                Finding(
                    "generator-pin",
                    repository.relative(repository.root),
                    0,
                    f"pins {name} at {len(revisions)} different commits: {where}",
                )
            )
    return findings


def rule_retired_paths(repository: Repository, retired: list[str]) -> list[Finding]:
    """Retired paths must be absent.

    A directory entry fires only when it holds a file, which is what a worktree
    walk sees and therefore what `vela replay` means by the same word. An empty
    directory left behind by a checkout is not a repository carrying a path.
    """
    findings: list[Finding] = []
    for entry in retired:
        target = repository.file(entry.rstrip("/"))
        if entry.endswith("/"):
            if target.is_dir() and any(item.is_file() for item in target.rglob("*")):
                findings.append(
                    Finding("retired-path", entry, 0, "retired directory still holds files")
                )
        elif target.is_file():
            findings.append(Finding("retired-path", entry, 0, "retired path is still present"))
    return findings


def rule_generated_files(repository: Repository, packages: list[SharedPackage]) -> list[Finding]:
    """A generated file has a declaration behind it, and that declaration says
    what to re-run.

    Not whether the lock's *shape* is right. `vela-source-lock --check` validates
    it against the schema the generator publishes, and the action runs that a
    step before this one; a second validator here would be a second opinion on
    the same bytes from the same schema, and two opinions are how a check starts
    disagreeing with itself.

    What --check cannot answer is a lock with no `sources.yaml` at all — it reads
    the declaration first and stops, and the action skips the step entirely when
    the file is absent. That case is a generated file with no generator behind
    it, which is exactly what this rule is about, so it is owned here.
    """
    findings: list[Finding] = []
    for package in packages:
        lock_name = package.constants.get("LOCK_FILE")
        declaration_name = package.constants.get("DECLARATION_FILE")
        if not lock_name or not declaration_name:
            continue
        lock_path = repository.file(lock_name)
        if not lock_path.is_file():
            continue
        declaration_path = repository.file(declaration_name)
        if not declaration_path.is_file():
            findings.append(
                Finding(
                    "generated-file",
                    lock_name,
                    0,
                    f"has no {declaration_name} to have been derived from",
                )
            )
            continue
        text = declaration_path.read_text(encoding="utf-8", errors="replace")
        if package.console_scripts and not any(
            script in text for script in package.console_scripts
        ):
            findings.append(
                Finding(
                    "generated-file",
                    declaration_name,
                    0,
                    f"never names {' or '.join(sorted(package.console_scripts))}, so nothing "
                    f"here says what regenerates {lock_name}",
                )
            )
    return findings


RULES = (
    "shared-package-copy",
    "non-production-dependency",
    "generator-pin",
    "retired-path",
    "generated-file",
)


def lint(repository_root: Path) -> list[Finding]:
    repository = Repository(repository_root)
    packages = shared_packages()
    findings = [
        *rule_shared_package_copy(repository, packages),
        *rule_non_production_dependency(repository, qualified_candidates()),
        *rule_generator_pin(repository, packages),
        *rule_retired_paths(repository, retired_paths()),
        *rule_generated_files(repository, packages),
    ]
    return sorted(findings)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="repository_lint",
        description="Check one Repository checkout against the shape consolidation left it in.",
    )
    parser.add_argument(
        "repository", nargs="?", default=".", help="path to one Repository checkout"
    )
    parser.add_argument("--json", action="store_true", help="emit findings as JSON")
    arguments = parser.parse_args(argv)

    root = Path(arguments.repository)
    try:
        findings = lint(root)
    except ConfigurationError as error:
        print(f"repository-lint: {error}", file=sys.stderr)
        return 2

    if arguments.json:
        print(
            json.dumps(
                {
                    "schema": "vela.repository-lint.v1",
                    "repository": str(root.resolve()),
                    "rules": list(RULES),
                    "ok": not findings,
                    "findings": [
                        {
                            "rule": finding.rule,
                            "path": finding.path,
                            "line": finding.line,
                            "message": finding.message,
                        }
                        for finding in findings
                    ],
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        for finding in findings:
            print(finding.render())
        name = root.resolve().name
        if findings:
            print(f"repository-lint: {name}: {len(findings)} finding(s) across {len(RULES)} rules")
        else:
            print(f"repository-lint: {name}: ok ({len(RULES)} rules)")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
