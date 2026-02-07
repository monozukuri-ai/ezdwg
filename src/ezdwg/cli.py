from __future__ import annotations

import argparse
import sys
from collections import OrderedDict
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from typing import Sequence

from .document import SUPPORTED_ENTITY_TYPES, read


def _package_version() -> str:
    try:
        return version("ezdwg")
    except PackageNotFoundError:
        return "0.0.0"


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ezdwg", description="Inspect DWG files.")
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {_package_version()}",
    )
    subparsers = parser.add_subparsers(dest="command")

    inspect_parser = subparsers.add_parser("inspect", help="Show basic DWG information.")
    inspect_parser.add_argument("path", help="Path to DWG file.")
    return parser


def _run_inspect(path: str) -> int:
    file_path = Path(path)
    if not file_path.exists():
        print(f"error: file not found: {file_path}", file=sys.stderr)
        return 2

    try:
        doc = read(str(file_path))
    except Exception as exc:
        print(f"error: failed to read DWG: {exc}", file=sys.stderr)
        return 2
    modelspace = doc.modelspace()

    counts: OrderedDict[str, int] = OrderedDict()
    total = 0
    for dxftype in SUPPORTED_ENTITY_TYPES:
        count = sum(1 for _ in modelspace.query(dxftype))
        if count > 0:
            counts[dxftype] = count
            total += count

    print(f"file: {file_path}")
    print(f"version: {doc.version}")
    print(f"decode_version: {doc.decode_version}")
    print(f"total_entities: {total}")
    for dxftype, count in counts.items():
        print(f"{dxftype}: {count}")

    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.command == "inspect":
        return _run_inspect(args.path)

    parser.print_help()
    return 0
