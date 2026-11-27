"""Linetype and text style table rows joined from the raw decoders (uncached: a path-keyed cache goes stale when a file is replaced)."""

from __future__ import annotations

import math

from . import raw


def linetype_rows(path: str) -> dict[int, dict]:
    """LTYPE handle -> row, the union of ``decode_linetypes`` and ``decode_write_tables``.

    A record missing from ``decode_linetypes`` has ``description``, ``pattern_length``
    and ``dashes`` set to ``None``; one missing from ``decode_write_tables`` has
    ``complex`` set to ``None`` and ``extended_data`` False.
    """
    flags = {
        r["handle"]: r for r in raw.decode_write_tables(path) if r["kind"] == "LTYPE"
    }
    # Same leniency as Document.linetypes: an undecodable table is an empty one.
    try:
        decoded = [
            (int(h), str(n), str(d), float(l), list(ds))
            for h, n, d, l, ds in raw.decode_linetypes(path)
        ]
    except Exception:
        decoded = []
    try:
        elements = {h: els for h, _name, els in raw.decode_linetype_elements(path)}
    except Exception:
        elements = {}
    rows: dict[int, dict] = {}
    for handle, name, description, length, dashes in decoded:
        rows.setdefault(
            handle,
            {
                "handle": handle,
                "name": name,
                "description": description,
                "pattern_length": length,
                "dashes": dashes,
            },
        )
    for handle, flag in flags.items():
        rows.setdefault(
            handle,
            {
                "handle": handle,
                "name": flag["name"],
                "description": None,
                "pattern_length": None,
                "dashes": None,
            },
        )
        rows[handle]["complex"] = bool(flag["complex"])
        rows[handle]["extended_data"] = bool(flag.get("extended_data", False))
    for handle, row in rows.items():
        row.setdefault("complex", None)
        row.setdefault("extended_data", False)
        row["elements"] = elements.get(handle)
    return rows


def text_style_rows(path: str) -> dict[int, dict]:
    """STYLE handle -> the ``decode_write_tables`` row, unchanged."""
    return {r["handle"]: r for r in raw.decode_write_tables(path) if r["kind"] == "STYLE"}


def text_style_font_xdata(path: str) -> dict[int, tuple[str, int]]:
    """STYLE handle -> ``(family, flags)`` from the ``ACAD`` EED block (codes 1000, 1071)."""
    styles = text_style_rows(path)
    fonts: dict[int, tuple[str, int]] = {}
    for handle, blocks in raw.decode_object_eed(path):
        if handle not in styles:
            continue
        for _app_handle, app_name, values in blocks:
            if app_name != "ACAD":
                continue
            family = next((v for c, v in values if c == 1000), None)
            if family is not None:
                fonts[handle] = (family, next((v for c, v in values if c == 1071), 0))
    return fonts


def clean_linetype_scale(value: object) -> float | int:
    """The value when it is a finite number above 0 (kept as given), else 1.0."""
    if isinstance(value, (int, float)) and math.isfinite(value) and value > 0:
        return value
    return 1.0
