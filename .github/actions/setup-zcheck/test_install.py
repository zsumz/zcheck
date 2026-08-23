"""Adversarial tests for the exact-release zcheck installer."""

from __future__ import annotations

import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

import install


class ActionManifestTests(unittest.TestCase):
    def test_runner_python_commands_are_explicit(self) -> None:
        source = Path(__file__).with_name("action.yml").read_text(encoding="utf-8")
        unique = [
            "using: composite",
            "if: runner.os != 'Windows'",
            "if: runner.os == 'Windows'",
            'run: python3 "${{ github.action_path }}/install.py"',
            'run: python "${{ github.action_path }}/install.py"',
        ]
        for contract in unique:
            self.assertEqual(source.count(contract), 1)
        self.assertEqual(source.count("ZCHECK_SETUP_VERSION: ${{ inputs.version }}"), 2)
        self.assertEqual(source.count("ZCHECK_SETUP_SHA256: ${{ inputs.sha256 }}"), 2)
        self.assertEqual(source.count("required: true"), 2)
        self.assertNotIn("default:", source)


class ReleaseTests(unittest.TestCase):
    def test_supported_runner_matrix_is_explicit(self) -> None:
        expected = {
            ("Linux", "X64"): ("x86_64-unknown-linux-gnu", "tar.gz", "zcheck"),
            ("Linux", "ARM64"): ("aarch64-unknown-linux-gnu", "tar.gz", "zcheck"),
            ("macOS", "X64"): ("x86_64-apple-darwin", "tar.gz", "zcheck"),
            ("macOS", "ARM64"): ("aarch64-apple-darwin", "tar.gz", "zcheck"),
            ("Windows", "X64"): ("x86_64-pc-windows-msvc", "zip", "zcheck.exe"),
        }
        for platform, values in expected.items():
            selected = install.release("0.0.1", *platform)
            self.assertEqual((selected.target, selected.extension, selected.binary), values)
            self.assertIn("/v0.0.1/", selected.url)
            self.assertTrue(selected.archive.startswith("zcheck-0.0.1-"))

    def test_unsupported_or_ambiguous_inputs_fail_closed(self) -> None:
        for version in ["", "v0.0.1", "latest", "0.0.1+local", "../0.0.1"]:
            with self.assertRaises(ValueError):
                install.release(version, "Linux", "X64")
        for platform in [("Linux", "RISCV64"), ("Windows", "ARM64"), ("FreeBSD", "X64")]:
            with self.assertRaises(ValueError):
                install.release("0.0.1", *platform)
        for digest in ["", "0" * 63, "g" * 64, "0" * 65]:
            with self.assertRaises(ValueError):
                install.expected_digest(digest)


class ArchiveTests(unittest.TestCase):
    def test_tar_and_zip_extract_only_the_exact_binary(self) -> None:
        with tempfile.TemporaryDirectory(prefix="setup-zcheck-test-") as temporary:
            root = Path(temporary)
            for runner_os, extension, binary in [
                ("Linux", "tar.gz", "zcheck"),
                ("Windows", "zip", "zcheck.exe"),
            ]:
                selected = install.release("0.0.1", runner_os, "X64")
                self.assertEqual((selected.extension, selected.binary), (extension, binary))
                archive = root / selected.archive
                payload = f"binary-{runner_os}".encode()
                write_archive(archive, selected, payload)
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                output = install.install_archive(
                    archive, selected, digest, root / f"out-{runner_os}"
                )
                self.assertEqual(output.read_bytes(), payload)

    def test_checksum_mismatch_and_duplicate_binary_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="setup-zcheck-test-") as temporary:
            root = Path(temporary)
            selected = install.release("0.0.1", "Linux", "X64")
            archive = root / selected.archive
            write_archive(archive, selected, b"binary")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                install.install_archive(archive, selected, "0" * 64, root / "mismatch")
            with tarfile.open(archive, "w:gz") as package:
                add_tar_file(package, selected.binary, b"one")
                add_tar_file(package, selected.binary, b"two")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with self.assertRaisesRegex(ValueError, "one regular"):
                install.install_archive(archive, selected, digest, root / "duplicate")

    def test_expanded_binary_size_is_bounded(self) -> None:
        with tempfile.TemporaryDirectory(prefix="setup-zcheck-test-") as temporary:
            root = Path(temporary)
            selected = install.release("0.0.1", "Linux", "X64")
            archive = root / selected.archive
            write_archive(archive, selected, b"too-large")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with mock.patch.object(install, "MAX_ARCHIVE_BYTES", 8):
                with self.assertRaisesRegex(ValueError, "binary exceeds"):
                    install.install_archive(archive, selected, digest, root / "bounded")

    def test_installed_identity_must_match_exactly(self) -> None:
        binary = Path("zcheck")
        passing = mock.Mock(returncode=0, stdout="zcheck 0.0.1\n")
        with mock.patch.object(install.subprocess, "run", return_value=passing) as run:
            install.verify(binary, "0.0.1")
            run.assert_called_once_with(
                [binary, "--version"],
                capture_output=True,
                check=False,
                text=True,
                timeout=10,
            )
        failing = mock.Mock(returncode=0, stdout="zcheck 0.0.2\n")
        with mock.patch.object(install.subprocess, "run", return_value=failing):
            with self.assertRaisesRegex(ValueError, "identity mismatch"):
                install.verify(binary, "0.0.1")

    def test_action_main_installs_verifies_and_exports_only_the_binary_directory(self) -> None:
        with tempfile.TemporaryDirectory(prefix="setup-zcheck-test-") as temporary:
            root = Path(temporary)
            selected = install.release("0.0.1", "Linux", "X64")
            source = root / "source.tar.gz"
            write_archive(source, selected, b"binary")
            github_path = root / "github-path"
            github_path.touch()
            environment = {
                "ZCHECK_SETUP_VERSION": selected.version,
                "ZCHECK_SETUP_SHA256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "RUNNER_OS": "Linux",
                "RUNNER_ARCH": "X64",
                "RUNNER_TEMP": str(root),
                "GITHUB_PATH": str(github_path),
            }

            def local_download(url: str, destination: Path) -> None:
                self.assertEqual(url, selected.url)
                destination.write_bytes(source.read_bytes())

            with (
                mock.patch.dict(install.os.environ, environment, clear=True),
                mock.patch.object(install, "download", side_effect=local_download),
                mock.patch.object(install, "verify") as verify,
            ):
                self.assertEqual(install.main(), 0)
            binary = root / f"zcheck-{selected.version}" / "bin" / "zcheck"
            verify.assert_called_once_with(binary, selected.version)
            self.assertEqual(
                github_path.read_text(encoding="utf-8"), f"{binary.parent}\n"
            )


def write_archive(path: Path, selected: install.Release, payload: bytes) -> None:
    if selected.extension == "zip":
        with zipfile.ZipFile(path, "w") as package:
            package.writestr(selected.binary, payload)
    else:
        with tarfile.open(path, "w:gz") as package:
            add_tar_file(package, selected.binary, payload)


def add_tar_file(package: tarfile.TarFile, name: str, payload: bytes) -> None:
    entry = tarfile.TarInfo(name)
    entry.size = len(payload)
    entry.mode = 0o755
    package.addfile(entry, io.BytesIO(payload))


if __name__ == "__main__":
    unittest.main()
