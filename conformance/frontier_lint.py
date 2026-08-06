#!/usr/bin/env python3
"""Hold one Frontier checkout to the consolidations that already happened.

This is not a protocol check. `vela replay` decides whether a repository's
scientific state reproduces; nothing here can change Standing, and a Frontier
that fails every rule below still replays. What this checks is the *shape* a
Frontier has after machinery moved out of it — one shared package where there
were four copies of a resolver, a list of retired paths, a lock with a
generator that owns it — because that shape has no other guard and copy-paste
restores it in one commit.

Two roots, and only two. `VELA_ROOT` is the checkout this file ships in.
`frontier_root` is the argument. A Frontier CI job checks out one repository:
`../vela` is not beside it, and nothing below may assume it is. Everything the
linter needs from Vela it reads through `vela_file`, everything it inspects it
reads through `Frontier.file` or `Frontier.walk`, and each refuses a path that
escapes its base.

Every rule reads the fact it needs from whatever declares it. The alternative
has already failed twice this week: a check that restates a pinned SHA, a
projection version, or a lock's contents goes stale the day the declaration
moves and reddens a repository for a change that was correct. So the shared
file list comes from the package, the retired paths come from the profile
contract, the lock's shape comes from the schema its own generator publishes,
and no hash is typed in here at all.
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

import jsonschema

VELA_ROOT = Path(__file__).resolve().parent.parent

# The wire identifier of a package-consumer reference. This is the name of the
# thing the dependency rule is about, not a pin: if it ever changes, the rule is
# about a different object and has to be rewritten, which is a louder failure
# than a stale hash and is meant to be.
CONSUMER_REFERENCE_SCHEMA = "vela.package-consumer-reference.v1"

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
# out — a Frontier CI job checks a slice of *this* repository out in there, and
# a linter that read it would report Vela's own files as a Frontier's copies.
PRUNED_DIRECTORIES = frozenset({"__pycache__", "node_modules", "venv"})
KEPT_DOT_DIRECTORIES = frozenset({".github", ".vela"})

# Anchors the retired-path list in the profile contract. A missing marker is a
# hard error rather than an empty rule: a check that silently stops checking is
# the failure mode this whole file exists to avoid.
RETIRED_PATHS_MARKER = "<!-- frontier-lint:retired-paths -->"
PROFILE_CONTRACT = "docs/FRONTIER_REPOSITORY_PROFILE.md"

# `uses:` as a YAML key at the start of a line or a sequence item. Workflows
# here use neither anchors nor block scalars around this key, so a line match is
# exact for them and cheap enough to stay exact; a `uses:` appearing inside a
# `run:` script would be a false positive and is the known limit of this form.
USES_LINE = re.compile(r"^\s*(?:-\s+)?uses:\s*(?P<ref>[^\s#]+)")
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")
DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")

# How much of a shared module has to reappear in one Frontier file before it is
# a copy rather than a coincidence. Calibrated against the real bytes, not
# guessed: the `scripts/write_sources_lock.py` that three Frontiers carried
# until today redefines 11 of `resolver.py`'s 20 top-level names (55%), while
# the highest-scoring innocent file in any Frontier today reaches 3 names and
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

    A Frontier script with a stale escape sequence has a problem, but not one
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

    `check` and `resolve` are English verbs and one Frontier already defines the
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


def lock_schema(package: SharedPackage) -> tuple[str, dict[str, Any]]:
    """The lock schema, resolved through the package's own name for it."""
    try:
        name = package.constants["LOCK_SCHEMA"]
    except KeyError as error:
        raise ConfigurationError(
            f"{package.name} no longer defines LOCK_SCHEMA; the generated-file rule "
            "cannot find the schema it validates against"
        ) from error
    matches = [path for path in package.root.rglob(name) if path.is_file()]
    if len(matches) != 1:
        raise ConfigurationError(f"{package.name} ships {len(matches)} copies of {name}")
    return name, json.loads(matches[0].read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# What one Frontier has
# ---------------------------------------------------------------------------


class Frontier:
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


def rule_shared_package_copy(frontier: Frontier, packages: list[SharedPackage]) -> list[Finding]:
    """A Frontier must not carry machinery that now lives in a shared package.

    Three independent signals, because the two copies removed today failed
    differently: three Frontiers held byte-identical files, and the fourth held
    a resolver of its own that shared only the entry point's name.
    """
    findings: list[Finding] = []
    for path in frontier.walk():
        relative = frontier.relative(path)
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


def rule_non_production_dependency(frontier: Frontier) -> list[Finding]:
    """No production dependency may resolve into a non-released tree.

    A consumer reference names the exact commit and path it binds. When that
    path opens on `research/` or `examples/`, the Frontier is depending on a
    candidate: nothing promises it will be there next release, and the package
    under it usually says so itself.
    """
    findings: list[Finding] = []
    needle = CONSUMER_REFERENCE_SCHEMA.encode("utf-8")
    for path in frontier.walk():
        relative = frontier.relative(path)
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


def rule_action_pinning(frontier: Frontier) -> list[Finding]:
    """Every third-party Action must be pinned to a full commit SHA.

    The pin is checked for shape only. What tag a SHA carried on the day it was
    written is upstream's to move and no business of a check that runs offline:
    a rule that compared the comment would have gone red this week for four
    repositories whose pins were, and are, correct.
    """
    findings: list[Finding] = []
    workflows = frontier.root / ".github"
    if not workflows.is_dir():
        return findings
    for path in sorted(workflows.rglob("*")):
        if not path.is_file() or path.suffix not in {".yml", ".yaml"}:
            continue
        relative = frontier.relative(path)
        for number, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1
        ):
            if line.lstrip().startswith("#"):
                continue
            match = USES_LINE.match(line)
            if not match:
                continue
            reference = match.group("ref").strip("'\"")
            if reference.startswith((".", "/")):
                continue
            if reference.startswith("docker://"):
                if "@" not in reference or not DIGEST.match(reference.rsplit("@", 1)[1]):
                    findings.append(
                        Finding(
                            "unpinned-action",
                            relative,
                            number,
                            f"{reference} is not pinned to an image digest",
                        )
                    )
                continue
            if "@" not in reference:
                findings.append(
                    Finding("unpinned-action", relative, number, f"{reference} carries no ref at all")
                )
                continue
            action, ref = reference.rsplit("@", 1)
            if not FULL_SHA.match(ref):
                findings.append(
                    Finding(
                        "unpinned-action",
                        relative,
                        number,
                        f"{action} is pinned to {ref!r}, which is not a 40-character commit SHA",
                    )
                )
    return findings


def rule_retired_paths(frontier: Frontier, retired: list[str]) -> list[Finding]:
    """Retired paths must be absent.

    A directory entry fires only when it holds a file, which is what a worktree
    walk sees and therefore what `vela replay` means by the same word. An empty
    directory left behind by a checkout is not a repository carrying a path.
    """
    findings: list[Finding] = []
    for entry in retired:
        target = frontier.file(entry.rstrip("/"))
        if entry.endswith("/"):
            if target.is_dir() and any(item.is_file() for item in target.rglob("*")):
                findings.append(
                    Finding("retired-path", entry, 0, "retired directory still holds files")
                )
        elif target.is_file():
            findings.append(Finding("retired-path", entry, 0, "retired path is still present"))
    return findings


def rule_generated_files(frontier: Frontier, packages: list[SharedPackage]) -> list[Finding]:
    """A generated file answers to its generator, in shape and by name.

    The lock is validated against the schema the generator publishes rather than
    against a restatement of it, so the two cannot drift; and the declaration it
    is derived from has to name the generator, so a reader who finds a wrong
    hash knows what to re-run instead of editing the lock to agree with itself.
    """
    findings: list[Finding] = []
    for package in packages:
        lock_name = package.constants.get("LOCK_FILE")
        declaration_name = package.constants.get("DECLARATION_FILE")
        if not lock_name or not declaration_name:
            continue
        lock_path = frontier.file(lock_name)
        if not lock_path.is_file():
            continue
        schema_name, schema = lock_schema(package)
        try:
            document = json.loads(lock_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            findings.append(Finding("generated-file", lock_name, 0, f"is not JSON: {error}"))
            continue
        validator_class = jsonschema.validators.validator_for(schema)
        validator_class.check_schema(schema)
        for error in sorted(
            validator_class(schema).iter_errors(document), key=lambda e: list(e.absolute_path)
        ):
            where = "/".join(str(part) for part in error.absolute_path)
            findings.append(
                Finding(
                    "generated-file",
                    f"{lock_name}{'/' + where if where else ''}",
                    0,
                    f"violates {schema_name} as published by {package.name}: {error.message}",
                )
            )
        declaration_path = frontier.file(declaration_name)
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
    "unpinned-action",
    "retired-path",
    "generated-file",
)


def lint(frontier_root: Path) -> list[Finding]:
    frontier = Frontier(frontier_root)
    packages = shared_packages()
    findings = [
        *rule_shared_package_copy(frontier, packages),
        *rule_non_production_dependency(frontier),
        *rule_action_pinning(frontier),
        *rule_retired_paths(frontier, retired_paths()),
        *rule_generated_files(frontier, packages),
    ]
    return sorted(findings)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="frontier_lint",
        description="Check one Frontier checkout against the shape consolidation left it in.",
    )
    parser.add_argument("frontier", nargs="?", default=".", help="path to one Frontier checkout")
    parser.add_argument("--json", action="store_true", help="emit findings as JSON")
    arguments = parser.parse_args(argv)

    root = Path(arguments.frontier)
    try:
        findings = lint(root)
    except ConfigurationError as error:
        print(f"frontier-lint: {error}", file=sys.stderr)
        return 2

    if arguments.json:
        print(
            json.dumps(
                {
                    "schema": "vela.frontier-lint.v1",
                    "frontier": str(root.resolve()),
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
            print(f"frontier-lint: {name}: {len(findings)} finding(s) across {len(RULES)} rules")
        else:
            print(f"frontier-lint: {name}: ok ({len(RULES)} rules)")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
