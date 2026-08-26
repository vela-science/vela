#!/usr/bin/env python3
"""Run the installer against a manifest the manifest generator actually wrote.

`install.sh` reads a digest out of `<asset>.release-manifest.json` and compares
it to the bytes on disk, which is the step that ties a signature to an artifact
rather than to a document mentioning a filename. That parse was written against
a hand-typed sample and shipped broken: `release_manifest.py` emits
`"sha256:<hex>"` and the installer's extraction required bare hex, so it matched
nothing, and because a non-matching `sed` passes its input through untouched the
failure would have surfaced as "the asset does not match the manifest" — sending
a reader hunting for tampering that never happened. Every real install would
have been refused.

A second opinion about the format is what caused that, so this test does not
hold one. It runs the real generator, signs with a throwaway identity supplied
through the documented `VELA_ALLOWED_SIGNERS` hook, and runs the real
`install.sh` end to end over `file://`. No network, no `gh`, no secrets, and no
copy of either side's idea of what the other emits.
"""

from __future__ import annotations

import hashlib
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INSTALLER = ROOT / "install.sh"
GENERATOR = ROOT / "scripts" / "release_manifest.py"

SIGNER_IDENTITY = "release@vela.space"
SIGNATURE_NAMESPACE = "vela-release"


def platform_asset() -> str:
    """The asset name `install.sh` will choose on this machine.

    Derived the same way the installer derives it, so the test follows the
    script onto a new platform instead of pinning one and skipping elsewhere.
    """
    system = platform.system().lower()
    machine = platform.machine()
    if system == "darwin" and machine in {"arm64", "aarch64"}:
        return "vela-macos-aarch64.zip"
    if system == "linux" and machine == "x86_64":
        return "vela-linux-x86_64.tar.gz"
    raise unittest.SkipTest(f"install.sh publishes no bundle for {system}-{machine}")


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    hasher.update(path.read_bytes())
    return hasher.hexdigest()


class ProviderIndependentInstall(unittest.TestCase):
    """A release retained anywhere, verified with OpenSSH and a checksum."""

    @classmethod
    def setUpClass(cls) -> None:
        if shutil.which("ssh-keygen") is None:
            raise unittest.SkipTest("ssh-keygen is not available")
        cls.asset = platform_asset()
        cls._directory = tempfile.TemporaryDirectory()
        cls.root = Path(cls._directory.name)
        cls.release = cls.root / "release"
        cls.release.mkdir()

        stage = cls.root / "stage"
        stage.mkdir()
        binary = stage / "vela"
        binary.write_text('#!/bin/sh\necho "vela 0.0.0-conformance"\n')
        binary.chmod(0o755)

        archive = cls.release / cls.asset
        if cls.asset.endswith(".zip"):
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.write(binary, "vela")
        else:
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(binary, "vela")

        sbom = cls.release / f"{cls.asset}.spdx.json"
        sbom.write_text('{"spdxVersion":"SPDX-2.3"}\n')
        (cls.release / f"{cls.asset}.sha256").write_text(f"{digest(archive)}  {cls.asset}\n")
        notices = stage / "THIRD-PARTY-LICENSES.txt"
        notices.write_text("fixture third-party notices\n")
        license_input = cls.root / "notice-input"
        license_input.write_text("fixture notice input\n")

        manifest = cls.release / f"{cls.asset}.release-manifest.json"
        subprocess.run(
            [
                "python3", str(GENERATOR),
                "--out", str(manifest),
                "--schema", "vela.release-bundle-manifest.v1",
                "--version", "0.0.0",
                "--tag", "v0.0.0",
                "--toolchain-channel", "stable",
                "--rustc", "rustc 0.0.0",
                "--target-triple", "x86_64-unknown-linux-musl",
                "--build-command", "cargo auditable build --locked --release",
                "--source-date-epoch", "1786406400",
                "--binary-build-count", "2",
                "--archive-build-count", "2",
                "--cargo-auditable-version", "0.0.0",
                "--license-generator", "cargo-about",
                "--license-generator-version", "0.0.0",
                "--license-notices", str(notices),
                "--license-input", f"fixture={license_input}",
                "--sbom-tool", "syft",
                "--sbom-tool-version", "0.0.0",
                "--binary", str(binary),
                "--asset", f"archive={archive}",
                "--asset", f"sbom={sbom}",
            ],
            check=True, capture_output=True, cwd=cls.release,
        )
        cls.manifest = manifest
        cls.archive = archive

        # A throwaway distribution identity. `VELA_ALLOWED_SIGNERS` exists so the
        # trust root can be pinned out of band, which is exactly what a test
        # needs: the real private key is not here and must never be.
        key = cls.root / "distribution"
        subprocess.run(
            ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "conformance", "-f", str(key)],
            check=True, capture_output=True,
        )
        subprocess.run(
            ["ssh-keygen", "-Y", "sign", "-f", str(key), "-n", SIGNATURE_NAMESPACE, str(manifest)],
            check=True, capture_output=True,
        )
        public = (key.with_suffix(".pub")).read_text().strip()
        cls.signers = cls.root / "allowed_signers"
        cls.signers.write_text(
            f'{SIGNER_IDENTITY} namespaces="{SIGNATURE_NAMESPACE}" {public}\n'
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls._directory.cleanup()

    def install(self, release: Path) -> subprocess.CompletedProcess[str]:
        prefix = Path(tempfile.mkdtemp(dir=self.root))
        environment = dict(os.environ)
        environment.update(
            VELA_VERSION="v0.0.0",
            VELA_RELEASE_BASE_URL=f"file://{release}",
            VELA_ALLOWED_SIGNERS=str(self.signers),
            VELA_INSTALL_PREFIX=str(prefix),
        )
        return subprocess.run(
            ["bash", str(INSTALLER), "install"],
            capture_output=True, text=True, env=environment, check=False,
        )

    def copy_release(self) -> Path:
        target = Path(tempfile.mkdtemp(dir=self.root)) / "release"
        shutil.copytree(self.release, target)
        return target

    def test_a_signed_release_installs_without_github(self) -> None:
        result = self.install(self.release)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("signed release manifest (provider-independent)", result.stdout)
        # The whole point: nothing consulted the provider.
        self.assertNotIn("api.github.com", result.stdout + result.stderr)
        self.assertNotIn("attestation", result.stdout + result.stderr)

    def test_the_digest_is_read_out_of_the_manifest_the_generator_wrote(self) -> None:
        """The regression itself, stated as its own test.

        `release_manifest.py` writes `sha256:<hex>`. If the installer's parse
        stops accepting that form, this fails on the digest comparison and the
        message below says which side moved.
        """
        self.assertIn(f'"sha256": "sha256:{digest(self.archive)}"', self.manifest.read_text())
        result = self.install(self.release)
        self.assertNotIn("does not match the digest", result.stderr)
        self.assertNotIn("could not read a SHA-256", result.stderr)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_bytes_that_disagree_with_the_signed_manifest_are_refused(self) -> None:
        release = self.copy_release()
        archive = release / self.asset
        archive.write_bytes(archive.read_bytes() + b"tampered")
        # Restore the sidecar checksum, so the only surviving objection is the
        # signed manifest. Otherwise this would prove `shasum -c` works.
        (release / f"{self.asset}.sha256").write_text(f"{digest(archive)}  {self.asset}\n")
        result = self.install(release)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match the digest in the signed release manifest", result.stderr)

    def test_a_manifest_signed_by_another_key_is_refused(self) -> None:
        release = self.copy_release()
        impostor = self.root / "impostor"
        if not impostor.exists():
            subprocess.run(
                ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "impostor", "-f", str(impostor)],
                check=True, capture_output=True,
            )
        signature = release / f"{self.asset}.release-manifest.json.sig"
        signature.unlink()
        subprocess.run(
            ["ssh-keygen", "-Y", "sign", "-f", str(impostor), "-n", SIGNATURE_NAMESPACE,
             str(release / f"{self.asset}.release-manifest.json")],
            check=True, capture_output=True,
        )
        result = self.install(release)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not verify against the distribution identity", result.stderr)

    def test_an_unsigned_manifest_is_never_treated_as_provenance(self) -> None:
        """The pipeline publishes this state, so it must not be fatal — or trusted.

        `release.yml` requires the manifest before publishing and declines to
        sign it in CI, so a manifest with no signature beside it is a real
        release, not an attack. `VELA_REQUIRE_SIGNED_MANIFEST` is the way to
        demand the strong path; without it the installer falls back and says so,
        and it must never report the unsigned manifest as having verified
        anything.
        """
        release = self.copy_release()
        (release / f"{self.asset}.release-manifest.json.sig").unlink()
        environment = dict(os.environ)
        environment.update(
            VELA_VERSION="v0.0.0",
            VELA_RELEASE_BASE_URL=f"file://{release}",
            VELA_REQUIRE_SIGNED_MANIFEST="1",
            VELA_INSTALL_PREFIX=str(Path(tempfile.mkdtemp(dir=self.root))),
        )
        result = subprocess.run(
            ["bash", str(INSTALLER), "install"],
            capture_output=True, text=True, env=environment, check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("VELA_REQUIRE_SIGNED_MANIFEST", result.stderr)
        self.assertNotIn("provider-independent", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
