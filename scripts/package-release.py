#!/usr/bin/env python3
"""Create one deterministic zcheck release archive."""

from __future__ import annotations

import argparse
import gzip
import os
from pathlib import Path
import tarfile
import zipfile

FIXED_ZIP_TIME = (1980, 1, 1, 0, 0, 0)


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--license", required=True, type=Path)
    parser.add_argument("--readme", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def members(options: argparse.Namespace) -> list[tuple[Path, str, int]]:
    binary_name = "zcheck.exe" if options.binary.suffix == ".exe" else "zcheck"
    values = [
        (options.binary, binary_name, 0o755),
        (options.license, "LICENSE", 0o644),
        (options.readme, "README.md", 0o644),
    ]
    for source, _, _ in values:
        if not source.is_file():
            raise SystemExit(f"release input is not a file: {source}")
    return values


def tar_archive(output: Path, values: list[tuple[Path, str, int]]) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                for source, name, mode in values:
                    info = tarfile.TarInfo(name)
                    info.size = source.stat().st_size
                    info.mode = mode
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    with source.open("rb") as contents:
                        archive.addfile(info, contents)


def zip_archive(output: Path, values: list[tuple[Path, str, int]]) -> None:
    with zipfile.ZipFile(
        output, mode="w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for source, name, mode in values:
            info = zipfile.ZipInfo(name, FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (0o100000 | mode) << 16
            archive.writestr(info, source.read_bytes(), compresslevel=9)


def verify(output: Path, expected: list[str], tar_format: bool) -> None:
    if tar_format:
        with tarfile.open(output, mode="r:gz") as archive:
            observed = archive.getnames()
    else:
        with zipfile.ZipFile(output) as archive:
            observed = archive.namelist()
    if observed != expected:
        raise SystemExit(f"unexpected archive members: {observed!r}")


def main() -> int:
    options = arguments()
    values = members(options)
    output = options.output
    tar_format = output.name.endswith(".tar.gz")
    if tar_format:
        writer = tar_archive
    elif output.suffix == ".zip":
        writer = zip_archive
    else:
        raise SystemExit("release output must end in .tar.gz or .zip")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    try:
        writer(temporary, values)
        verify(temporary, [name for _, name, _ in values], tar_format)
        os.replace(temporary, output)
    finally:
        temporary.unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
