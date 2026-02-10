from __future__ import annotations

import argparse
import sys
from collections import OrderedDict
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from typing import Sequence

from .convert import to_dxf
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

    convert_parser = subparsers.add_parser(
        "convert",
        help="Convert DWG to DXF using ezdxf as the writing backend.",
    )
    convert_parser.add_argument("input_path", help="Path to DWG file.")
    convert_parser.add_argument("output_path", help="Path to output DXF file.")
    convert_parser.add_argument(
        "--types",
        default=None,
        help='Entity filter passed to query(), e.g. "LINE ARC LWPOLYLINE".',
    )
    convert_parser.add_argument(
        "--dxf-version",
        default="R2010",
        help="DXF version for ezdxf.new(), e.g. R2000/R2010/R2018.",
    )
    convert_parser.add_argument(
        "--strict",
        action="store_true",
        help="Fail if any entity cannot be converted.",
    )
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
    for entity in modelspace.query():
        dxftype = entity.dxftype
        counts[dxftype] = counts.get(dxftype, 0) + 1
        total += 1

    print(f"file: {file_path}")
    print(f"version: {doc.version}")
    print(f"decode_version: {doc.decode_version}")
    print(f"total_entities: {total}")
    for dxftype in SUPPORTED_ENTITY_TYPES:
        count = counts.get(dxftype, 0)
        if count > 0:
            print(f"{dxftype}: {count}")

    return 0


def _run_convert(
    input_path: str,
    output_path: str,
    *,
    types: str | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
) -> int:
    dwg_path = Path(input_path)
    if not dwg_path.exists():
        print(f"error: file not found: {dwg_path}", file=sys.stderr)
        return 2

    try:
        result = to_dxf(
            str(dwg_path),
            output_path,
            types=types,
            dxf_version=dxf_version,
            strict=strict,
        )
    except Exception as exc:
        print(f"error: failed to convert DWG to DXF: {exc}", file=sys.stderr)
        return 2

    print(f"input: {result.source_path}")
    print(f"output: {result.output_path}")
    print(f"total_entities: {result.total_entities}")
    print(f"written_entities: {result.written_entities}")
    print(f"skipped_entities: {result.skipped_entities}")
    for dxftype, count in result.skipped_by_type.items():
        print(f"skipped[{dxftype}]: {count}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.command == "inspect":
        return _run_inspect(args.path)
    if args.command == "convert":
        return _run_convert(
            args.input_path,
            args.output_path,
            types=args.types,
            dxf_version=args.dxf_version,
            strict=bool(args.strict),
        )

    parser.print_help()
    return 0
