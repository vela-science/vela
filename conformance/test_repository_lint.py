#!/usr/bin/env python3
"""Prove every `repository_lint` rule fires, and prove it against real bytes.

A linter whose rules have never failed is worse than no linter, because a green
run then means only that the rules ran. So each rule below is shown failing on
a Frontier built to break it and passing on the same Frontier with the break
removed — the pair, not either half.

Where a rule's subject exists in this repository, the fixture is built from it
rather than from a hand-written imitation: the copy fixture copies the shared
resolver as it is today, the export fixture reads the package's own `__all__`,
the retired-path fixture reads the profile contract. A fixture typed out here
would be a second opinion about what the rule is for, and would pass on the day
the real thing changed shape.
"""

from __future__ import annotations

import ast
import json
import shutil
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import repository_lint as lint  # noqa: E402


def rules(findings: list[lint.Finding]) -> set[str]:
    return {finding.rule for finding in findings}


class FrontierFixture:
    """One throwaway Frontier, built file by file.

    Written into a temporary directory with nothing beside it. A test that
    prepared a Frontier next to a `vela` checkout would pass while the linter
    silently resolved `../vela`, which is the mistake this whole design is
    arranged around.
    """

    def __init__(self, case: unittest.TestCase) -> None:
        self.root = Path(tempfile.mkdtemp(prefix="frontier-lint-")) / "some-frontier"
        self.root.mkdir()
        case.addCleanup(shutil.rmtree, self.root.parent, ignore_errors=True)

    def write(self, relative: str, content: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def lint(self) -> list[lint.Finding]:
        return lint.lint(self.root)


class SharedPackageCopy(unittest.TestCase):
    def setUp(self) -> None:
        self.packages = lint.shared_packages()
        self.package = self.packages[0]

    def test_an_empty_frontier_is_not_a_copy_of_anything(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("README.md", "# nothing here\n")
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_verbatim_copy_of_a_shared_module_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        largest = max(self.package.modules, key=lambda name: len(self.package.modules[name]))
        source = (self.package.root / largest).read_text(encoding="utf-8")
        frontier.write("scripts/write_sources_lock.py", source)
        findings = [f for f in frontier.lint() if f.rule == "shared-package-copy"]
        self.assertTrue(findings, "a byte-for-byte copy of the shared resolver went unreported")
        self.assertIn("byte-identical", " ".join(f.message for f in findings))

    def test_a_renamed_copy_is_still_a_finding(self) -> None:
        """The bytes differ, the module does not.

        A copy that gets its docstring rewritten on the way in defeats the
        digest, which is why the digest is not the only signal.
        """
        frontier = FrontierFixture(self)
        largest = max(self.package.modules, key=lambda name: len(self.package.modules[name]))
        source = (self.package.root / largest).read_text(encoding="utf-8")
        frontier.write("tools/sources.py", f'"""Our own copy, lightly edited."""\n\n{source}\n')
        findings = [f for f in frontier.lint() if f.rule == "shared-package-copy"]
        self.assertTrue(findings)
        self.assertTrue(
            any("redefines" in f.message for f in findings),
            f"expected an overlap finding, got {[f.message for f in findings]}",
        )

    def test_one_exported_name_reimplemented_is_a_finding(self) -> None:
        """The fourth Frontier's copy shared one name with the package and nothing else.

        A divergent reimplementation scores far too low on overlap to trip the
        copy threshold, so the exported entry point has to be its own signal.
        """
        frontier = FrontierFixture(self)
        exported = sorted(self.package.exported)[0]
        frontier.write("acquire.py", f"def {exported}(root):\n    return None\n")
        findings = [f for f in frontier.lint() if f.rule == "shared-package-copy"]
        self.assertTrue(findings, f"redefining {exported} went unreported")
        self.assertIn(exported, findings[0].message)

    def test_a_shared_name_that_is_an_ordinary_word_is_not_a_finding(self) -> None:
        """One Frontier defines `check` for its own reasons and is right to.

        The rule keeps only compound or capitalised names, so this stays quiet
        without anyone maintaining a list of words to forgive.
        """
        frontier = FrontierFixture(self)
        frontier.write("lemma.py", "def check(x):\n    return x\n")
        self.assertEqual(rules(frontier.lint()), set())

    def test_the_copy_threshold_sits_below_the_copy_it_was_cut_for(self) -> None:
        """Calibration, asserted rather than remembered.

        `COPY_MIN_SYMBOLS` and `COPY_MIN_FRACTION` were chosen from two measured
        numbers. If the shared package shrinks until the larger of them is out
        of reach, the rule has quietly stopped being able to fire.
        """
        largest = max(len(names) for names in self.package.modules.values())
        self.assertGreaterEqual(largest, lint.COPY_MIN_SYMBOLS)
        self.assertGreaterEqual(1.0, lint.COPY_MIN_FRACTION)
        self.assertTrue(self.package.exported, "the package exports nothing to detect a copy by")


class NonProductionDependency(unittest.TestCase):
    def reference(self, path: str) -> str:
        return json.dumps(
            {
                "schema": lint.CONSUMER_REFERENCE_SCHEMA,
                "package": {"id": "vela-science/some-contract", "version": "0.0.0-source-local"},
                "source": {
                    "repository": "https://github.com/vela-science/vela.git",
                    "commit": "0" * 40,
                    "path": path,
                },
            },
            indent=2,
        )

    def test_a_reference_into_a_released_tree_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("reproductions/x/contract.consumer.v1.json", self.reference("packages/thing"))
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_reference_into_research_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write(
            "reproductions/x/contract.consumer.v1.json", self.reference("research/some-contract")
        )
        findings = [f for f in frontier.lint() if f.rule == "non-production-dependency"]
        self.assertTrue(findings)
        self.assertIn("research/", findings[0].message)

    def test_every_quarantined_directory_fires(self) -> None:
        """Each name in the set earns its place by being checked, not by being listed."""
        for directory in sorted(lint.NON_PRODUCTION_DIRECTORIES):
            with self.subTest(directory=directory):
                frontier = FrontierFixture(self)
                frontier.write("r/contract.consumer.v1.json", self.reference(f"{directory}/thing"))
                self.assertEqual(
                    rules(frontier.lint()),
                    {"non-production-dependency"},
                    f"a dependency into {directory}/ went unreported",
                )

    def test_a_git_subdirectory_dependency_into_tests_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write(
            "pyproject.toml",
            textwrap.dedent(
                """\
                [project]
                name = "some-frontier"
                version = "0.1.0"

                [tool.uv.sources]
                helper = { git = "https://example.invalid/x", subdirectory = "tests/helper" }
                """
            ),
        )
        findings = [f for f in frontier.lint() if f.rule == "non-production-dependency"]
        self.assertTrue(findings)
        self.assertEqual(findings[0].line, 6)


class QualifiedCandidateDependency(unittest.TestCase):
    """The record that answers for a candidate dependency, held to all of it.

    Built from the records Vela actually retains, not from an imitation: these
    are the bytes the rule reads in a real run, so a shape change in them fails
    here rather than quietly widening what passes. Each test perturbs one field
    of a reference the record covers, because it is the conjunction — package,
    root, path, repository — that makes it the same dependency.
    """

    def setUp(self) -> None:
        self.candidates = lint.qualified_candidates()
        self.assertTrue(
            self.candidates,
            "no qualification record is retained, so the rule's one exit has stopped "
            "being reachable and every candidate dependency now reports",
        )
        self.candidate = self.candidates[0]
        self.consumer = sorted(self.candidate.consumers)[0]

    def reference(self, **overrides: object) -> str:
        document: dict[str, object] = {
            "schema": lint.CONSUMER_REFERENCE_SCHEMA,
            "package": {"id": self.candidate.package_id, "version": "0.0.0-source-local"},
            "source": {
                "repository": "https://github.com/vela-science/vela.git",
                "commit": "0" * 40,
                "path": self.candidate.source_path,
            },
            "package_root": self.candidate.root,
            "consumer": f"{self.consumer}/reproductions/x",
        }
        document.update(overrides)
        return json.dumps(document, indent=2)

    def fire(self, **overrides: object) -> set[str]:
        frontier = FrontierFixture(self)
        frontier.write("reproductions/x/contract.consumer.v1.json", self.reference(**overrides))
        return rules(frontier.lint())

    def test_a_recorded_candidate_dependency_is_quiet(self) -> None:
        self.assertEqual(self.fire(), set())

    def test_the_same_reference_from_a_repository_no_record_names_fires(self) -> None:
        self.assertEqual(
            self.fire(consumer="a-fifth-frontier/reproductions/x"),
            {"non-production-dependency"},
        )

    def test_a_root_that_has_drifted_from_the_record_fires(self) -> None:
        self.assertEqual(
            self.fire(package_root="sha256:" + "0" * 64), {"non-production-dependency"}
        )

    def test_a_reference_that_names_no_root_fires(self) -> None:
        frontier = FrontierFixture(self)
        document = json.loads(self.reference())
        del document["package_root"]
        frontier.write("reproductions/x/contract.consumer.v1.json", json.dumps(document, indent=2))
        self.assertEqual(rules(frontier.lint()), {"non-production-dependency"})

    def test_a_different_package_at_the_same_path_fires(self) -> None:
        self.assertEqual(
            self.fire(package={"id": "vela-science/something-else", "version": "0.0.0"}),
            {"non-production-dependency"},
        )

    def test_every_retained_record_qualifies_something_unreleased(self) -> None:
        """A record for a path the rule never inspects qualifies nothing and hides that."""
        for candidate in self.candidates:
            with self.subTest(package=candidate.package_id):
                head = candidate.source_path.split("/")[0]
                self.assertIn(head, lint.NON_PRODUCTION_DIRECTORIES)
                self.assertTrue(candidate.root)
                self.assertTrue(candidate.consumers)

    def test_every_named_qualification_schema_is_retained(self) -> None:
        """A schema named here but present nowhere is a rule that has stopped reading."""
        found = set()
        for path in (lint.VELA_ROOT / lint.QUALIFICATION_TREE).rglob("*.json"):
            try:
                document = json.loads(path.read_bytes())
            except (json.JSONDecodeError, UnicodeDecodeError):
                continue
            if isinstance(document, dict) and isinstance(document.get("schema"), str):
                found.add(document["schema"])
        self.assertEqual(lint.CANDIDATE_QUALIFICATION_SCHEMAS - found, set())


class GeneratorPin(unittest.TestCase):
    """The generator a Frontier locks its sources with, named at one commit.

    The package and its path come from the real `packages/` tree, so the day the
    shared package is renamed these fixtures follow it instead of testing a
    string nobody resolves any more.
    """

    REVISION = "73d278b0020b1699fcf80749104db19860d1bec2"
    OTHER = "0123456789abcdef0123456789abcdef01234567"

    def setUp(self) -> None:
        self.package = lint.shared_packages()[0]
        self.path = self.package.root.relative_to(lint.VELA_ROOT).as_posix()

    def invocation(self, revision: str) -> str:
        return (
            "# Regenerate the lock with:\n"
            f'#     uvx --from "git+https://github.com/vela-science/vela@{revision}'
            f'#subdirectory={self.path}" vela-source-lock\n'
            "sources: {}\n"
        )

    def test_a_declaration_pinning_one_full_commit_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("sources.yaml", self.invocation(self.REVISION))
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_branch_instead_of_a_commit_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("sources.yaml", self.invocation("main"))
        findings = [f for f in frontier.lint() if f.rule == "generator-pin"]
        self.assertTrue(findings, "an unpinned generator invocation went unreported")
        self.assertEqual(findings[0].line, 2)
        self.assertIn("40-character commit", findings[0].message)

    def test_a_short_commit_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("sources.yaml", self.invocation(self.REVISION[:12]))
        self.assertEqual(rules(frontier.lint()), {"generator-pin"})

    def test_prose_naming_the_package_path_is_not_a_dependency(self) -> None:
        """The paragraph that says where the generator lives resolves nothing."""
        frontier = FrontierFixture(self)
        frontier.write(
            "sources.yaml",
            self.invocation(self.REVISION)
            + f"# The generator is the shared package at `{self.path}` in the vela repository.\n",
        )
        self.assertEqual(rules(frontier.lint()), set())

    def test_two_revisions_in_one_frontier_are_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("sources.yaml", self.invocation(self.REVISION))
        frontier.write(
            "pyproject.toml",
            "[tool.uv.sources]\n"
            f'{self.package.name} = {{ git = "https://github.com/vela-science/vela", '
            f'rev = "{self.OTHER}", subdirectory = "{self.path}" }}\n',
        )
        findings = [f for f in frontier.lint() if f.rule == "generator-pin"]
        self.assertTrue(findings, "a Frontier naming two generator commits went unreported")
        self.assertIn("2 different commits", findings[0].message)

    def test_the_same_revision_restated_in_a_lock_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("sources.yaml", self.invocation(self.REVISION))
        frontier.write(
            "uv.lock",
            "[[package]]\n"
            f'source = {{ git = "https://github.com/vela-science/vela?subdirectory='
            f'{self.path.replace("/", "%2F")}&rev={self.REVISION}#{self.REVISION}" }}\n',
        )
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_frontier_that_does_not_use_the_generator_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("README.md", "# a Frontier with no source declaration\n")
        self.assertEqual(rules(frontier.lint()), set())


class RetiredPaths(unittest.TestCase):
    def setUp(self) -> None:
        self.retired = lint.retired_paths()

    def test_the_contract_still_declares_a_list(self) -> None:
        self.assertTrue(self.retired)

    def test_every_declared_retirement_fires(self) -> None:
        """Read from the contract, so a path added there is covered without an edit here."""
        for entry in self.retired:
            with self.subTest(entry=entry):
                frontier = FrontierFixture(self)
                target = entry.rstrip("/") + ("/kept.txt" if entry.endswith("/") else "")
                frontier.write(target, "x\n")
                self.assertEqual(
                    rules(frontier.lint()),
                    {"retired-path"},
                    f"a Frontier carrying {entry} went unreported",
                )

    def test_an_empty_retired_directory_is_not_a_finding(self) -> None:
        """Matching what a worktree walk sees, which is what `vela replay` means too."""
        directories = [entry for entry in self.retired if entry.endswith("/")]
        if not directories:
            self.skipTest("no directory is currently retired")
        frontier = FrontierFixture(self)
        (frontier.root / directories[0].rstrip("/")).mkdir(parents=True)
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_missing_marker_stops_the_run_rather_than_passing_it(self) -> None:
        original = lint.RETIRED_PATHS_MARKER
        lint.RETIRED_PATHS_MARKER = "<!-- frontier-lint:no-such-marker -->"
        self.addCleanup(setattr, lint, "RETIRED_PATHS_MARKER", original)
        with self.assertRaises(lint.ConfigurationError):
            lint.retired_paths()


class GeneratedFiles(unittest.TestCase):
    def setUp(self) -> None:
        self.package = lint.shared_packages()[0]
        self.lock_name = self.package.constants["LOCK_FILE"]
        self.declaration_name = self.package.constants["DECLARATION_FILE"]
        self.script = sorted(self.package.console_scripts)[0]

    def declaration(self) -> str:
        return f"# Regenerate the lock with `{self.script}`.\nsources:\n  thing:\n    kind: dataset\n"

    def valid_lock(self) -> dict[str, object]:
        return {
            "sources": {"thing": {"kind": "dataset", "sha256": "sha256:" + "a" * 64}},
        }

    def test_a_lock_its_generator_would_have_written_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write(self.declaration_name, self.declaration())
        frontier.write(self.lock_name, json.dumps(self.valid_lock(), indent=2))
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_lock_of_the_wrong_shape_is_left_to_its_generator(self) -> None:
        """A malformed root is silent here, and loud one step earlier.

        `vela-source-lock --check` validates this same document against the
        schema its own package publishes, and the action runs it before the
        linter. The rejection is asserted where the schema lives, in
        `packages/vela-source-manifest/tests/test_schemas.py`; asserted here is
        only that the linter no longer offers a second opinion on it, so a
        reader who expects a finding learns where the finding really comes from.
        """
        frontier = FrontierFixture(self)
        frontier.write(self.declaration_name, self.declaration())
        document = self.valid_lock()
        document["sources"]["thing"]["sha256"] = "32c4f405"
        frontier.write(self.lock_name, json.dumps(document, indent=2))
        self.assertEqual(rules(frontier.lint()), set())

    def test_a_lock_with_no_declaration_behind_it_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write(self.lock_name, json.dumps(self.valid_lock(), indent=2))
        self.assertEqual(rules(frontier.lint()), {"generated-file"})

    def test_a_declaration_that_never_names_its_generator_is_a_finding(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write(self.declaration_name, "sources:\n  thing:\n    kind: dataset\n")
        frontier.write(self.lock_name, json.dumps(self.valid_lock(), indent=2))
        findings = [f for f in frontier.lint() if f.rule == "generated-file"]
        self.assertTrue(findings)
        self.assertIn(self.script, findings[0].message)

    def test_a_frontier_with_no_lock_at_all_is_quiet(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("README.md", "# no sources here\n")
        self.assertEqual(rules(frontier.lint()), set())


class OneRepositoryOnly(unittest.TestCase):
    """The constraint that broke things twice today, asserted directly."""

    def test_a_sibling_named_vela_changes_nothing(self) -> None:
        frontier = FrontierFixture(self)
        frontier.write("README.md", "# a frontier\n")
        decoy = frontier.root.parent / "vela"
        (decoy / "docs").mkdir(parents=True)
        (decoy / "docs" / "REPOSITORY_PROFILE.md").write_text(
            f"{lint.RETIRED_PATHS_MARKER}\n```text\nREADME.md\n```\n", encoding="utf-8"
        )
        (decoy / "packages" / "decoy" / "src" / "decoy").mkdir(parents=True)
        (decoy / "packages" / "decoy" / "pyproject.toml").write_text(
            '[project]\nname = "decoy"\n', encoding="utf-8"
        )
        self.assertEqual(
            rules(frontier.lint()),
            set(),
            "the linter resolved a sibling `vela` instead of the checkout it ships in",
        )

    def test_a_frontier_cannot_be_pointed_outside_itself(self) -> None:
        frontier = FrontierFixture(self)
        with self.assertRaises(lint.ConfigurationError):
            lint.Frontier(frontier.root).file("../vela/Cargo.toml")

    def test_the_walk_stays_out_of_a_vendored_checkout(self) -> None:
        """Erdős CI puts a slice of `vela` inside the workspace during the run."""
        frontier = FrontierFixture(self)
        largest = max(
            lint.shared_packages()[0].modules,
            key=lambda name: len(lint.shared_packages()[0].modules[name]),
        )
        source = (lint.shared_packages()[0].root / largest).read_text(encoding="utf-8")
        frontier.write(".contract-source/vela/packages/x/src/x/resolver.py", source)
        self.assertEqual(rules(frontier.lint()), set())


class EveryRuleIsReachable(unittest.TestCase):
    def test_each_named_rule_has_a_test_that_makes_it_fire(self) -> None:
        """A rule listed in the output but absent from these tests is decoration."""
        source = Path(__file__).read_text(encoding="utf-8")
        asserted = {
            node.value
            for node in ast.walk(ast.parse(source))
            if isinstance(node, ast.Constant) and isinstance(node.value, str)
        }
        for rule in lint.RULES:
            with self.subTest(rule=rule):
                self.assertIn(rule, asserted, f"{rule} is reported but never provoked here")

    def test_the_published_table_names_exactly_the_rules_that_exist(self) -> None:
        """`conformance/README.md` is where a reader learns what this checks.

        It had no reader of its own and drifted in both directions at once: it
        described `unpinned-action`, deleted when zizmor took that job, and had
        never mentioned `generator-pin`. A table that names a rule nobody can
        provoke is worse than an absent table, because it reads as coverage.
        """
        readme = (lint.VELA_ROOT / "conformance" / "README.md").read_text(encoding="utf-8")
        # The one table under this heading, and only it — a second table added
        # to the file later is not this contract, and a check that swept up
        # every backticked cell in the document would go red for one.
        _, _, after = readme.partition("| Rule | Reads |\n")
        self.assertTrue(after, "conformance/README.md no longer carries the rule table")
        documented = {
            line.split("`")[1]
            for line in after.split("\n\n")[0].splitlines()
            if line.startswith("| `")
        }
        self.assertEqual(documented, set(lint.RULES))


if __name__ == "__main__":
    unittest.main(verbosity=2)
