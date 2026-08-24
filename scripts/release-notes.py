#!/usr/bin/env python3
"""Extract one exact version section from the reviewed changelog."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re

VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("changelog", type=Path)
    parser.add_argument("output", type=Path)
    return parser.parse_args()


def section(source: str, version: str) -> str:
    if VERSION.fullmatch(version) is None:
        raise SystemExit(f"invalid release version: {version!r}")
    heading = re.compile(rf"^## {re.escape(version)}(?: - .+)?$")
    lines = source.splitlines()
    starts = [index for index, line in enumerate(lines) if heading.fullmatch(line)]
    if len(starts) != 1:
        raise SystemExit(f"expected one changelog section for {version}, found {len(starts)}")
    start = starts[0] + 1
    end = next(
        (index for index in range(start, len(lines)) if lines[index].startswith("## ")),
        len(lines),
    )
    body = "\n".join(lines[start:end]).strip()
    if not body:
        raise SystemExit(f"changelog section for {version} is empty")
    return f"{body}\n"


def main() -> int:
    options = arguments()
    notes = section(options.changelog.read_text(encoding="utf-8"), options.version)
    options.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = options.output.with_name(f".{options.output.name}.tmp")
    try:
        temporary.write_text(notes, encoding="utf-8", newline="\n")
        os.replace(temporary, options.output)
    finally:
        temporary.unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
