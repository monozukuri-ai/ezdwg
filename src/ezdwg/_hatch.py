"""Editable HATCH support: predefined patterns, pattern transforms and validation.

A pattern definition line is ``{"angle", "base", "offset", "dashes"}`` with the
angle in degrees. DWG stores the lines already rotated by the hatch angle and
scaled by the hatch scale; ``transform_pattern_lines`` converts an ``acad.pat``
style definition (offset in the line's own coordinate system) into that form.
"""

from __future__ import annotations

import math
from typing import Any

#: ``acad.pat`` definitions in inches: angle, base x, base y, offset dx (along
#: the line), offset dy (perpendicular), dashes. Scale by 25.4 for ISO drawings.
PATTERNS: dict[str, list[tuple[float, float, float, float, float, list[float]]]] = {
    "ANSI31": [(45.0, 0.0, 0.0, 0.0, 0.125, [])],
    "ANSI32": [(45.0, 0.0, 0.0, 0.0, 0.375, []), (45.0, 0.176776695, 0.0, 0.0, 0.375, [])],
    "ANSI33": [
        (45.0, 0.0, 0.0, 0.0, 0.25, []),
        (45.0, 0.176776695, 0.0, 0.0, 0.25, [0.125, -0.0625]),
    ],
    "ANSI37": [(45.0, 0.0, 0.0, 0.0, 0.125, []), (135.0, 0.0, 0.0, 0.0, 0.125, [])],
    "DOTS": [(0.0, 0.0, 0.0, 0.03125, 0.0625, [0.0, -0.0625])],
    "LINE": [(0.0, 0.0, 0.0, 0.0, 0.125, [])],
    "NET": [(0.0, 0.0, 0.0, 0.0, 0.125, []), (90.0, 0.0, 0.0, 0.0, 0.125, [])],
}


def _rotate(x: float, y: float, degrees: float) -> tuple[float, float]:
    c, s = math.cos(math.radians(degrees)), math.sin(math.radians(degrees))
    return x * c - y * s, x * s + y * c


def transform_pattern_lines(
    lines: str | list[Any], angle: float = 0.0, scale: float = 1.0
) -> list[dict[str, Any]]:
    """Stored (rotated and scaled) pattern lines of a definition.

    ``lines`` is a name from ``PATTERNS`` or a list of ``acad.pat`` style rows
    ``(angle, base_x, base_y, offset_dx, offset_dy, dashes)``. The result has
    degrees for angles and OCS-oriented base/offset vectors, as DWG stores them.
    """
    if isinstance(lines, str):
        try:
            rows = PATTERNS[lines.upper()]
        except KeyError as exc:
            raise ValueError(f"unknown predefined hatch pattern: {lines!r}") from exc
    else:
        rows = [tuple(r) for r in lines]
    if not math.isfinite(scale) or scale <= 0:
        raise ValueError("hatch pattern scale must be finite and positive")
    stored = []
    for row in rows:
        if len(row) != 6:
            raise ValueError("pattern line rows are (angle, base_x, base_y, dx, dy, dashes)")
        line_angle, bx, by, dx, dy, dashes = row
        base = _rotate(float(bx) * scale, float(by) * scale, angle)
        offset = _rotate(float(dx) * scale, float(dy) * scale, float(line_angle) + angle)
        stored.append(
            {
                "angle": float(line_angle) + angle,
                "base": base,
                "offset": offset,
                "dashes": [float(d) * scale for d in dashes],
            }
        )
    return stored


EDGE_TYPES = {"line": 1, "arc": 2, "ellipse": 3, "spline": 4}


def hatch_error(d: dict[str, Any], number, point2) -> None:
    """Raise ValueError for an invalid editable HATCH dictionary."""
    name = d.get("pattern_name", "SOLID")
    if not isinstance(name, str) or not name or len(name.encode("utf-8")) > 255:
        raise ValueError("pattern_name must be a non-empty string")
    for key, limit in (("hatch_style", 2), ("pattern_type", 2)):
        value = d.get(key, 0)
        if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= limit:
            raise ValueError(f"{key} must be an integer 0..{limit}")
    paths = d.get("paths")
    if not isinstance(paths, (list, tuple)) or not paths:
        raise ValueError("hatch requires at least one boundary path")
    derived = False
    for path in paths:
        if not isinstance(path, dict):
            raise ValueError("hatch paths must be dictionaries")
        flags = path.get("flags")
        if flags is not None and (
            isinstance(flags, bool) or not isinstance(flags, int) or not 0 <= flags <= 255
        ):
            raise ValueError("hatch path flags must be an integer 0..255")
        if "edges" in path:
            if "points" in path or "bulges" in path:
                raise ValueError("a hatch path is either points or edges")
            if flags is not None and flags & 2:
                raise ValueError("edge hatch path cannot carry path flag 2")
            edges = path["edges"]
            if not isinstance(edges, (list, tuple)) or not edges:
                raise ValueError("edge hatch path requires at least one edge")
            for edge in edges:
                if not isinstance(edge, dict) or edge.get("type") not in EDGE_TYPES:
                    raise ValueError("hatch edge type must be line, arc, ellipse or spline")
                kind = edge["type"]
                if kind == "line":
                    point2(edge["start"])
                    point2(edge["end"])
                elif kind == "arc":
                    point2(edge["center"])
                    if number(edge["radius"]) <= 0:
                        raise ValueError("hatch arc edge radius must be positive")
                    number(edge.get("start_angle", 0.0))
                    number(edge.get("end_angle", 360.0))
                elif kind == "ellipse":
                    point2(edge["center"])
                    if point2(edge["major_axis"]) == (0.0, 0.0):
                        raise ValueError("hatch ellipse edge major axis must be nonzero")
                    ratio = number(edge.get("ratio", 1.0))
                    if not 0 < ratio <= 1:
                        raise ValueError("hatch ellipse edge ratio must be in (0, 1]")
                    number(edge.get("start_angle", 0.0))
                    number(edge.get("end_angle", 360.0))
                else:
                    degree = edge.get("degree", 3)
                    if isinstance(degree, bool) or not isinstance(degree, int) or not 1 <= degree <= 10:
                        raise ValueError("hatch spline edge degree must be in 1..10")
                    controls = [point2(p) for p in edge.get("control_points", [])]
                    knots = [number(k) for k in edge.get("knots", [])]
                    weights = [number(w) for w in edge.get("weights", [])]
                    if len(controls) <= degree or len(knots) != len(controls) + degree + 1:
                        raise ValueError("hatch spline edge knot/control count does not match degree")
                    if edge.get("rational") and len(weights) != len(controls):
                        raise ValueError("rational hatch spline edge requires one weight per control point")
        else:
            if flags is not None and not flags & 2:
                raise ValueError("polyline hatch path requires path flag 2")
            points = path.get("points")
            if not isinstance(points, (list, tuple)) or len(points) < 2:
                raise ValueError("polyline hatch path requires at least 2 points")
            for p in points:
                point2(p)
            bulges = path.get("bulges")
            if bulges is not None:
                if len(bulges) != len(points):
                    raise ValueError("hatch path bulge count must match its points")
                for b in bulges:
                    number(b)
        if flags is not None and flags & 4:
            derived = True
    pixel = d.get("pixel_size")
    if pixel is not None and number(pixel) <= 0:
        raise ValueError("hatch pixel size must be positive")
    if derived and pixel is None:
        raise ValueError("derived hatch paths (flag 4) require a pixel_size")
    for seed in d.get("seed_points", []):
        point2(seed)
    solid = d.get("solid_fill", True)
    if not isinstance(solid, bool):
        raise ValueError("solid_fill must be a boolean")
    if solid:
        return
    if number(d.get("pattern_scale", 1.0)) <= 0:
        raise ValueError("hatch pattern scale must be positive")
    number(d.get("pattern_angle", 0.0))
    lines = d.get("pattern_lines")
    if not isinstance(lines, (list, tuple)) or not lines:
        raise ValueError("pattern hatch requires pattern_lines")
    for line in lines:
        if not isinstance(line, dict):
            raise ValueError("pattern lines must be dictionaries")
        number(line.get("angle", 0.0))
        point2(line.get("base", (0.0, 0.0)))
        point2(line.get("offset", (0.0, 0.0)))
        for dash in line.get("dashes", []):
            number(dash)
