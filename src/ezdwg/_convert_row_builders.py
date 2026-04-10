from __future__ import annotations

import math

from .entity import Entity


def _as_line_row(entity: Entity) -> tuple[int, float, float, float, float, float, float] | None:
    start = entity.dxf.get("start")
    end = entity.dxf.get("end")
    if not (
        isinstance(start, tuple)
        and len(start) == 3
        and isinstance(end, tuple)
        and len(end) == 3
    ):
        return None
    return (
        int(entity.handle),
        float(start[0]),
        float(start[1]),
        float(start[2]),
        float(end[0]),
        float(end[1]),
        float(end[2]),
    )


def _as_point_row(entity: Entity) -> tuple[int, float, float, float, float] | None:
    location = entity.dxf.get("location")
    x_axis_angle = entity.dxf.get("x_axis_angle", 0.0)
    if not (
        isinstance(location, tuple)
        and len(location) == 3
        and isinstance(x_axis_angle, (int, float))
    ):
        return None
    return (
        int(entity.handle),
        float(location[0]),
        float(location[1]),
        float(location[2]),
        float(x_axis_angle),
    )


def _as_ray_row(entity: Entity) -> tuple[int, tuple[float, float, float], tuple[float, float, float]] | None:
    start = entity.dxf.get("start")
    unit_vector = entity.dxf.get("unit_vector")
    if not (
        isinstance(start, tuple)
        and len(start) == 3
        and isinstance(unit_vector, tuple)
        and len(unit_vector) == 3
    ):
        return None
    return (
        int(entity.handle),
        (float(start[0]), float(start[1]), float(start[2])),
        (float(unit_vector[0]), float(unit_vector[1]), float(unit_vector[2])),
    )


def _as_xline_row(entity: Entity) -> tuple[int, tuple[float, float, float], tuple[float, float, float]] | None:
    return _as_ray_row(entity)


def _as_arc_row(entity: Entity) -> tuple[int, float, float, float, float, float, float] | None:
    center = entity.dxf.get("center")
    radius = entity.dxf.get("radius")
    start_angle = entity.dxf.get("start_angle")
    end_angle = entity.dxf.get("end_angle")
    if not (
        isinstance(center, tuple)
        and len(center) == 3
        and isinstance(radius, (int, float))
        and isinstance(start_angle, (int, float))
        and isinstance(end_angle, (int, float))
    ):
        return None
    return (
        int(entity.handle),
        float(center[0]),
        float(center[1]),
        float(center[2]),
        float(radius),
        math.radians(float(start_angle)),
        math.radians(float(end_angle)),
    )


def _as_circle_row(entity: Entity) -> tuple[int, float, float, float, float] | None:
    center = entity.dxf.get("center")
    radius = entity.dxf.get("radius")
    if not (isinstance(center, tuple) and len(center) == 3 and isinstance(radius, (int, float))):
        return None
    return (
        int(entity.handle),
        float(center[0]),
        float(center[1]),
        float(center[2]),
        float(radius),
    )


def _as_lwpolyline_row(
    entity: Entity,
) -> tuple[int, int, list[tuple[float, float]], list[float], list[tuple[float, float]], float | None] | None:
    points = entity.dxf.get("points")
    flags = entity.dxf.get("flags", 0)
    bulges = entity.dxf.get("bulges") or []
    widths = entity.dxf.get("widths") or []
    const_width = entity.dxf.get("const_width")
    if not isinstance(points, list):
        return None
    points2d: list[tuple[float, float]] = []
    for point in points:
        if not (isinstance(point, tuple) and len(point) >= 2):
            return None
        points2d.append((float(point[0]), float(point[1])))
    out_bulges = [float(value) for value in list(bulges)]
    out_widths: list[tuple[float, float]] = []
    for width in list(widths):
        if not (isinstance(width, tuple) and len(width) == 2):
            continue
        out_widths.append((float(width[0]), float(width[1])))
    out_const_width = float(const_width) if isinstance(const_width, (int, float)) else None
    return (
        int(entity.handle),
        int(flags),
        points2d,
        out_bulges,
        out_widths,
        out_const_width,
    )


def _as_text_row(entity: Entity) -> tuple[int, str, tuple[float, float, float], float, float] | None:
    text = entity.dxf.get("text")
    insert = entity.dxf.get("insert")
    height = entity.dxf.get("height")
    rotation = entity.dxf.get("rotation", 0.0)
    if not (
        isinstance(text, str)
        and isinstance(insert, tuple)
        and len(insert) == 3
        and isinstance(height, (int, float))
        and isinstance(rotation, (int, float))
    ):
        return None
    return (
        int(entity.handle),
        text,
        (float(insert[0]), float(insert[1]), float(insert[2])),
        float(height),
        math.radians(float(rotation)),
    )


def _as_mtext_row(
    entity: Entity,
) -> tuple[int, str, tuple[float, float, float], tuple[float, float, float], float, float, int, int] | None:
    text = entity.dxf.get("raw_text")
    if not isinstance(text, str):
        text = entity.dxf.get("text")
    insert = entity.dxf.get("insert")
    text_direction = entity.dxf.get("text_direction")
    if not (
        isinstance(insert, tuple)
        and len(insert) == 3
        and isinstance(text, str)
    ):
        return None

    if isinstance(text_direction, tuple) and len(text_direction) == 3:
        direction = (
            float(text_direction[0]),
            float(text_direction[1]),
            float(text_direction[2]),
        )
    else:
        rotation = float(entity.dxf.get("rotation", 0.0))
        angle = math.radians(rotation)
        direction = (math.cos(angle), math.sin(angle), 0.0)

    rect_width = float(entity.dxf.get("rect_width", 0.0))
    char_height = float(entity.dxf.get("char_height", entity.dxf.get("height", 1.0)))
    attachment_point = int(entity.dxf.get("attachment_point", 1))
    drawing_direction = int(entity.dxf.get("drawing_direction", 1))

    return (
        int(entity.handle),
        text,
        (float(insert[0]), float(insert[1]), float(insert[2])),
        direction,
        rect_width,
        char_height,
        attachment_point,
        drawing_direction,
    )
