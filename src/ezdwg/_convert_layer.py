from __future__ import annotations

from typing import Any

from . import raw
from ._convert_utils import _to_valid_aci, _to_valid_true_color

_INVALID_DXF_LAYER_NAME_CHARS = frozenset('<>/\\":;?*|=')


def _layer_styles_by_handle(decode_path: str | None) -> dict[int, tuple[int, int | None]]:
    if not decode_path:
        return {}
    try:
        rows = raw.decode_layer_colors(decode_path)
    except Exception:
        return {}
    styles: dict[int, tuple[int, int | None]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 3:
            continue
        try:
            handle = int(row[0])
            index = int(row[1])
        except Exception:
            continue
        true_color = row[2]
        try:
            true_color_int = int(true_color) if true_color is not None else None
        except Exception:
            true_color_int = None
        styles[handle] = (index, true_color_int)
    return styles


def _layer_names_by_handle(decode_path: str | None) -> dict[int, str]:
    if not decode_path:
        return {}
    try:
        rows = raw.decode_layer_names(decode_path)
    except Exception:
        return {}
    names: dict[int, str] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 2:
            continue
        try:
            handle = int(row[0])
        except Exception:
            continue
        name = str(row[1]).strip()
        if not name:
            continue
        names[handle] = name
    return names


def _is_valid_dxf_layer_name(name: str) -> bool:
    candidate = str(name).strip()
    if not candidate:
        return False
    return not any(
        ord(ch) < 32 or ch in _INVALID_DXF_LAYER_NAME_CHARS for ch in candidate
    )


def _prepare_dxf_layers(
    dxf_doc: Any,
    layer_styles_by_handle: dict[int, tuple[int, int | None]],
    layer_names_by_handle: dict[int, str] | None = None,
) -> dict[int, str]:
    mapping: dict[int, str] = {0: "0"}
    for handle in sorted(layer_styles_by_handle):
        if handle <= 0:
            continue
        fallback_name = f"LAYER_{handle:X}"
        name = fallback_name
        if isinstance(layer_names_by_handle, dict):
            candidate = layer_names_by_handle.get(handle)
            if isinstance(candidate, str) and _is_valid_dxf_layer_name(candidate):
                name = candidate.strip()
        dxfattribs: dict[str, Any] = {}
        style = layer_styles_by_handle.get(handle)
        if style is not None:
            index, true_color = style
            color = _to_valid_aci(index)
            if color is not None:
                dxfattribs["color"] = color
            resolved_true = _to_valid_true_color(true_color)
            if resolved_true is not None:
                dxfattribs["true_color"] = resolved_true
        if name not in dxf_doc.layers:
            try:
                dxf_doc.layers.new(name=name, dxfattribs=dxfattribs or None)
            except Exception:
                name = fallback_name
                if name not in dxf_doc.layers:
                    try:
                        dxf_doc.layers.new(name=name, dxfattribs=dxfattribs or None)
                    except Exception:
                        continue
        mapping[handle] = name
    return mapping
