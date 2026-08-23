#!/usr/bin/env python3
"""Install one exact, caller-digest-pinned zcheck release archive."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import hmac
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from typing import BinaryIO

REPOSITORY = "zsumz/zcheck"
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$")
DIGEST = re.compile(r"^[0-9a-fA-F]{64}$")


@dataclass(frozen=True)
class Release:
    version: str
    target: str
    extension: str
    binary: str

    @property
    def archive(self) -> str:
        return f"zcheck-{self.version}-{self.target}.{self.extension}"

    @property
    def url(self) -> str:
        return f"https://github.com/{REPOSITORY}/releases/download/v{self.version}/{self.archive}"


def release(version: str, runner_os: str, runner_arch: str) -> Release:
    if VERSION.fullmatch(version) is None:
        raise ValueError("version must be an exact zcheck version without a leading v")
    platforms = {
        ("Linux", "X64"): ("x86_64-unknown-linux-gnu", "tar.gz", "zcheck"),
        ("Linux", "ARM64"): ("aarch64-unknown-linux-gnu", "tar.gz", "zcheck"),
        ("macOS", "X64"): ("x86_64-apple-darwin", "tar.gz", "zcheck"),
        ("macOS", "ARM64"): ("aarch64-apple-darwin", "tar.gz", "zcheck"),
        ("Windows", "X64"): ("x86_64-pc-windows-msvc", "zip", "zcheck.exe"),
    }
    selected = platforms.get((runner_os, runner_arch))
    if selected is None:
        raise ValueError(f"unsupported zcheck release platform: {runner_os}/{runner_arch}")
    target, extension, binary = selected
    return Release(version, target, extension, binary)


def expected_digest(value: str) -> str:
    if DIGEST.fullmatch(value) is None:
        raise ValueError("sha256 must contain exactly 64 hexadecimal characters")
    return value.lower()


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "setup-zcheck/0.0.1"})
    with urllib.request.urlopen(request, timeout=60) as response:  # noqa: S310
        length = response.headers.get("Content-Length")
        if length is not None and int(length) > MAX_ARCHIVE_BYTES:
            raise ValueError("zcheck release archive exceeds the 64 MiB limit")
        total = 0
        with destination.open("xb") as output:
            while chunk := response.read(64 * 1024):
                total += len(chunk)
                if total > MAX_ARCHIVE_BYTES:
                    raise ValueError("zcheck release archive exceeds the 64 MiB limit")
                output.write(chunk)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(64 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def extract(archive: Path, selected: Release, destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
    output = destination / selected.binary
    temporary = destination / f".{selected.binary}.tmp"
    temporary.unlink(missing_ok=True)
    try:
        extract_to(archive, selected, temporary)
    except (OSError, ValueError, tarfile.TarError, zipfile.BadZipFile) as error:
        temporary.unlink(missing_ok=True)
        raise ValueError(f"cannot extract verified zcheck release: {error}") from error
    if temporary.stat().st_size == 0:
        temporary.unlink(missing_ok=True)
        raise ValueError("zcheck release binary has an invalid size")
    if selected.binary != "zcheck.exe":
        temporary.chmod(temporary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    temporary.replace(output)
    return output


def extract_to(archive: Path, selected: Release, temporary: Path) -> None:
    if selected.extension == "zip":
        with zipfile.ZipFile(archive) as source:
            entries = [entry for entry in source.infolist() if entry.filename == selected.binary]
            if len(entries) != 1 or entries[0].is_dir():
                raise ValueError(f"release archive must contain one regular {selected.binary}")
            mode = entries[0].external_attr >> 16
            if stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                raise ValueError(f"release archive must contain one regular {selected.binary}")
            with source.open(entries[0]) as reader, temporary.open("xb") as writer:
                copy_bounded(reader, writer)
    else:
        with tarfile.open(archive, mode="r:gz") as source:
            entries = [entry for entry in source.getmembers() if entry.name == selected.binary]
            if len(entries) != 1 or not entries[0].isfile():
                raise ValueError(f"release archive must contain one regular {selected.binary}")
            reader = source.extractfile(entries[0])
            if reader is None:
                raise ValueError(f"cannot read {selected.binary} from release archive")
            with reader, temporary.open("xb") as writer:
                copy_bounded(reader, writer)


def copy_bounded(reader: BinaryIO, writer: BinaryIO) -> None:
    total = 0
    while chunk := reader.read(64 * 1024):
        total += len(chunk)
        if total > MAX_ARCHIVE_BYTES:
            raise ValueError("zcheck release binary exceeds the 64 MiB limit")
        writer.write(chunk)


def install_archive(
    archive: Path, selected: Release, digest: str, destination: Path
) -> Path:
    observed = sha256(archive)
    if not hmac.compare_digest(observed, expected_digest(digest)):
        raise ValueError(f"zcheck release checksum mismatch: expected {digest}, observed {observed}")
    return extract(archive, selected, destination)


def verify(binary: Path, version: str) -> None:
    completed = subprocess.run(
        [binary, "--version"], capture_output=True, check=False, text=True, timeout=10
    )
    observed = completed.stdout.strip()
    if completed.returncode != 0 or observed != f"zcheck {version}":
        raise ValueError(
            f"installed zcheck identity mismatch: expected 'zcheck {version}', observed {observed!r}"
        )


def required_environment(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        raise ValueError(f"required environment variable {name} is missing")
    return value


def main() -> int:
    try:
        version = required_environment("ZCHECK_SETUP_VERSION")
        digest = required_environment("ZCHECK_SETUP_SHA256")
        selected = release(
            version,
            required_environment("RUNNER_OS"),
            required_environment("RUNNER_ARCH"),
        )
        runner_temp = Path(required_environment("RUNNER_TEMP"))
        destination = runner_temp / f"zcheck-{version}" / "bin"
        if destination.exists() or destination.is_symlink():
            raise ValueError(f"zcheck installation destination already exists: {destination}")
        with tempfile.TemporaryDirectory(prefix="setup-zcheck-", dir=runner_temp) as temporary:
            archive = Path(temporary) / selected.archive
            download(selected.url, archive)
            binary = install_archive(archive, selected, digest, destination)
        verify(binary, version)
        github_path = Path(required_environment("GITHUB_PATH"))
        with github_path.open("a", encoding="utf-8", newline="\n") as path_file:
            path_file.write(f"{destination}\n")
        print(f"installed zcheck {version} for {selected.target}")
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"setup-zcheck: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
