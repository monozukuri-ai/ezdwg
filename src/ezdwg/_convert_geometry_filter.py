from __future__ import annotations

import math
import unicodedata
from typing import Any


def _ezdxf_entity_type(entity: Any) -> str:
    try:
        token = entity.dxftype()
    except Exception:
        try:
            token = entity.dxftype
        except Exception:
            return ""
        if callable(token):
            try:
                token = token()
            except Exception:
                return ""
    return str(token).strip().upper()


def _has_tiny_origin_arc_geometry(entity: Any) -> bool:
    dxf = getattr(entity, "dxf", None)
    if dxf is None:
        return False
    try:
        center = getattr(dxf, "center")
        radius = float(getattr(dxf, "radius"))
        center_x = float(getattr(center, "x", center[0]))
        center_y = float(getattr(center, "y", center[1]))
    except Exception:
        return False
    if not (
        math.isfinite(center_x)
        and math.isfinite(center_y)
        and math.isfinite(radius)
    ):
        return False
    if radius <= 1.0e-6:
        return True
    return max(abs(center_x), abs(center_y)) <= 10.0 and radius <= 1.0e-3


def _has_tiny_origin_3dface_geometry(entity: Any) -> bool:
    dxf = getattr(entity, "dxf", None)
    if dxf is None:
        return False
    values: list[float] = []
    for token in ("vtx0", "vtx1", "vtx2", "vtx3"):
        try:
            point = getattr(dxf, token)
            values.extend((float(point[0]), float(point[1]), float(point[2])))
        except Exception:
            return False
    finite_values = [abs(value) for value in values if math.isfinite(value)]
    if not finite_values:
        return False
    return max(finite_values) <= 1.0e-6


def _has_tiny_origin_ray_geometry(entity: Any) -> bool:
    dxf = getattr(entity, "dxf", None)
    if dxf is None:
        return False
    try:
        start = getattr(dxf, "start")
        start_x = float(getattr(start, "x", start[0]))
        start_y = float(getattr(start, "y", start[1]))
    except Exception:
        return False
    if not (math.isfinite(start_x) and math.isfinite(start_y)):
        return False
    return max(abs(start_x), abs(start_y)) <= 10.0


def _has_origin_anchor_far_lwpolyline_geometry(entity: Any) -> bool:
    if _ezdxf_entity_type(entity) != "LWPOLYLINE":
        return False
    try:
        points = [(float(point[0]), float(point[1])) for point in entity.get_points("xy")]
    except Exception:
        return False
    if len(points) < 2 or len(points) > 4:
        return False
    unique_points = {
        (round(point[0], 6), round(point[1], 6))
        for point in points
        if math.isfinite(point[0]) and math.isfinite(point[1])
    }
    if len(unique_points) > 3:
        return False
    has_origin_anchor = any(max(abs(x), abs(y)) <= 1.0 for x, y in unique_points)
    if not has_origin_anchor:
        return False
    has_far_point = any(max(abs(x), abs(y)) >= 10000.0 for x, y in unique_points)
    return has_far_point


def _is_implausible_repeated_block_primitive(entity: Any) -> bool:
    dxftype = _ezdxf_entity_type(entity)
    if dxftype == "LWPOLYLINE":
        return _has_origin_anchor_far_lwpolyline_geometry(entity)
    if dxftype == "RAY":
        return _has_tiny_origin_ray_geometry(entity)
    if dxftype == "ARC":
        return _has_tiny_origin_arc_geometry(entity)
    if dxftype == "3DFACE":
        return _has_tiny_origin_3dface_geometry(entity)
    return False


def _score_text_plausibility_char(ch: str) -> int:
    if ch == "\uFFFD" or ("\uE000" <= ch <= "\uF8FF"):
        return -6
    if unicodedata.category(ch).startswith("C") and ch not in {"\n", "\r", "\t"}:
        return -5
    if ch.isascii() and ch.isalnum():
        return 2
    if ch.isascii() and (ch in r""" !"#$%&'()*+,-./:;<=>?@[\]^_`{|}~""" or ch.isspace()):
        return 1
    if (
        "\u3000" <= ch <= "\u303F"
        or "\u3040" <= ch <= "\u309F"
        or "\u30A0" <= ch <= "\u30FF"
        or "\u3400" <= ch <= "\u4DBF"
        or "\u4E00" <= ch <= "\u9FFF"
        or "\uFF01" <= ch <= "\uFF60"
        or "\uFFE0" <= ch <= "\uFFE6"
    ):
        return 2
    if ch.isalpha() or ch.isnumeric() or ch.isspace():
        return 1
    return -2


def _is_plausible_text_content(text: str) -> bool:
    if text == "":
        return True
    if len(text) > 512:
        return False
    score = 0
    count = 0
    control_count = 0
    for ch in text:
        count += 1
        if unicodedata.category(ch).startswith("C") and ch not in {"\n", "\r", "\t"}:
            control_count += 1
        score += _score_text_plausibility_char(ch)
    if control_count >= 4:
        return False
    return score >= -(count // 2)


def _is_plausible_text_insert(insert: Any) -> bool:
    if not isinstance(insert, (list, tuple)) or len(insert) < 2:
        return False
    try:
        x = float(insert[0])
        y = float(insert[1])
    except Exception:
        return False
    if not (math.isfinite(x) and math.isfinite(y)):
        return False
    _MAX_COORD_ABS = 1.0e12
    return abs(x) <= _MAX_COORD_ABS and abs(y) <= _MAX_COORD_ABS
