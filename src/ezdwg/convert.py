from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from .document import Document, Layout, read
from .entity import Entity


_POLYLINE_2D_SPLINE_CURVE_TYPES = {"QuadraticBSpline", "CubicBSpline", "Bezier"}


@dataclass(frozen=True)
class ConvertResult:
    source_path: str
    output_path: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]


def to_dxf(
    source: str | Document | Layout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
) -> ConvertResult:
    ezdxf = _require_ezdxf()
    source_path, layout = _resolve_layout(source)

    dxf_doc = ezdxf.new(dxfversion=dxf_version)
    modelspace = dxf_doc.modelspace()

    total = 0
    written = 0
    skipped_by_type: dict[str, int] = {}

    for entity in layout.query(types):
        total += 1
        if _write_entity_to_modelspace(modelspace, entity):
            written += 1
            continue
        skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1

    skipped = total - written
    if strict and skipped > 0:
        summary = ", ".join(
            f"{dxftype}:{count}" for dxftype, count in sorted(skipped_by_type.items())
        )
        raise ValueError(f"failed to convert {skipped} entities ({summary})")

    out_path = Path(output_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    dxf_doc.saveas(str(out_path))

    return ConvertResult(
        source_path=source_path,
        output_path=str(out_path),
        total_entities=total,
        written_entities=written,
        skipped_entities=skipped,
        skipped_by_type=dict(sorted(skipped_by_type.items())),
    )


def _require_ezdxf():
    try:
        import ezdxf
    except Exception as exc:
        raise ImportError(
            "ezdxf is required for DWG->DXF conversion. "
            'Install it with `pip install "ezdwg[dxf]"`.'
        ) from exc
    return ezdxf


def _resolve_layout(source: str | Document | Layout) -> tuple[str, Layout]:
    if isinstance(source, Layout):
        return source.doc.path, source
    if isinstance(source, Document):
        return source.path, source.modelspace()
    doc = read(source)
    return str(source), doc.modelspace()


def _write_entity_to_modelspace(modelspace: Any, entity: Entity) -> bool:
    try:
        return _write_entity_to_modelspace_unsafe(modelspace, entity)
    except Exception:
        return False


def _write_entity_to_modelspace_unsafe(modelspace: Any, entity: Entity) -> bool:
    dxftype = entity.dxftype
    dxf = entity.dxf
    dxfattribs = _entity_dxfattribs(dxf)

    if dxftype == "LINE":
        modelspace.add_line(_point3(dxf.get("start")), _point3(dxf.get("end")), dxfattribs=dxfattribs)
        return True

    if dxftype == "POINT":
        modelspace.add_point(_point3(dxf.get("location")), dxfattribs=dxfattribs)
        return True

    if dxftype == "ARC":
        modelspace.add_arc(
            _point3(dxf.get("center")),
            float(dxf.get("radius", 0.0)),
            float(dxf.get("start_angle", 0.0)),
            float(dxf.get("end_angle", 0.0)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "CIRCLE":
        modelspace.add_circle(
            _point3(dxf.get("center")),
            float(dxf.get("radius", 0.0)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "ELLIPSE":
        modelspace.add_ellipse(
            _point3(dxf.get("center")),
            major_axis=_point3(dxf.get("major_axis")),
            ratio=float(dxf.get("axis_ratio", 1.0)),
            start_param=float(dxf.get("start_angle", 0.0)),
            end_param=float(dxf.get("end_angle", 0.0)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "LWPOLYLINE":
        points = [_point3(point) for point in dxf.get("points", [])]
        if not points:
            return False
        bulges = list(dxf.get("bulges", []) or [])
        widths = list(dxf.get("widths", []) or [])
        vertices = []
        for i, point in enumerate(points):
            start_width = 0.0
            end_width = 0.0
            if i < len(widths):
                width = widths[i]
                if isinstance(width, (list, tuple)) and len(width) >= 2:
                    start_width = float(width[0])
                    end_width = float(width[1])
            bulge = float(bulges[i]) if i < len(bulges) else 0.0
            vertices.append((point[0], point[1], start_width, end_width, bulge))
        lw = modelspace.add_lwpolyline(
            vertices,
            format="xyseb",
            close=bool(dxf.get("closed", False)),
            dxfattribs=dxfattribs,
        )
        const_width = dxf.get("const_width")
        if const_width is not None and len(widths) == 0:
            try:
                lw.dxf.const_width = float(const_width)
            except Exception:
                pass
        return True

    if dxftype == "POLYLINE_2D":
        if _should_write_polyline_2d_as_spline(dxf):
            payload = _polyline_2d_spline_payload(dxf)
            if payload is not None:
                return _write_spline(
                    modelspace,
                    payload,
                    dxfattribs,
                )

        points = [_point3(point) for point in dxf.get("points", [])]
        if not points:
            return False
        bulges = list(dxf.get("bulges", []) or [])
        widths = list(dxf.get("widths", []) or [])
        if len(points) > 1 and points[0] == points[-1]:
            points = points[:-1]
            if bulges:
                bulges = bulges[: len(points)]
            if widths:
                widths = widths[: len(points)]
        vertices = []
        for i, point in enumerate(points):
            start_width = 0.0
            end_width = 0.0
            if i < len(widths):
                width = widths[i]
                if isinstance(width, (list, tuple)) and len(width) >= 2:
                    start_width = float(width[0])
                    end_width = float(width[1])
            bulge = float(bulges[i]) if i < len(bulges) else 0.0
            vertices.append((point[0], point[1], start_width, end_width, bulge))
        modelspace.add_lwpolyline(
            vertices,
            format="xyseb",
            close=bool(dxf.get("closed", False)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "POLYLINE_3D":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 2:
            return False
        modelspace.add_polyline3d(
            points,
            close=bool(dxf.get("closed", False)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "POLYLINE_MESH":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 2:
            return False
        modelspace.add_polyline3d(
            points,
            close=bool(dxf.get("closed", False)),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "POLYLINE_PFACE":
        vertices = [_point3(vertex) for vertex in dxf.get("vertices", [])]
        faces = dxf.get("faces", []) or []
        face_written = False
        for face in faces:
            if not isinstance(face, (list, tuple)):
                continue
            points: list[tuple[float, float, float]] = []
            for raw_index in face:
                try:
                    idx = abs(int(raw_index))
                except Exception:
                    continue
                if idx <= 0:
                    continue
                if idx <= len(vertices):
                    points.append(vertices[idx - 1])
            if len(points) < 3:
                continue
            while len(points) < 4:
                points.append(points[-1])
            modelspace.add_3dface(points[:4], dxfattribs=dxfattribs)
            face_written = True
        if face_written:
            return True
        if len(vertices) >= 2:
            modelspace.add_polyline3d(vertices, close=False, dxfattribs=dxfattribs)
            return True
        return False

    if dxftype == "3DFACE":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 3:
            return False
        while len(points) < 4:
            points.append(points[-1])
        modelspace.add_3dface(points[:4], dxfattribs=dxfattribs)
        return True

    if dxftype == "SOLID":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 3:
            return False
        while len(points) < 4:
            points.append(points[-1])
        modelspace.add_solid(points[:4], dxfattribs=dxfattribs)
        return True

    if dxftype == "TRACE":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 3:
            return False
        while len(points) < 4:
            points.append(points[-1])
        modelspace.add_trace(points[:4], dxfattribs=dxfattribs)
        return True

    if dxftype == "SHAPE":
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "SPLINE":
        return _write_spline(modelspace, dxf, dxfattribs)

    if dxftype in {"TEXT", "ATTRIB", "ATTDEF"}:
        return _write_text_like(modelspace, dxf, dxfattribs)

    if dxftype == "MTEXT":
        return _write_mtext(modelspace, dxf, dxfattribs)

    if dxftype == "LEADER":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 2:
            return False
        modelspace.add_polyline3d(points, close=False, dxfattribs=dxfattribs)
        return True

    if dxftype == "HATCH":
        return _write_hatch(modelspace, dxf, dxfattribs)

    if dxftype == "TOLERANCE":
        return _write_text_like(modelspace, dxf, dxfattribs)

    if dxftype == "MLINE":
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 2:
            return False
        modelspace.add_mline(points, close=bool(dxf.get("closed", False)), dxfattribs=dxfattribs)
        return True

    if dxftype == "MINSERT":
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "INSERT":
        name = dxf.get("name")
        if isinstance(name, str) and name:
            insert = _point3(dxf.get("insert"))
            try:
                ref = modelspace.add_blockref(name, insert, dxfattribs=dxfattribs)
                ref.dxf.xscale = float(dxf.get("xscale", 1.0))
                ref.dxf.yscale = float(dxf.get("yscale", 1.0))
                ref.dxf.zscale = float(dxf.get("zscale", 1.0))
                ref.dxf.rotation = float(dxf.get("rotation", 0.0))
                return True
            except Exception:
                # Block definitions are not exported yet. Keep insert location visible.
                pass
        # Block name is absent or unresolved block definition is unavailable.
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "DIMENSION":
        return _write_dimension_native(modelspace, dxf, dxfattribs)

    return False


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


def _write_spline(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    fit_points = [_point3(point) for point in dxf.get("fit_points", [])]
    if len(fit_points) >= 2:
        closed = bool(dxf.get("closed", False))
        degree = max(2, int(dxf.get("degree", 3)))
        fit_tangents = [_point3(point) for point in list(dxf.get("fit_tangents") or [])]
        if len(fit_tangents) >= 2 and not closed:
            modelspace.add_cad_spline_control_frame(
                fit_points=fit_points,
                tangents=[fit_tangents[0], fit_tangents[-1]],
                dxfattribs=dxfattribs,
            )
            return True

        if closed:
            closed_fit_points = list(fit_points)
            if len(closed_fit_points) > 1 and closed_fit_points[0] == closed_fit_points[-1]:
                closed_fit_points = closed_fit_points[:-1]
            if len(closed_fit_points) >= max(3, degree + 1):
                try:
                    spline = modelspace.add_spline_control_frame(
                        fit_points=closed_fit_points,
                        degree=degree,
                        method="chord",
                        dxfattribs=dxfattribs,
                    )
                    spline.set_flag_state(spline.CLOSED, True)
                    spline.set_flag_state(spline.PERIODIC, True)
                    return True
                except Exception:
                    pass

            if fit_points[0] != fit_points[-1]:
                fit_points = [*fit_points, fit_points[0]]

        spline = modelspace.add_spline(fit_points=fit_points, degree=degree, dxfattribs=dxfattribs)
        if closed:
            spline.set_flag_state(spline.CLOSED, True)
            spline.set_flag_state(spline.PERIODIC, True)
        return True

    control_points = [_point3(point) for point in dxf.get("control_points", [])]
    if len(control_points) < 2:
        points = [_point3(point) for point in dxf.get("points", [])]
        if len(points) < 2:
            return False
        modelspace.add_polyline3d(points, close=bool(dxf.get("closed", False)), dxfattribs=dxfattribs)
        return True

    closed = bool(dxf.get("closed", False))
    if len(control_points) > 1 and control_points[0] == control_points[-1]:
        control_points = control_points[:-1]

    degree = max(2, int(dxf.get("degree", 3)))
    knots = [float(v) for v in dxf.get("knots", [])]
    weights = [float(v) for v in dxf.get("weights", [])]
    rational = bool(dxf.get("rational", False))

    if closed and len(control_points) >= 3:
        spline = modelspace.add_spline(dxfattribs=dxfattribs)
        if rational and len(weights) == len(control_points) and len(weights) > 0:
            spline.set_closed_rational(control_points, weights, degree=degree)
        else:
            spline.set_closed(control_points, degree=degree)
        return True

    if rational and len(weights) == len(control_points) and len(weights) > 0:
        modelspace.add_rational_spline(
            control_points=control_points,
            weights=weights,
            degree=degree,
            knots=knots if knots else None,
            dxfattribs=dxfattribs,
        )
        return True

    modelspace.add_open_spline(
        control_points=control_points,
        degree=degree,
        knots=knots if knots else None,
        dxfattribs=dxfattribs,
    )
    return True


def _write_text_like(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    text = str(dxf.get("text", "") or "")
    if text == "":
        return False
    height = dxf.get("height")
    rotation = dxf.get("rotation")
    text_entity = modelspace.add_text(
        text,
        height=float(height) if height is not None else None,
        rotation=float(rotation) if rotation is not None else None,
        dxfattribs=dxfattribs,
    )
    text_entity.dxf.insert = _point3(dxf.get("insert"))
    return True


def _write_mtext(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    text = str(dxf.get("raw_text") or dxf.get("text") or "")
    if text == "":
        return False
    mtext = modelspace.add_mtext(text, dxfattribs=dxfattribs)
    mtext.set_location(_point3(dxf.get("insert")))
    char_height = dxf.get("char_height")
    if char_height is not None:
        try:
            mtext.dxf.char_height = float(char_height)
        except Exception:
            pass
    return True


def _write_hatch(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    paths = dxf.get("paths", []) or []
    if len(paths) == 0:
        return False

    color = _to_valid_aci(dxf.get("resolved_color_index"))
    if color is None:
        color = _to_valid_aci(dxf.get("color_index"))
    if color is None:
        color = 7

    hatch = modelspace.add_hatch(color=color, dxfattribs=dxfattribs)
    if bool(dxf.get("solid_fill", False)):
        rgb = _to_rgb(_to_valid_true_color(dxf.get("resolved_true_color")))
        hatch.set_solid_fill(color=color, rgb=rgb)
    else:
        pattern_name = str(dxf.get("pattern_name") or "ANSI31")
        hatch.set_pattern_fill(pattern_name, color=color)

    path_written = False
    for path in paths:
        if not isinstance(path, dict):
            continue
        points = path.get("points", []) or []
        xy = [(float(point[0]), float(point[1])) for point in points if len(point) >= 2]
        if len(xy) < 2:
            continue
        hatch.paths.add_polyline_path(xy, is_closed=bool(path.get("closed", False)))
        path_written = True
    return path_written


def _write_dimension_native(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    dimtype = str(dxf.get("dimtype") or "").upper()
    text = _dimension_text(dxf.get("text"))
    text_mid = _point2_or_none(dxf.get("text_midpoint"))

    try:
        if dimtype == "LINEAR":
            dim = modelspace.add_linear_dim(
                base=_point2(dxf.get("defpoint")),
                p1=_point2(dxf.get("defpoint2")),
                p2=_point2(dxf.get("defpoint3")),
                location=text_mid,
                text=text,
                angle=float(dxf.get("angle", 0.0)),
                text_rotation=_float_or_none(dxf.get("text_rotation")),
                dxfattribs=dxfattribs,
            )
            dim.render()
            return True

        if dimtype == "ALIGNED":
            p1 = _point2(dxf.get("defpoint2"))
            p2 = _point2(dxf.get("defpoint3"))
            base = _point2(dxf.get("defpoint"))
            distance = _signed_line_distance_2d(base, p1, p2)
            dim = modelspace.add_aligned_dim(
                p1=p1,
                p2=p2,
                distance=distance,
                text=text,
                dxfattribs=dxfattribs,
            )
            if text_mid is not None:
                dim.set_location(text_mid, leader=False, relative=False)
            dim.render()
            return True

        if dimtype == "RADIUS":
            center = _point2(dxf.get("defpoint2"))
            mpoint = _point2_or_none(dxf.get("defpoint3"))
            dim = modelspace.add_radius_dim(
                center=center,
                mpoint=mpoint,
                text=text,
                dxfattribs=dxfattribs,
            )
            if text_mid is not None:
                dim.set_location(text_mid, leader=False, relative=False)
            dim.render()
            return True

        if dimtype == "DIAMETER":
            center = _point2(dxf.get("defpoint2"))
            mpoint = _point2_or_none(dxf.get("defpoint3"))
            dim = modelspace.add_diameter_dim(
                center=center,
                mpoint=mpoint,
                text=text,
                dxfattribs=dxfattribs,
            )
            if text_mid is not None:
                dim.set_location(text_mid, leader=False, relative=False)
            dim.render()
            return True

        if dimtype == "ORDINATE":
            feature = _point2(dxf.get("defpoint2"))
            offset = _point2(dxf.get("defpoint3"))
            origin = _point2_or_none(dxf.get("defpoint")) or (0.0, 0.0)
            dim = modelspace.add_ordinate_dim(
                feature_location=feature,
                offset=offset,
                dtype=_ordinate_dim_type(feature, offset),
                origin=origin,
                rotation=float(dxf.get("angle", 0.0)),
                text=text,
                dxfattribs=dxfattribs,
            )
            dim.render()
            return True
    except Exception:
        # Keep conversion robust and avoid generating synthetic geometry lines.
        pass

    return _write_dimension_text_fallback(modelspace, dxf, dxfattribs)


def _write_dimension_text_fallback(
    modelspace: Any,
    dxf: dict[str, Any],
    dxfattribs: dict[str, Any],
) -> bool:
    text_mid = dxf.get("text_midpoint")
    if text_mid is None:
        return False
    text = _dimension_text(dxf.get("text"))
    if text in {"", "<>"}:
        return False
    text_entity = modelspace.add_text(text, dxfattribs=dxfattribs)
    text_entity.dxf.insert = _point3(text_mid)
    return True


def _entity_dxfattribs(dxf: dict[str, Any]) -> dict[str, Any]:
    attribs: dict[str, Any] = {}
    color = _to_valid_aci(dxf.get("resolved_color_index"))
    if color is None:
        color = _to_valid_aci(dxf.get("color_index"))
    if color is not None:
        attribs["color"] = color

    true_color = _to_valid_true_color(dxf.get("resolved_true_color"))
    if true_color is None:
        true_color = _to_valid_true_color(dxf.get("true_color"))
    if true_color is not None:
        attribs["true_color"] = true_color
    return attribs


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


def _point3(value: Any) -> tuple[float, float, float]:
    if value is None:
        return (0.0, 0.0, 0.0)
    if isinstance(value, (list, tuple)):
        if len(value) >= 3:
            return (float(value[0]), float(value[1]), float(value[2]))
        if len(value) >= 2:
            return (float(value[0]), float(value[1]), 0.0)
    raise ValueError(f"invalid point value: {value!r}")


def _point2(value: Any) -> tuple[float, float]:
    if value is None:
        raise ValueError("invalid point value: None")
    if isinstance(value, (list, tuple)):
        if len(value) >= 2:
            return (float(value[0]), float(value[1]))
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
