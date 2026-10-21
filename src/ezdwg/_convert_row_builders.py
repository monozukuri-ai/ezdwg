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


def _as_ellipse_row(entity: Entity) -> tuple[
    int,
    tuple[float, float, float],
    tuple[float, float, float],
    tuple[float, float, float],
    float,
    float,
    float,
]:
    """Keep WCS coordinates and eccentric anomaly parameters in radians."""
    d = entity.dxf
    return (
        int(entity.handle),
        tuple(float(v) for v in d["center"]),
        tuple(float(v) for v in d["major_axis"]),
        tuple(float(v) for v in d.get("extrusion", (0.0, 0.0, 1.0))),
        float(d["axis_ratio"]),
        float(d["start_angle"]),
        float(d["end_angle"]),
    )


def _as_legacy_polyline_row(entity: Entity) -> dict:
    d = entity.dxf
    is_3d = entity.dxftype == "POLYLINE_3D"
    flags = int(d.get("flags", 8 if is_3d else 0))
    flags = (flags & ~1) | int(bool(d.get("closed", flags & 1)))
    row = {"handle": entity.handle, "points": d["points"], "flags": flags}
    if not is_3d:
        row.update({k: d[k] for k in ("width_start", "width_end", "widths", "bulges") if k in d})
    return row


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


def _as_spline_row(entity: Entity) -> dict:
    from ._write_spline import FIELDS

    return {"handle": int(entity.handle), **{k: v for k, v in entity.dxf.items() if k in FIELDS}}


def _as_face_row(entity: Entity) -> tuple:
    """Prevalidated corners retain stored order; triangles repeat the last corner."""
    d = entity.dxf
    points = [tuple(float(v) for v in p) for p in d["points"]]
    if len(points) == 3:
        points.append(points[-1])
    if entity.dxftype == "3DFACE":
        return (int(entity.handle), *points, d.get("invisible_edge_flags", 0))
    return (
        int(entity.handle), *points, float(d.get("thickness", 0.0)),
        tuple(float(v) for v in d.get("extrusion", (0.0, 0.0, 1.0))),
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


def _as_hatch_row(entity: Entity) -> dict:
    """Editable HATCH dictionary -> writer row (angles in radians, 2D points)."""
    import math

    from ._hatch import EDGE_TYPES

    d = entity.dxf
    paths = []
    for path in d["paths"]:
        row: dict = {}
        if "flags" in path:
            row["flags"] = int(path["flags"])
        if "edges" in path:
            edges = []
            for edge in path["edges"]:
                kind = EDGE_TYPES[edge["type"]]
                e: dict = {"type": kind}
                if kind == 1:
                    e["start"] = tuple(float(v) for v in edge["start"][:2])
                    e["end"] = tuple(float(v) for v in edge["end"][:2])
                elif kind in (2, 3):
                    e["center"] = tuple(float(v) for v in edge["center"][:2])
                    if kind == 2:
                        e["radius"] = float(edge["radius"])
                    else:
                        e["major_endpoint"] = tuple(float(v) for v in edge["major_axis"][:2])
                        e["ratio"] = float(edge.get("ratio", 1.0))
                    e["start_angle"] = math.radians(float(edge.get("start_angle", 0.0)))
                    e["end_angle"] = math.radians(float(edge.get("end_angle", 360.0)))
                    e["ccw"] = bool(edge.get("ccw", True))
                else:
                    e.update(
                        degree=int(edge.get("degree", 3)),
                        rational=bool(edge.get("rational", False)),
                        periodic=bool(edge.get("periodic", False)),
                        knots=[float(k) for k in edge.get("knots", [])],
                        control_points=[
                            tuple(float(v) for v in p[:2]) for p in edge.get("control_points", [])
                        ],
                        weights=[float(w) for w in edge.get("weights", [])],
                    )
                edges.append(e)
            row["edges"] = edges
        else:
            row["vertices"] = [tuple(float(v) for v in p[:2]) for p in path["points"]]
            row["closed"] = bool(path.get("closed", True))
            if path.get("bulges") is not None:
                row["bulges"] = [float(b) for b in path["bulges"]]
        paths.append(row)
    result = {
        "handle": int(entity.handle),
        "name": str(d.get("pattern_name", "SOLID")),
        "solid_fill": bool(d.get("solid_fill", True)),
        "paths": paths,
        "style": int(d.get("hatch_style", 0)),
        "pattern_type": int(d.get("pattern_type", 1)),
        "seed_points": [tuple(float(v) for v in p[:2]) for p in d.get("seed_points", [])],
    }
    if d.get("pixel_size") is not None:
        result["pixel_size"] = float(d["pixel_size"])
    if not result["solid_fill"]:
        result["pattern"] = {
            "angle": math.radians(float(d.get("pattern_angle", 0.0))),
            "scale": float(d.get("pattern_scale", 1.0)),
            "double": bool(d.get("pattern_double", False)),
            "lines": [
                {
                    "angle": math.radians(float(line.get("angle", 0.0))),
                    "base": tuple(float(v) for v in line.get("base", (0.0, 0.0))[:2]),
                    "offset": tuple(float(v) for v in line.get("offset", (0.0, 0.0))[:2]),
                    "dashes": [float(v) for v in line.get("dashes", [])],
                }
                for line in d.get("pattern_lines", [])
            ],
        }
    return result


def _as_dimension_row(entity: Entity) -> dict:
    """Editable DIMENSION dictionary -> writer row (radians, 3D points)."""
    import math

    from ._dimension import kind_of

    d = entity.dxf

    def p3(key):
        value = d.get(key, (0.0, 0.0, 0.0))
        return (float(value[0]), float(value[1]), float(value[2]) if len(value) > 2 else 0.0)

    def p2(key, default=(0.0, 0.0)):
        value = d.get(key)
        if value is None:
            return default
        return (float(value[0]), float(value[1]))

    text_midpoint = d.get("text_midpoint") or (0.0, 0.0, 0.0)
    elevation = float(d.get("elevation", text_midpoint[2] if len(text_midpoint) > 2 else 0.0))
    scale = d.get("insert_scale", (1.0, 1.0, 1.0))
    return {
        "handle": int(entity.handle),
        "kind": kind_of(d.get("dimtype", "linear")),
        "point10": p3("defpoint"),
        "point13": p3("defpoint2"),
        "point14": p3("defpoint3"),
        "point15": p3("defpoint4"),
        "point16": p2("defpoint5"),
        "ext_line_rotation": math.radians(float(d.get("oblique_angle", 0.0))),
        "dim_rotation": math.radians(float(d.get("angle", 0.0))),
        "leader_length": float(d.get("leader_length", 0.0)),
        "flags2": 1 if d.get("ordinate_x", True) else 0,
        "text_midpoint": p2("text_midpoint"),
        "elevation": elevation,
        "flags": int(d.get("dim_flags", 3)),
        "user_text": str(d.get("text", "") or ""),
        "text_rotation": math.radians(float(d.get("text_rotation", 0.0))),
        "horizontal_direction": math.radians(float(d.get("horizontal_direction", 0.0))),
        "insert_scale": (float(scale[0]), float(scale[1]), float(scale[2])),
        "insert_rotation": math.radians(float(d.get("insert_rotation", 0.0))),
        "attachment_point": int(d.get("attachment_point", 5)),
        "line_spacing_style": int(d.get("line_spacing_style", 1)),
        "line_spacing_factor": float(d.get("line_spacing_factor", 1.0)),
        "actual_measurement": float(d.get("actual_measurement") or 0.0),
        "insert_point": p2("insert"),
        "dimstyle": str(d.get("dimstyle", "Standard")),
        "block_name": str(d.get("geometry", "")),
    }


def _as_image_row(entity: Entity) -> dict:
    """Editable IMAGE dictionary -> writer row."""
    d = entity.dxf

    def p3(key, default=(0.0, 0.0, 0.0)):
        value = d.get(key, default)
        return (float(value[0]), float(value[1]), float(value[2]) if len(value) > 2 else 0.0)

    row = {
        "handle": int(entity.handle),
        "image_def": str(d.get("image_def", "")),
        "insert": p3("insert"),
        "u_vector": p3("u_vector", (1.0, 0.0, 0.0)),
        "v_vector": p3("v_vector", (0.0, 1.0, 0.0)),
        "size": (float(d["size"][0]), float(d["size"][1])),
        "display_props": int(d.get("display_props", 3)),
        "clipping": bool(d.get("clipping", False)),
        "brightness": int(d.get("brightness", 50)),
        "contrast": int(d.get("contrast", 50)),
        "fade": int(d.get("fade", 0)),
        "clip_type": int(d.get("clip_type", 1)),
    }
    vertices = d.get("clip_vertices")
    if vertices:
        row["clip_vertices"] = [(float(v[0]), float(v[1])) for v in vertices]
    return row


def _as_viewport_row(entity: Entity) -> dict:
    """Editable VIEWPORT dictionary -> writer row (angles in radians)."""
    d = entity.dxf

    def p3(key, default=(0.0, 0.0, 0.0)):
        value = d.get(key, default)
        return (float(value[0]), float(value[1]), float(value[2]) if len(value) > 2 else 0.0)

    def p2(key, default=(0.0, 0.0)):
        value = d.get(key, default)
        return (float(value[0]), float(value[1]))

    return {
        "handle": int(entity.handle),
        "center": p3("center"),
        "width": float(d["width"]),
        "height": float(d["height"]),
        "view_target": p3("view_target"),
        "view_direction": p3("view_direction", (0.0, 0.0, 1.0)),
        "twist_angle": math.radians(float(d.get("view_twist_angle", 0.0))),
        "view_height": float(d["view_height"]),
        "lens_length": float(d.get("lens_length", 50.0)),
        "front_clip_z": float(d.get("front_clip_z", 0.0)),
        "back_clip_z": float(d.get("back_clip_z", 0.0)),
        "snap_angle": math.radians(float(d.get("snap_angle", 0.0))),
        "view_center": p2("view_center"),
        "snap_base": p2("snap_base"),
        "snap_spacing": p2("snap_spacing", (10.0, 10.0)),
        "grid_spacing": p2("grid_spacing", (10.0, 10.0)),
        "circle_zoom": int(d.get("circle_zoom", 1000)),
        "status_flags": int(d.get("status_flags", 0xC8060)),
        "style_sheet": str(d.get("style_sheet", "")),
        "render_mode": int(d.get("render_mode", 0)),
        "ucs_at_origin": bool(d.get("ucs_at_origin", False)),
        "ucs_per_viewport": bool(d.get("ucs_per_viewport", True)),
        "ucs_origin": p3("ucs_origin"),
        "ucs_x_axis": p3("ucs_x_axis", (1.0, 0.0, 0.0)),
        "ucs_y_axis": p3("ucs_y_axis", (0.0, 1.0, 0.0)),
        "ucs_elevation": float(d.get("ucs_elevation", 0.0)),
        "ucs_ortho_view_type": int(d.get("ucs_ortho_view_type", 0)),
        "frozen_layers": [str(name) for name in d.get("frozen_layers", [])],
    }
