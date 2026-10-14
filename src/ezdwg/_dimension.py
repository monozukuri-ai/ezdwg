"""Editable DIMENSION support: schema, validation and block rendering.

AutoCAD does not regenerate dimension graphics when it opens a drawing, so a
DIMENSION entity needs an anonymous block (``*D1``...) that holds its lines,
arrowheads and text. ``render_dimension`` builds those entities from the
definition points with the defaults of the ``Standard`` DIMSTYLE that the
writer stores (ISO-25 like values: arrow 2.5, text 2.5, gap 0.625, 2 decimals).
"""

from __future__ import annotations

import math
from typing import Any

KINDS = {
    "linear": "linear",
    "aligned": "aligned",
    "angular": "angular",
    "angular3pt": "angular",
    "angular2ln": "angular2ln",
    "angular2line": "angular2ln",
    "radius": "radius",
    "diameter": "diameter",
    "ordinate": "ordinate",
    # Reader type names.
    "ang3pt": "angular",
    "ang2ln": "angular2ln",
    "dim_linear": "linear",
    "dim_aligned": "aligned",
    "dim_ang3pt": "angular",
    "dim_ang2ln": "angular2ln",
    "dim_radius": "radius",
    "dim_diameter": "diameter",
    "dim_ordinate": "ordinate",
}
TYPE_CODES = {
    "ordinate": 6,
    "linear": 0,
    "aligned": 1,
    "angular": 5,
    "angular2ln": 2,
    "radius": 4,
    "diameter": 3,
}

#: Standard DIMSTYLE values written by the AC1015 writer (see database.rs).
STYLE = {
    "dimasz": 2.5,
    "dimexo": 0.625,
    "dimexe": 1.25,
    "dimtxt": 2.5,
    "dimgap": 0.625,
    "dimdec": 2,
    "dimscale": 1.0,
}

FIELDS = {
    "dimtype",
    "defpoint",
    "defpoint2",
    "defpoint3",
    "defpoint4",
    "defpoint5",
    "oblique_angle",
    "angle",
    "leader_length",
    "ordinate_x",
    "text_midpoint",
    "insert",
    "elevation",
    "dim_flags",
    "text",
    "text_rotation",
    "horizontal_direction",
    "insert_scale",
    "insert_rotation",
    "attachment_point",
    "line_spacing_style",
    "line_spacing_factor",
    "actual_measurement",
    "dimstyle",
    "geometry",
    "extrusion",
    # Reader-only keys that an editable import keeps for reference.
    "anonymous_block_name",
    "anonymous_block_handle",
    "dimstyle_handle",
    "char_height",
    "char_height_source",
    "common",
}


def kind_of(value: Any) -> str:
    try:
        return KINDS[str(value).lower()]
    except KeyError as exc:
        raise ValueError(f"unsupported dimension type: {value!r}") from exc


def dimension_error(d: dict[str, Any], number, point) -> None:
    """Raise ValueError for an invalid editable DIMENSION dictionary."""
    kind = kind_of(d.get("dimtype", "linear"))
    needed = {
        "linear": ("defpoint", "defpoint2", "defpoint3"),
        "aligned": ("defpoint", "defpoint2", "defpoint3"),
        "angular": ("defpoint", "defpoint2", "defpoint3", "defpoint4"),
        "angular2ln": ("defpoint", "defpoint2", "defpoint3", "defpoint4", "defpoint5"),
        "radius": ("defpoint", "defpoint4"),
        "diameter": ("defpoint", "defpoint4"),
        "ordinate": ("defpoint", "defpoint2", "defpoint3"),
    }[kind]
    for key in needed:
        if key not in d:
            raise ValueError(f"{kind} dimension requires {key}")
        point(d[key])
    for key in ("oblique_angle", "angle", "text_rotation", "horizontal_direction", "insert_rotation"):
        number(d.get(key, 0.0))
    if number(d.get("leader_length", 0.0)) < 0:
        raise ValueError("leader_length must not be negative")
    for key in ("text_midpoint", "insert"):
        if d.get(key) is not None:
            point(d[key])
    scale = d.get("insert_scale", (1.0, 1.0, 1.0))
    if point(scale) == (0.0, 0.0, 0.0) or any(number(v) == 0 for v in scale):
        raise ValueError("insert_scale components must be nonzero")
    flags = d.get("dim_flags", 3)
    if isinstance(flags, bool) or not isinstance(flags, int) or not 0 <= flags <= 3:
        raise ValueError("dim_flags must be an integer 0..3")
    attachment = d.get("attachment_point", 5)
    if isinstance(attachment, bool) or not isinstance(attachment, int) or not 1 <= attachment <= 9:
        raise ValueError("attachment_point must be in 1..9")
    if d.get("line_spacing_style", 1) not in (1, 2):
        raise ValueError("line_spacing_style must be 1 or 2")
    if not 0.25 <= number(d.get("line_spacing_factor", 1.0)) <= 4.0:
        raise ValueError("line_spacing_factor must be in 0.25..4")
    text = d.get("text", "")
    if not isinstance(text, str) or len(text.encode("utf-8")) > 255:
        raise ValueError("dimension text must be a string of at most 255 bytes")
    if d.get("actual_measurement") is not None:
        number(d["actual_measurement"])
    geometry = d.get("geometry")
    if geometry is not None and (not isinstance(geometry, str) or not geometry.startswith("*")):
        raise ValueError("geometry must name an anonymous block (*D...)")
    dimstyle = d.get("dimstyle", "Standard")
    if not isinstance(dimstyle, str) or dimstyle.upper() != "STANDARD":
        raise ValueError("the AC1015 writer supports the Standard DIMSTYLE only")


def format_measurement(value: float, decimals: int = STYLE["dimdec"]) -> str:
    text = f"{abs(value):.{decimals}f}"
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return text or "0"


def _xy(p) -> tuple[float, float]:
    return float(p[0]), float(p[1])


def _unit(v: tuple[float, float]) -> tuple[float, float]:
    length = math.hypot(*v)
    if length < 1e-12:
        return (1.0, 0.0)
    return (v[0] / length, v[1] / length)


def _add(a, b):
    return (a[0] + b[0], a[1] + b[1])


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def _mul(v, s):
    return (v[0] * s, v[1] * s)


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1]


def _perp(v):
    return (-v[1], v[0])


def _readable(direction: tuple[float, float]) -> tuple[float, float]:
    """Flip a text direction so it reads left to right / bottom to top."""
    angle = math.degrees(math.atan2(direction[1], direction[0]))
    if angle > 90.0 or angle <= -90.0:
        return (-direction[0], -direction[1])
    return direction


def _measurement_text(d: dict[str, Any], measurement: float, prefix: str = "") -> str:
    text = str(d.get("text", "") or "")
    if text == " ":
        return ""
    formatted = prefix + format_measurement(measurement)
    if not text:
        return formatted
    return text.replace("<>", formatted)


class _Builder:
    def __init__(self) -> None:
        self.entities: list[tuple[str, dict[str, Any]]] = []
        self.common = {"layer": "0", "color_index": 0}

    def line(self, a, b) -> None:
        if math.hypot(b[0] - a[0], b[1] - a[1]) > 1e-9:
            self.entities.append(("LINE", {"start": (*a, 0.0), "end": (*b, 0.0), **self.common}))

    def arrow(self, tip, direction, size) -> None:
        """Closed filled arrowhead with its tip at ``tip`` pointing along ``direction``."""
        u = _unit(direction)
        back = _sub(tip, _mul(u, size))
        half = _mul(_perp(u), size / 6.0)
        p1, p2 = _add(back, half), _sub(back, half)
        self.entities.append(
            (
                "SOLID",
                {
                    "points": [(*tip, 0.0), (*p1, 0.0), (*p2, 0.0), (*p2, 0.0)],
                    **self.common,
                },
            )
        )

    def arc(self, center, radius, start_deg, end_deg) -> None:
        if radius > 1e-9:
            self.entities.append(
                (
                    "ARC",
                    {
                        "center": (*center, 0.0),
                        "radius": radius,
                        "start_angle": start_deg,
                        "end_angle": end_deg,
                        **self.common,
                    },
                )
            )

    def text(self, content, center, rotation_deg, height) -> None:
        if content:
            self.entities.append(
                (
                    "TEXT",
                    {
                        "text": content,
                        "insert": (*center, 0.0),
                        "align_point": (*center, 0.0),
                        "height": height,
                        "rotation": rotation_deg,
                        "halign": 1,
                        "valign": 2,
                        "style": "Standard",
                        **self.common,
                    },
                )
            )


def render_dimension(d: dict[str, Any]) -> tuple[list[tuple[str, dict[str, Any]]], dict[str, Any]]:
    """Entities of the anonymous block and the derived dimension values.

    Returns ``(entities, derived)`` where ``derived`` holds ``actual_measurement``,
    ``text_midpoint`` and ``text_rotation`` for the DIMENSION entity.
    """
    kind = kind_of(d.get("dimtype", "linear"))
    scale = float(d.get("dimscale", STYLE["dimscale"]))
    asz, exo, exe = STYLE["dimasz"] * scale, STYLE["dimexo"] * scale, STYLE["dimexe"] * scale
    txt, gap = STYLE["dimtxt"] * scale, STYLE["dimgap"] * scale
    b = _Builder()
    derived: dict[str, Any] = {}
    if kind in ("linear", "aligned"):
        p1, p2, base = _xy(d["defpoint2"]), _xy(d["defpoint3"]), _xy(d["defpoint"])
        if kind == "linear":
            angle = math.radians(float(d.get("angle", 0.0)))
            direction = (math.cos(angle), math.sin(angle))
        else:
            direction = _unit(_sub(p2, p1))
        normal = _perp(direction)
        offset = _dot(_sub(base, p1), normal)
        d1 = _add(p1, _mul(normal, offset))
        d2 = _add(p2, _mul(normal, offset))
        if kind == "linear":
            d1 = _add(base, _mul(direction, _dot(_sub(p1, base), direction)))
            d2 = _add(base, _mul(direction, _dot(_sub(p2, base), direction)))
        measurement = abs(_dot(_sub(p2, p1), direction)) if kind == "linear" else math.hypot(*_sub(p2, p1))
        for origin, end in ((p1, d1), (p2, d2)):
            e = _sub(end, origin)
            if math.hypot(*e) > 1e-9:
                u = _unit(e)
                b.line(_add(origin, _mul(u, exo)), _add(end, _mul(u, exe)))
        along = _unit(_sub(d2, d1)) if math.hypot(*_sub(d2, d1)) > 1e-9 else direction
        b.line(d1, d2)
        b.arrow(d1, _mul(along, -1.0), asz)
        b.arrow(d2, along, asz)
        read = _readable(along)
        up = _perp(read)
        center = _add(_mul(_add(d1, d2), 0.5), _mul(up, gap + txt / 2.0))
        rotation = math.degrees(math.atan2(read[1], read[0]))
        b.text(_measurement_text(d, measurement), center, rotation, txt)
        derived.update(actual_measurement=measurement, text_midpoint=center, text_rotation=rotation)
    elif kind in ("radius", "diameter"):
        p10, p15 = _xy(d["defpoint"]), _xy(d["defpoint4"])
        if kind == "radius":
            center, on_arc = p10, p15
            radius = math.hypot(*_sub(on_arc, center))
            direction = _unit(_sub(on_arc, center))
            leader = float(d.get("leader_length", 0.0)) or asz * 2.0
            end = _add(on_arc, _mul(direction, leader))
            b.line(center, end)
            b.arrow(on_arc, direction, asz)
            read = _readable(direction)
            text = _measurement_text(d, radius, "R")
            text_center = _add(end, _mul(read, gap + txt * 0.35 * len(text)))
            b.text(text, text_center, 0.0, txt)
            derived.update(actual_measurement=radius, text_midpoint=text_center, text_rotation=0.0)
        else:
            direction = _unit(_sub(p10, p15))
            diameter = math.hypot(*_sub(p10, p15))
            b.line(p15, p10)
            b.arrow(p15, _mul(direction, -1.0), asz)
            b.arrow(p10, direction, asz)
            read = _readable(direction)
            up = _perp(read)
            center = _add(_mul(_add(p10, p15), 0.5), _mul(up, gap + txt / 2.0))
            rotation = math.degrees(math.atan2(read[1], read[0]))
            b.text(_measurement_text(d, diameter, "%%C"), center, rotation, txt)
            derived.update(actual_measurement=diameter, text_midpoint=center, text_rotation=rotation)
    elif kind in ("angular", "angular2ln"):
        if kind == "angular":
            vertex, p1, p2, on_arc = _xy(d["defpoint4"]), _xy(d["defpoint2"]), _xy(d["defpoint3"]), _xy(d["defpoint"])
        else:
            a1, a2 = _xy(d["defpoint2"]), _xy(d["defpoint3"])
            b1, b2 = _xy(d["defpoint4"]), _xy(d["defpoint"])
            on_arc = _xy(d["defpoint5"])
            vertex = _intersection(a1, a2, b1, b2)
            p1 = a2 if math.hypot(*_sub(a2, vertex)) > math.hypot(*_sub(a1, vertex)) else a1
            p2 = b2 if math.hypot(*_sub(b2, vertex)) > math.hypot(*_sub(b1, vertex)) else b1
        radius = math.hypot(*_sub(on_arc, vertex))
        start = math.degrees(math.atan2(*_sub(p1, vertex)[::-1]))
        end = math.degrees(math.atan2(*_sub(p2, vertex)[::-1]))
        sweep = (end - start) % 360.0
        measurement = sweep
        b.arc(vertex, radius, start, start + sweep)
        for p in (p1, p2):
            u = _unit(_sub(p, vertex))
            arc_point = _add(vertex, _mul(u, radius))
            if math.hypot(*_sub(arc_point, p)) > exo:
                seg = _unit(_sub(arc_point, p))
                b.line(_add(p, _mul(seg, exo)), _add(arc_point, _mul(seg, exe)))
        for ang, sign in ((start, 1.0), (start + sweep, -1.0)):
            r = math.radians(ang)
            tip = _add(vertex, _mul((math.cos(r), math.sin(r)), radius))
            tangent = _mul((-math.sin(r), math.cos(r)), -sign)
            b.arrow(tip, tangent, asz)
        mid = math.radians(start + sweep / 2.0)
        radial = (math.cos(mid), math.sin(mid))
        center = _add(vertex, _mul(radial, radius + gap + txt / 2.0))
        read = _readable(_perp(radial))
        rotation = math.degrees(math.atan2(read[1], read[0]))
        b.text(_measurement_text(d, measurement) + ("%%D" if not str(d.get("text", "")) else ""), center, rotation, txt)
        derived.update(actual_measurement=measurement, text_midpoint=center, text_rotation=rotation)
    else:  # ordinate
        origin, feature, leader = _xy(d["defpoint"]), _xy(d["defpoint2"]), _xy(d["defpoint3"])
        x_type = bool(d.get("ordinate_x", True))
        if x_type:
            measurement = abs(feature[0] - origin[0])
            start = (feature[0], feature[1] + math.copysign(exo, leader[1] - feature[1]))
            knee = (feature[0], leader[1] - math.copysign(asz, leader[1] - feature[1]))
            b.line(start, knee)
            b.line(knee, leader)
            rotation = 90.0
            center = _add(leader, (0.0, math.copysign(gap + txt / 2.0, leader[1] - feature[1])))
        else:
            measurement = abs(feature[1] - origin[1])
            start = (feature[0] + math.copysign(exo, leader[0] - feature[0]), feature[1])
            knee = (leader[0] - math.copysign(asz, leader[0] - feature[0]), feature[1])
            b.line(start, knee)
            b.line(knee, leader)
            rotation = 0.0
            center = _add(leader, (math.copysign(gap + txt / 2.0, leader[0] - feature[0]), 0.0))
        b.text(_measurement_text(d, measurement), center, rotation, txt)
        derived.update(actual_measurement=measurement, text_midpoint=center, text_rotation=rotation)
    return b.entities, derived


def _intersection(a1, a2, b1, b2) -> tuple[float, float]:
    d1, d2 = _sub(a2, a1), _sub(b2, b1)
    cross = d1[0] * d2[1] - d1[1] * d2[0]
    if abs(cross) < 1e-12:
        return a1
    t = ((b1[0] - a1[0]) * d2[1] - (b1[1] - a1[1]) * d2[0]) / cross
    return _add(a1, _mul(d1, t))
