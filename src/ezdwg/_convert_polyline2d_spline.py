from __future__ import annotations

import math
from typing import Any

from ._convert_utils import _point3

_POLYLINE_2D_SPLINE_CURVE_TYPES = {"QuadraticBSpline", "CubicBSpline", "Bezier"}


def _should_write_polyline_2d_as_spline(dxf: dict[str, Any]) -> bool:
    if bool(dxf.get("interpolation_applied", False)):
        return len(list(dxf.get("interpolated_points", []) or [])) >= 2
    if bool(dxf.get("curve_fit", False)) or bool(dxf.get("spline_fit", False)):
        return len(list(dxf.get("points", []) or [])) >= 2
    curve_type_label = str(dxf.get("curve_type_label") or "")
    if curve_type_label in _POLYLINE_2D_SPLINE_CURVE_TYPES:
        return len(list(dxf.get("points", []) or [])) >= 2
    return False


def _polyline_2d_spline_points(dxf: dict[str, Any]) -> list[tuple[float, float, float]]:
    points = _polyline_2d_select_curve_points(dxf)
    if len(points) < 2 and bool(dxf.get("interpolation_applied", False)):
        points = [_point3(point) for point in list(dxf.get("interpolated_points") or [])]
    if len(points) < 2:
        return points
    closed = bool(dxf.get("closed", False))
    if closed:
        if points[0] != points[-1]:
            points.append(points[0])
    else:
        while len(points) > 1 and points[0] == points[-1]:
            points.pop()
    return points


def _polyline_2d_spline_degree(dxf: dict[str, Any], point_count: int) -> int:
    label = str(dxf.get("curve_type_label") or "")
    if label == "QuadraticBSpline":
        preferred = 2
    elif label in {"CubicBSpline", "Bezier"}:
        preferred = 3
    else:
        preferred = int(dxf.get("degree", 3))
    clamped = max(2, min(preferred, max(2, point_count - 1)))
    return clamped


def _polyline_2d_spline_payload(dxf: dict[str, Any]) -> dict[str, Any] | None:
    spline_points = _polyline_2d_spline_points(dxf)
    if len(spline_points) < 2:
        return None

    spline_degree = _polyline_2d_spline_degree(dxf, len(spline_points))
    curve_type_label = str(dxf.get("curve_type_label") or "")
    curve_fit = bool(dxf.get("curve_fit", False))
    spline_fit = bool(dxf.get("spline_fit", False))

    # For pure curve_type-driven splines, preserve control points directly.
    if (
        curve_type_label in _POLYLINE_2D_SPLINE_CURVE_TYPES
        and not curve_fit
        and not spline_fit
    ):
        control_points = list(spline_points)
        if len(control_points) >= 2:
            if len(control_points) > 1 and control_points[0] == control_points[-1]:
                control_points = control_points[:-1]
            if len(control_points) >= 2:
                degree = max(2, min(spline_degree, max(2, len(control_points) - 1)))
                return {
                    "control_points": control_points,
                    "degree": degree,
                    "knots": _open_uniform_knot_vector(len(control_points), degree),
                    "closed": bool(dxf.get("closed", False)),
                }

    spline_tangents = _polyline_2d_spline_tangents(dxf)
    return {
        "fit_points": spline_points,
        "degree": spline_degree,
        "fit_tangents": spline_tangents,
        "closed": bool(dxf.get("closed", False)),
    }


def _open_uniform_knot_vector(control_point_count: int, degree: int) -> list[float]:
    n = int(control_point_count)
    p = int(degree)
    if n < 2:
        return []
    p = max(1, min(p, n - 1))
    knot_count = n + p + 1
    if knot_count <= 0:
        return []

    knots: list[float] = []
    for i in range(knot_count):
        if i <= p:
            knots.append(0.0)
        elif i >= n:
            knots.append(1.0)
        else:
            knots.append((i - p) / (n - p))
    return knots


def _polyline_2d_select_curve_points(dxf: dict[str, Any]) -> list[tuple[float, float, float]]:
    points = [_point3(point) for point in list(dxf.get("points") or [])]
    if len(points) < 2:
        return points
    indices = _polyline_2d_select_curve_indices(dxf, len(points))
    return [points[i] for i in indices]


def _polyline_2d_select_curve_indices(dxf: dict[str, Any], point_count: int) -> list[int]:
    if point_count < 2:
        return list(range(point_count))

    vertex_flags_raw = list(dxf.get("vertex_flags") or [])
    if not vertex_flags_raw:
        return list(range(point_count))

    vertex_flags = [int(flag) for flag in vertex_flags_raw]
    count = min(point_count, len(vertex_flags))
    if count < 2:
        return list(range(point_count))

    paired = [(i, vertex_flags[i]) for i in range(count)]
    has_spline_frame = any((flag & 0x10) != 0 for _, flag in paired)
    if has_spline_frame:
        selected = [idx for idx, flag in paired if (flag & 0x10) != 0]
        if len(selected) >= 2:
            return selected

    # Exclude curve/spline generated vertices (DXF vertex flags bit1/bit8).
    selected = [idx for idx, flag in paired if (flag & 0x09) == 0]
    if len(selected) >= 2:
        return selected
    return list(range(point_count))


def _polyline_2d_spline_tangents(dxf: dict[str, Any]) -> list[tuple[float, float, float]] | None:
    if bool(dxf.get("closed", False)):
        return None

    points = list(dxf.get("points") or [])
    if len(points) < 2:
        return None
    indices = _polyline_2d_select_curve_indices(dxf, len(points))
    if len(indices) < 2:
        return None

    tangent_dirs = list(dxf.get("tangent_dirs") or [])
    vertex_flags = [int(flag) for flag in list(dxf.get("vertex_flags") or [])]
    if not tangent_dirs or not vertex_flags:
        return None

    limit = min(len(points), len(vertex_flags), len(tangent_dirs))
    if limit < 2:
        return None

    angle_unit = _polyline_2d_tangent_angle_unit(tangent_dirs)

    def tangent_vector(angle: float) -> tuple[float, float, float]:
        if angle_unit == "deg":
            angle = math.radians(angle)
        return (math.cos(angle), math.sin(angle), 0.0)

    start = None
    for idx in indices:
        if idx >= limit:
            continue
        if (vertex_flags[idx] & 0x02) == 0:
            continue
        angle = float(tangent_dirs[idx])
        if not math.isfinite(angle):
            continue
        start = tangent_vector(angle)
        break

    end = None
    for idx in reversed(indices):
        if idx >= limit:
            continue
        if (vertex_flags[idx] & 0x02) == 0:
            continue
        angle = float(tangent_dirs[idx])
        if not math.isfinite(angle):
            continue
        end = tangent_vector(angle)
        break

    if start is None or end is None:
        return None
    return [start, end]


def _polyline_2d_tangent_angle_unit(raw_angles: list[Any]) -> str:
    finite: list[float] = []
    for raw in raw_angles:
        try:
            value = float(raw)
        except Exception:
            continue
        if math.isfinite(value):
            finite.append(value)
    if not finite:
        return "rad"
    max_abs = max(abs(value) for value in finite)
    # Most DWG data stores radians. Values clearly beyond one full turn
    # indicate degree-like data from upstream conversion quirks.
    if max_abs > (2.0 * math.pi + 1.0e-3):
        return "deg"
    return "rad"
