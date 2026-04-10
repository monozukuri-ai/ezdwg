from __future__ import annotations

import math
from typing import Any

_MAX_COORD_ABS = 1.0e12
_DIM_BLOCK_POLICIES = {"smart", "legacy"}


def _to_valid_aci(value: Any) -> int | None:
    try:
        aci = int(value)
    except Exception:
        return None
    if aci in (0, 256, 257):
        return None
    if 1 <= aci <= 255:
        return aci
    return None


def _to_valid_true_color(value: Any) -> int | None:
    try:
        color = int(value) & 0xFFFFFF
    except Exception:
        return None
    return color


def _to_rgb(true_color: int | None) -> tuple[int, int, int] | None:
    if true_color is None:
        return None
    return (
        (true_color >> 16) & 0xFF,
        (true_color >> 8) & 0xFF,
        true_color & 0xFF,
    )


def _validate_coord(value: Any) -> float:
    coord = float(value)
    if not math.isfinite(coord):
        raise ValueError(f"invalid coordinate value: {value!r}")
    if abs(coord) > _MAX_COORD_ABS:
        raise ValueError(f"coordinate out of supported range: {coord!r}")
    return coord


def _finite_float(value: Any, default: float) -> float:
    try:
        parsed = float(value)
    except Exception:
        return float(default)
    if not math.isfinite(parsed):
        return float(default)
    if abs(parsed) > _MAX_COORD_ABS:
        return float(default)
    return parsed


def _distinct_xy_count(points: list[tuple[float, float, float]]) -> int:
    return len({(float(point[0]), float(point[1])) for point in points})


def _normalize_dim_block_policy(policy: str) -> str:
    token = str(policy or "").strip().lower()
    if token in {"", "smart", "auto", "default"}:
        return "smart"
    if token in _DIM_BLOCK_POLICIES:
        return token
    allowed = ", ".join(sorted(_DIM_BLOCK_POLICIES))
    raise ValueError(f"unsupported dim-block policy: {policy!r} (expected one of: {allowed})")


def _quantize_dim_block_value(value: float) -> float:
    return round(float(value), 9)


def _normalized_angle_degrees(value: float) -> float:
    normalized = float(value) % 360.0
    if abs(normalized) <= 1.0e-9:
        return 0.0
    if abs(normalized - 360.0) <= 1.0e-9:
        return 0.0
    return normalized


def _point3(value: Any) -> tuple[float, float, float]:
    if value is None:
        return (0.0, 0.0, 0.0)
    if isinstance(value, (list, tuple)):
        if len(value) >= 3:
            return (
                _validate_coord(value[0]),
                _validate_coord(value[1]),
                _validate_coord(value[2]),
            )
        if len(value) >= 2:
            return (
                _validate_coord(value[0]),
                _validate_coord(value[1]),
                0.0,
            )
    raise ValueError(f"invalid point value: {value!r}")


def _point2(value: Any) -> tuple[float, float]:
    if value is None:
        raise ValueError("invalid point value: None")
    if isinstance(value, (list, tuple)):
        if len(value) >= 2:
            return (_validate_coord(value[0]), _validate_coord(value[1]))
    raise ValueError(f"invalid point value: {value!r}")


def _point2_or_none(value: Any) -> tuple[float, float] | None:
    if value is None:
        return None
    try:
        return _point2(value)
    except Exception:
        return None


def _float_or_none(value: Any) -> float | None:
    if value is None:
        return None
    try:
        return float(value)
    except Exception:
        return None


def _dimension_text(value: Any) -> str:
    text = str(value or "")
    if text.strip() == "":
        return "<>"
    return text


def _signed_line_distance_2d(
    point: tuple[float, float],
    line_start: tuple[float, float],
    line_end: tuple[float, float],
) -> float:
    dx = line_end[0] - line_start[0]
    dy = line_end[1] - line_start[1]
    length = math.hypot(dx, dy)
    if length <= 1.0e-12:
        return 0.0
    cross = dx * (point[1] - line_start[1]) - dy * (point[0] - line_start[0])
    return cross / length


def _ordinate_dim_type(
    feature: tuple[float, float],
    offset: tuple[float, float],
) -> int:
    dx = abs(offset[0] - feature[0])
    dy = abs(offset[1] - feature[1])
    return 0 if dx >= dy else 1
