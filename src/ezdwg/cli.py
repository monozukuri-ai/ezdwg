from __future__ import annotations

import argparse
import sys
from collections import Counter, OrderedDict
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path
from typing import Sequence

from .convert import to_dwg, to_dxf
from .document import SUPPORTED_ENTITY_TYPES, read
from . import raw

_RECORD_DIAGNOSTIC_TYPES: tuple[str, ...] = ("LONG_TRANSACTION", "OLEFRAME", "OLE2FRAME")


def _is_unresolved_type_name(type_name: str) -> bool:
    name = type_name.strip().upper()
    return name == "UNKNOWN" or name.startswith("UNKNOWN(")


def _unknown_type_code_label(type_name: str, type_code: object) -> str | None:
    try:
        code = int(type_code)
    except Exception:
        code = 0
    if code > 0:
        return f"0x{code:X}"
    name = type_name.strip().upper()
    if name.startswith("UNKNOWN(0X") and name.endswith(")"):
        try:
            return f"0x{int(name[10:-1], 16):X}"
        except Exception:
            return None
    return None


def _header_type_hint(type_name: object, type_class: object, type_code: object) -> str:
    name = str(type_name).strip().upper()
    if not name:
        name = "UNKNOWN"
    if _is_unresolved_type_name(name):
        code_label = _unknown_type_code_label(name, type_code)
        if code_label is not None:
            name = f"UNKNOWN({code_label})"
    cls = str(type_class).strip().upper()
    if cls:
        return f"{name}/{cls}"
    return name


def _build_header_handle_hint_map(rows: list[tuple]) -> dict[int, str]:
    out: dict[int, str] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        try:
            handle = int(row[0])
        except Exception:
            continue
        if handle <= 0:
            continue
        out[handle] = _header_type_hint(row[4], row[5], row[3])
    return out


def _build_header_type_code_hint_map(rows: list[tuple]) -> dict[str, str]:
    counters_by_code: dict[str, Counter[str]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        try:
            type_code = int(row[3])
        except Exception:
            continue
        if type_code <= 0:
            continue
        code_label = f"0x{type_code:X}"
        hint = _header_type_hint(row[4], row[5], row[3])
        counters_by_code.setdefault(code_label, Counter())[hint] += 1
    out: dict[str, str] = {}
    for code_label, hint_counter in counters_by_code.items():
        if not hint_counter:
            continue
        out[code_label] = hint_counter.most_common(1)[0][0]
    return out


def _package_version() -> str:
    try:
        return version("ezdwg")
    except PackageNotFoundError:
        return "0.0.0"


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ezdwg", description="Inspect, convert, and write DWG files.")
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {_package_version()}",
    )
    subparsers = parser.add_subparsers(dest="command")

    inspect_parser = subparsers.add_parser("inspect", help="Show basic DWG information.")
    inspect_parser.add_argument("path", help="Path to DWG file.")
    inspect_parser.add_argument(
        "--verbose",
        action="store_true",
        help="Show expanded diagnostics (e.g. more unknown handle/type-code entries).",
    )

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
    convert_parser.add_argument(
        "--include-unsupported",
        action="store_true",
        help="Also query unsupported entity types (keeps legacy skip reporting behavior).",
    )
    convert_parser.add_argument(
        "--no-colors",
        action="store_true",
        help="Skip DWG color resolution for faster conversion.",
    )
    convert_parser.add_argument(
        "--modelspace-only",
        action="store_true",
        help="Limit export to entities directly owned by *MODEL_SPACE.",
    )
    convert_parser.add_argument(
        "--native-dimensions",
        action="store_true",
        help="Keep DIMENSION entities instead of exploding them to primitive geometry (default).",
    )
    convert_parser.add_argument(
        "--explode-dimensions",
        action="store_true",
        help="Explode DIMENSION entities to primitive geometry (legacy behavior).",
    )
    convert_parser.add_argument(
        "--flatten-inserts",
        action="store_true",
        help="Explode INSERT/MINSERT references into primitive geometry in modelspace.",
    )
    convert_parser.add_argument(
        "--dim-block-policy",
        choices=("smart", "legacy"),
        default="smart",
        help=(
            "Policy for anonymous dimension block (*D...) INSERT handling: "
            "smart (default) suppresses only references confirmed as duplicates by successful "
            "DIMENSION conversion, legacy restores geometric suppression heuristics."
        ),
    )

    write_parser = subparsers.add_parser(
        "write",
        help="Write a DWG file using the native AC1015 writer.",
    )
    write_parser.add_argument("input_path", help="Path to input DWG file.")
    write_parser.add_argument("output_path", help="Path to output DWG file.")
    write_parser.add_argument(
        "--types",
        default=None,
        help='Entity filter passed to query(), e.g. "LINE ARC LWPOLYLINE".',
    )
    write_parser.add_argument(
        "--dwg-version",
        default="AC1015",
        help="Output DWG version (currently AC1015 only).",
    )
    write_parser.add_argument(
        "--strict",
        action="store_true",
        help="Fail if any entity cannot be written.",
    )
    return parser


def _run_inspect(path: str, *, verbose: bool = False) -> int:
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
    record_diag_stats: dict[str, dict[str, int]] = {}
    record_diag_unknown_handles: dict[str, Counter[int]] = {}
    record_diag_unknown_type_codes: dict[str, Counter[str]] = {}
    total = 0
    for entity in modelspace.query():
        dxftype = entity.dxftype
        counts[dxftype] = counts.get(dxftype, 0) + 1
        total += 1
        if dxftype not in _RECORD_DIAGNOSTIC_TYPES:
            continue
        stats = record_diag_stats.setdefault(
            dxftype,
            {
                "entities": 0,
                "record_bytes": 0,
                "ascii": 0,
                "likely_refs": 0,
                "unresolved_likely_refs": 0,
                "decoded_refs": 0,
                "unresolved_decoded_refs": 0,
            },
        )
        unknown_handle_counter = record_diag_unknown_handles.setdefault(dxftype, Counter())
        unknown_type_code_counter = record_diag_unknown_type_codes.setdefault(dxftype, Counter())
        stats["entities"] += 1
        record_size = entity.dxf.get("record_size")
        if isinstance(record_size, int):
            stats["record_bytes"] += 1
        if entity.dxf.get("ascii_preview"):
            stats["ascii"] += 1
        likely_ref_details = entity.dxf.get("likely_handle_ref_details")
        if isinstance(likely_ref_details, list):
            stats["likely_refs"] += len(likely_ref_details)
            for item in likely_ref_details:
                if not isinstance(item, dict):
                    continue
                if _is_unresolved_type_name(str(item.get("type_name") or "")):
                    stats["unresolved_likely_refs"] += 1
                    unknown_type_code = _unknown_type_code_label(
                        str(item.get("type_name") or ""),
                        item.get("type_code"),
                    )
                    if unknown_type_code:
                        unknown_type_code_counter[unknown_type_code] += 1
                    try:
                        unknown_handle = int(item.get("handle"))
                    except Exception:
                        unknown_handle = None
                    if isinstance(unknown_handle, int) and unknown_handle > 0:
                        unknown_handle_counter[unknown_handle] += 1
        else:
            likely_refs = entity.dxf.get("likely_handle_refs")
            stats["likely_refs"] += len(list(likely_refs or []))
        if dxftype != "LONG_TRANSACTION":
            continue
        decoded_ref_details = entity.dxf.get("decoded_handle_ref_details")
        if isinstance(decoded_ref_details, list):
            stats["decoded_refs"] += len(decoded_ref_details)
            for item in decoded_ref_details:
                if not isinstance(item, dict):
                    continue
                if _is_unresolved_type_name(str(item.get("type_name") or "")):
                    stats["unresolved_decoded_refs"] += 1
                    unknown_type_code = _unknown_type_code_label(
                        str(item.get("type_name") or ""),
                        item.get("type_code"),
                    )
                    if unknown_type_code:
                        unknown_type_code_counter[unknown_type_code] += 1
                    try:
                        unknown_handle = int(item.get("handle"))
                    except Exception:
                        unknown_handle = None
                    if isinstance(unknown_handle, int) and unknown_handle > 0:
                        unknown_handle_counter[unknown_handle] += 1
        else:
            decoded_refs = entity.dxf.get("decoded_handle_refs")
            stats["decoded_refs"] += len(list(decoded_refs or []))

    print(f"file: {file_path}")
    print(f"version: {doc.version}")
    print(f"decode_version: {doc.decode_version}")
    print(f"total_entities: {total}")
    for dxftype in SUPPORTED_ENTITY_TYPES:
        count = counts.get(dxftype, 0)
        if count > 0:
            print(f"{dxftype}: {count}")

    try:
        header_rows = raw.list_object_headers_with_type(str(file_path))
    except Exception:
        header_rows = []
    header_handle_hints = _build_header_handle_hint_map(header_rows)
    header_type_code_hints = _build_header_type_code_hint_map(header_rows)
    if header_rows:
        raw_entity_counts: Counter[str] = Counter(
            type_name
            for _, _, _, _, type_name, type_class in header_rows
            if type_class == "E"
        )
        if raw_entity_counts:
            print(f"raw_entity_headers: {sum(raw_entity_counts.values())}")
            for dxftype in SUPPORTED_ENTITY_TYPES:
                gap = raw_entity_counts.get(dxftype, 0) - counts.get(dxftype, 0)
                if gap > 0:
                    print(f"decode_gap[{dxftype}]: {gap}")
            for dxftype, count in sorted(raw_entity_counts.items()):
                if dxftype in SUPPORTED_ENTITY_TYPES:
                    continue
                print(f"raw_only[{dxftype}]: {count}")

    for dxftype in _RECORD_DIAGNOSTIC_TYPES:
        stats = record_diag_stats.get(dxftype)
        if not stats:
            continue
        line = (
            f"record_diag[{dxftype}]: entities={stats['entities']} "
            f"record_bytes={stats['record_bytes']} ascii={stats['ascii']} "
            f"likely_refs={stats['likely_refs']} "
            f"unresolved_likely_refs={stats['unresolved_likely_refs']}"
        )
        if dxftype == "LONG_TRANSACTION":
            line = (
                f"{line} decoded_refs={stats['decoded_refs']} "
                f"unresolved_decoded_refs={stats['unresolved_decoded_refs']}"
            )
        print(line)
        top_n = 10 if verbose else 3
        unknown_handles = record_diag_unknown_handles.get(dxftype, Counter())
        if unknown_handles:
            top_handles = ", ".join(
                f"{handle}:{count}({header_handle_hints.get(handle, 'missing')})"
                for handle, count in unknown_handles.most_common(top_n)
            )
            print(f"record_diag_unknown_handles[{dxftype}]: {top_handles}")
        unknown_type_codes = record_diag_unknown_type_codes.get(dxftype, Counter())
        if unknown_type_codes:
            top_codes = ", ".join(
                f"{type_code}:{count}({header_type_code_hints.get(type_code, 'unmapped')})"
                for type_code, count in unknown_type_codes.most_common(top_n)
            )
            print(f"record_diag_unknown_type_codes[{dxftype}]: {top_codes}")

    return 0


def _run_convert(
    input_path: str,
    output_path: str,
    *,
    types: str | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
    include_unsupported: bool = False,
    preserve_colors: bool = True,
    modelspace_only: bool = False,
    explode_dimensions: bool = False,
    flatten_inserts: bool = False,
    dim_block_policy: str = "smart",
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
            include_unsupported=include_unsupported,
            preserve_colors=preserve_colors,
            modelspace_only=modelspace_only,
            explode_dimensions=explode_dimensions,
            flatten_inserts=flatten_inserts,
            dim_block_policy=dim_block_policy,
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


def _run_write(
    input_path: str,
    output_path: str,
    *,
    types: str | None = None,
    dwg_version: str = "AC1015",
    strict: bool = False,
) -> int:
    dwg_path = Path(input_path)
    if not dwg_path.exists():
        print(f"error: file not found: {dwg_path}", file=sys.stderr)
        return 2

    try:
        result = to_dwg(
            str(dwg_path),
            output_path,
            types=types,
            version=dwg_version,
            strict=strict,
        )
    except Exception as exc:
        print(f"error: failed to write DWG: {exc}", file=sys.stderr)
        return 2

    print(f"input: {result.source_path}")
    print(f"output: {result.output_path}")
    print(f"target_version: {result.target_version}")
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
        return _run_inspect(args.path, verbose=bool(args.verbose))
    if args.command == "convert":
        explode_dimensions = bool(args.explode_dimensions)
        if bool(args.native_dimensions):
            explode_dimensions = False
        return _run_convert(
            args.input_path,
            args.output_path,
            types=args.types,
            dxf_version=args.dxf_version,
            strict=bool(args.strict),
            include_unsupported=bool(args.include_unsupported),
            preserve_colors=not bool(args.no_colors),
            modelspace_only=bool(args.modelspace_only),
            explode_dimensions=explode_dimensions,
            flatten_inserts=bool(args.flatten_inserts),
            dim_block_policy=str(args.dim_block_policy),
        )
    if args.command == "write":
        return _run_write(
            args.input_path,
            args.output_path,
            types=args.types,
            dwg_version=args.dwg_version,
            strict=bool(args.strict),
        )

    parser.print_help()
    return 0
