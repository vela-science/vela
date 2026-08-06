"""A Frontier root and a GitHub that only serves what a test says it serves.

No test here touches the network. That is not only about speed: the two failure
cases this suite exists for — a declared root that upstream contradicts, and a
source that cannot be reached — are both states you cannot ask a real server to
be in on demand.
"""

from __future__ import annotations

import hashlib
import json
import urllib.error
from pathlib import Path

import pytest
import yaml


def root_of(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


class FakeGitHub:
    """A fetcher over a fixed world.

    `blobs` maps a url to the bytes served there. `commits` maps `(repo, rev)` to
    the `(commit, tree)` the API reports. Anything not in either is unreachable,
    which is the same thing the resolver sees when a url has rotted.
    """

    def __init__(
        self,
        blobs: dict[str, bytes] | None = None,
        commits: dict[tuple[str, str], tuple[str, str]] | None = None,
    ) -> None:
        self.blobs = dict(blobs or {})
        self.commits = dict(commits or {})
        self.requested: list[str] = []

    def raw(self, repo: str, commit: str, path: str, data: bytes) -> None:
        self.blobs[f"https://raw.githubusercontent.com/{repo}/{commit}/{path}"] = data

    def __call__(self, url: str, headers=None) -> bytes:
        self.requested.append(url)
        if url.startswith("https://api.github.com/repos/"):
            rest = url[len("https://api.github.com/repos/") :]
            owner, name, _, rev = rest.split("/", 3)
            try:
                commit, tree = self.commits[(f"{owner}/{name}", rev)]
            except KeyError:
                raise urllib.error.HTTPError(url, 404, "Not Found", {}, None) from None
            return json.dumps({"sha": commit, "commit": {"tree": {"sha": tree}}}).encode()
        try:
            return self.blobs[url]
        except KeyError:
            raise urllib.error.URLError(f"no route to {url}") from None


@pytest.fixture
def frontier(tmp_path: Path):
    """Write a `sources.yaml` into a throwaway Frontier root."""

    def build(sources: dict, files: dict[str, bytes] | None = None) -> Path:
        (tmp_path / "sources.yaml").write_text(yaml.safe_dump({"sources": sources}))
        for name, data in (files or {}).items():
            target = tmp_path / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        return tmp_path

    return build


@pytest.fixture
def offline(monkeypatch):
    """Make any unmocked network call a loud failure rather than a slow one."""

    def refuse(url: str, headers=None) -> bytes:
        raise AssertionError(f"the resolver reached for {url}, and this test forbids it")

    monkeypatch.setattr("vela_source_manifest.resolver.urlopen_fetch", refuse)
    return refuse


def lock_of(root: Path) -> dict:
    return json.loads((root / "sources.lock.json").read_text())
