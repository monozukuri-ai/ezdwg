from __future__ import annotations

from typing import Iterable, Any


def plot(
    target: Any,
    types: str | Iterable[str] | None = None,
    ax: Any | None = None,
    show: bool = True,
    equal: bool = True,
    title: str | None = None,
    line_width: float = 1.0,
    arc_segments: int = 64,
    auto_fit: bool = True,
    fit_margin: float = 0.04,
    dimension_color: Any | None = "black",
):
    layout = _resolve_layout(target)
    return plot_layout(
        layout,
        types=types,
        ax=ax,
        show=show,
        equal=equal,
        title=title,
        line_width=line_width,
        arc_segments=arc_segments,
        auto_fit=auto_fit,
        fit_margin=fit_margin,
        dimension_color=dimension_color,
    )


def plot_layout(
    layout: Any,
    types: str | Iterable[str] | None = None,
    ax: Any | None = None,
    show: bool = True,
    equal: bool = True,
    title: str | None = None,
    line_width: float = 1.0,
    arc_segments: int = 64,
    auto_fit: bool = True,
    fit_margin: float = 0.04,
    dimension_color: Any | None = "black",
):
    plt = _require_matplotlib()
    if ax is None:
        _, ax = plt.subplots()

    for entity in layout.query(types):
        color = _resolve_dwg_color(entity.dxf)
        if color is None:
            color = "#000000"
        dxftype = entity.dxftype
        if dxftype == "LINE":
            _draw_line(ax, entity.dxf["start"], entity.dxf["end"], line_width, color=color)
        elif dxftype == "POINT":
            _draw_point(ax, entity.dxf["location"], line_width, color=color)
        elif dxftype == "LWPOLYLINE":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                bulges=entity.dxf.get("bulges"),
                closed=bool(entity.dxf.get("closed", False)),
                arc_segments=arc_segments,
            )
        elif dxftype == "POLYLINE_2D":
            interpolation_applied = bool(entity.dxf.get("interpolation_applied", False))
            draw_points = (
                entity.dxf.get("interpolated_points", [])
                if interpolation_applied
                else entity.dxf.get("points", [])
            )
            draw_bulges = None if interpolation_applied else entity.dxf.get("bulges")
            _draw_polyline(
                ax,
                draw_points,
                line_width,
                color=color,
                bulges=draw_bulges,
                closed=bool(entity.dxf.get("closed", False)),
                arc_segments=arc_segments,
            )
        elif dxftype == "POLYLINE_3D":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                closed=bool(entity.dxf.get("closed", False)),
                arc_segments=arc_segments,
            )
        elif dxftype == "POLYLINE_MESH":
            _draw_polyline_mesh(
                ax,
                entity.dxf.get("points", []),
                entity.dxf.get("m_vertex_count", 0),
                entity.dxf.get("n_vertex_count", 0),
                bool(entity.dxf.get("closed", False)),
                line_width,
                color=color,
                arc_segments=arc_segments,
            )
        elif dxftype == "POLYLINE_PFACE":
            _draw_polyline_pface(
                ax,
                entity.dxf.get("vertices", []),
                entity.dxf.get("faces", []),
                line_width,
                color=color,
                arc_segments=arc_segments,
            )
        elif dxftype == "3DFACE":
            _draw_3dface(
                ax,
                entity.dxf.get("points", []),
                int(entity.dxf.get("invisible_edge_flags", 0)),
                line_width,
                color=color,
            )
        elif dxftype == "SOLID" or dxftype == "TRACE":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                closed=True,
                arc_segments=arc_segments,
            )
        elif dxftype == "SHAPE":
            _draw_point(ax, entity.dxf.get("insert", (0.0, 0.0, 0.0)), line_width, color=color)
        elif dxftype == "ARC":
            _draw_arc(
                ax,
                entity.dxf["center"],
                entity.dxf["radius"],
                entity.dxf["start_angle"],
                entity.dxf["end_angle"],
                arc_segments,
                line_width,
                color=color,
            )
        elif dxftype == "CIRCLE":
            _draw_circle(
                ax,
                entity.dxf["center"],
                entity.dxf["radius"],
                arc_segments,
                line_width,
                color=color,
            )
        elif dxftype == "ELLIPSE":
            _draw_ellipse(
                ax,
                entity.dxf["center"],
                entity.dxf["major_axis"],
                entity.dxf["axis_ratio"],
                entity.dxf["start_angle"],
                entity.dxf["end_angle"],
                arc_segments,
                line_width,
                color=color,
            )
        elif dxftype == "SPLINE":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                closed=bool(entity.dxf.get("closed", False)),
                arc_segments=arc_segments,
            )
        elif dxftype == "TEXT":
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                entity.dxf.get("height", 1.0),
                entity.dxf.get("rotation", 0.0),
                color=color,
            )
        elif dxftype == "ATTRIB" or dxftype == "ATTDEF":
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                entity.dxf.get("height", 1.0),
                entity.dxf.get("rotation", 0.0),
                color=color,
            )
        elif dxftype == "MTEXT":
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                entity.dxf.get("char_height", 1.0),
                entity.dxf.get("rotation", 0.0),
                color=color,
                background=_resolve_mtext_background_bbox(ax, entity.dxf),
            )
        elif dxftype == "LEADER":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                closed=False,
                arc_segments=arc_segments,
            )
        elif dxftype == "HATCH":
            for path in entity.dxf.get("paths", []):
                points = path.get("points", []) if isinstance(path, dict) else []
                closed = bool(path.get("closed", False)) if isinstance(path, dict) else False
                _draw_polyline(
                    ax,
                    points,
                    line_width,
                    color=color,
                    closed=closed,
                    arc_segments=arc_segments,
                )
        elif dxftype == "TOLERANCE":
            text_height = entity.dxf.get("height", 1.0)
            try:
                text_height = float(text_height)
            except Exception:
                text_height = 1.0
            if text_height <= 0.0:
                text_height = 1.0
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                text_height,
                entity.dxf.get("rotation", 0.0),
                color=color,
            )
        elif dxftype == "MLINE":
            _draw_polyline(
                ax,
                entity.dxf.get("points", []),
                line_width,
                color=color,
                closed=bool(entity.dxf.get("closed", False)),
                arc_segments=arc_segments,
            )
        elif dxftype == "INSERT":
            _draw_point(ax, entity.dxf.get("insert", (0.0, 0.0, 0.0)), line_width, color=color)
        elif dxftype == "MINSERT":
            _draw_point(ax, entity.dxf.get("insert", (0.0, 0.0, 0.0)), line_width, color=color)
        elif dxftype == "DIMENSION":
            dim_color = color if dimension_color is None else dimension_color
            _draw_dimension(ax, entity.dxf, line_width, color=dim_color)

    if title:
        ax.set_title(title)
    if auto_fit:
        _apply_auto_limits(ax, equal=equal, margin=fit_margin)
    else:
        ax.autoscale(True)
        if equal:
            _apply_equal_limits(ax)
            ax.set_aspect("equal", adjustable="box")
    if show:
        plt.show()
    return ax


def _require_matplotlib():
    try:
        import matplotlib.pyplot as plt
    except Exception as exc:
        raise ImportError(
            "matplotlib is required for plotting. "
            "Install it with `pip install matplotlib`."
        ) from exc
    return plt


def _resolve_layout(target: Any):
    if hasattr(target, "query"):
        return target
    if hasattr(target, "modelspace"):
        return target.modelspace()
    if isinstance(target, str):
        from .document import read

        return read(target).modelspace()
    raise TypeError("plot() expects a path, Document, or Layout")


def _resolve_dwg_color(dxf):
    true_color = dxf.get("resolved_true_color")
    if true_color is None:
        true_color = dxf.get("true_color")
    color = _true_color_to_hex(true_color)
    if color is not None:
        return color

    index = dxf.get("resolved_color_index")
    if index is None:
        index = dxf.get("color_index")
    if index is not None:
        try:
            aci = int(index)
        except Exception:
            aci = None
        if aci is not None and aci not in (0, 256, 257):
            mapped = _aci_to_hex(aci)
            if mapped is not None:
                return mapped

    return None


def _true_color_to_hex(value):
    if value is None:
        return None
    try:
        raw = int(value) & 0xFFFFFF
    except Exception:
        return None
    return f"#{(raw >> 16) & 0xFF:02x}{(raw >> 8) & 0xFF:02x}{raw & 0xFF:02x}"


def _aci_to_hex(index: int):
    if index <= 0:
        return None
    base = {
        1: (255, 0, 0),
        2: (255, 255, 0),
        3: (0, 255, 0),
        4: (0, 255, 255),
        5: (0, 0, 255),
        6: (255, 0, 255),
        # ACI 7 is white/black depending on background. Use black for
        # matplotlib's default light background so geometry stays visible.
        7: (0, 0, 0),
        8: (128, 128, 128),
        9: (192, 192, 192),
    }
    rgb = base.get(index)
    if rgb is None:
        rgb = _aci_approx_rgb(index)
    if rgb is None:
        return None
    return f"#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}"


def _aci_approx_rgb(index: int):
    import colorsys

    if 10 <= index <= 249:
        step = index - 10
        hue = (step % 24) / 24.0
        band = step // 24
        sat = 1.0 if band < 5 else 0.7
        val = max(0.28, 1.0 - band * 0.08)
        r, g, b = colorsys.hsv_to_rgb(hue, sat, val)
        return (int(round(r * 255)), int(round(g * 255)), int(round(b * 255)))
    if 250 <= index <= 255:
        t = (index - 250) / 5.0
        gray = int(round(255 * t))
        return (gray, gray, gray)
    return None


def _draw_line(ax, start, end, line_width: float, color=None):
    ax.plot([start[0], end[0]], [start[1], end[1]], linewidth=line_width, color=color)


def _draw_point(ax, location, line_width: float, color=None):
    size = max(2.0, line_width * 4.0)
    ax.plot([location[0]], [location[1]], marker="o", markersize=size, linewidth=0, color=color)


def _draw_polyline(
    ax,
    points,
    line_width: float,
    color=None,
    bulges=None,
    closed: bool = False,
    arc_segments: int = 64,
):
    if not points:
        return
    path = _build_lwpolyline_path(points, bulges=bulges, closed=closed, arc_segments=arc_segments)
    if not path:
        return
    xs = [pt[0] for pt in path]
    ys = [pt[1] for pt in path]
    ax.plot(xs, ys, linewidth=line_width, color=color)


def _draw_polyline_mesh(
    ax,
    points,
    m_vertex_count,
    n_vertex_count,
    closed: bool,
    line_width: float,
    color=None,
    arc_segments: int = 64,
):
    try:
        m_count = int(m_vertex_count)
    except Exception:
        m_count = 0
    try:
        n_count = int(n_vertex_count)
    except Exception:
        n_count = 0

    points3d = [pt for pt in points if _to_xy(pt) is not None]
    if m_count <= 1 or n_count <= 1 or len(points3d) < (m_count * n_count):
        _draw_polyline(
            ax,
            points3d,
            line_width,
            color=color,
            closed=closed,
            arc_segments=arc_segments,
        )
        return

    for row in range(n_count):
        start = row * m_count
        row_points = points3d[start : start + m_count]
        if len(row_points) >= 2:
            _draw_polyline(
                ax,
                row_points,
                line_width,
                color=color,
                closed=False,
                arc_segments=arc_segments,
            )

    for col in range(m_count):
        col_points = []
        for row in range(n_count):
            idx = row * m_count + col
            if idx < len(points3d):
                col_points.append(points3d[idx])
        if len(col_points) >= 2:
            _draw_polyline(
                ax,
                col_points,
                line_width,
                color=color,
                closed=False,
                arc_segments=arc_segments,
            )


def _draw_polyline_pface(
    ax,
    vertices,
    faces,
    line_width: float,
    color=None,
    arc_segments: int = 64,
):
    vertex_points = [pt for pt in vertices if _to_xy(pt) is not None]
    if not vertex_points:
        return

    face_drawn = False
    for face in faces or []:
        if not isinstance(face, (tuple, list)) or not face:
            continue
        face_points = []
        for raw_index in face:
            try:
                index = abs(int(raw_index))
            except Exception:
                continue
            if index <= 0:
                continue
            idx0 = index - 1
            if 0 <= idx0 < len(vertex_points):
                face_points.append(vertex_points[idx0])
        if len(face_points) >= 2:
            _draw_polyline(
                ax,
                face_points,
                line_width,
                color=color,
                closed=True,
                arc_segments=arc_segments,
            )
            face_drawn = True

    if not face_drawn:
        _draw_polyline(
            ax,
            vertex_points,
            line_width,
            color=color,
            closed=False,
            arc_segments=arc_segments,
        )


def _draw_3dface(ax, points, invisible_edge_flags: int, line_width: float, color=None):
    pts = list(points)[:4]
    if len(pts) < 3:
        return
    while len(pts) < 4:
        pts.append(pts[-1])

    edges = ((0, 1, 1), (1, 2, 2), (2, 3, 4), (3, 0, 8))
    for start_i, end_i, mask in edges:
        if invisible_edge_flags & mask:
            continue
        p1 = pts[start_i]
        p2 = pts[end_i]
        if p1 == p2:
            continue
        _draw_line(ax, p1, p2, line_width, color=color)


def _build_lwpolyline_path(points, bulges=None, closed: bool = False, arc_segments: int = 64):
    points2d = []
    for point in points:
        xy = _to_xy(point)
        if xy is not None:
            points2d.append(xy)
    count = len(points2d)
    if count == 0:
        return []
    if count == 1:
        return [points2d[0]]

    bulge_values = [0.0] * count
    if bulges:
        for idx, value in enumerate(list(bulges)[:count]):
            try:
                bulge_values[idx] = float(value)
            except Exception:
                bulge_values[idx] = 0.0

    seg_count = count if closed else (count - 1)
    path: list[tuple[float, float]] = []
    for idx in range(seg_count):
        start = points2d[idx]
        end = points2d[(idx + 1) % count]
        bulge = bulge_values[idx]
        segment = _segment_path_with_bulge(start, end, bulge, arc_segments=arc_segments)
        if not segment:
            continue
        if path:
            path.extend(segment[1:])
        else:
            path.extend(segment)
    return path


def _segment_path_with_bulge(start, end, bulge: float, arc_segments: int):
    import math

    if abs(bulge) <= 1.0e-12:
        return [start, end]

    dx = end[0] - start[0]
    dy = end[1] - start[1]
    chord = math.hypot(dx, dy)
    if chord <= 1.0e-12:
        return [start, end]

    theta = 4.0 * math.atan(bulge)
    if abs(theta) <= 1.0e-12:
        return [start, end]

    normal = (-dy / chord, dx / chord)
    center_offset = chord * (1.0 - bulge * bulge) / (4.0 * bulge)
    mid = ((start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5)
    center = (
        mid[0] + normal[0] * center_offset,
        mid[1] + normal[1] * center_offset,
    )
    radius = math.hypot(start[0] - center[0], start[1] - center[1])
    if radius <= 1.0e-12:
        return [start, end]

    start_angle = math.atan2(start[1] - center[1], start[0] - center[0])
    segments = max(2, int(math.ceil(abs(theta) * max(8, arc_segments) / (2.0 * math.pi))))
    out = []
    for i in range(segments + 1):
        t = i / segments
        angle = start_angle + theta * t
        out.append((center[0] + radius * math.cos(angle), center[1] + radius * math.sin(angle)))
    out[0] = start
    out[-1] = end
    return out


def _draw_arc(
    ax,
    center,
    radius: float,
    start_angle: float,
    end_angle: float,
    segments: int,
    line_width: float,
    color=None,
):
    import math

    if segments < 4:
        segments = 4
    start = start_angle
    end = end_angle
    if end < start:
        end += 360.0
    step = (end - start) / segments

    xs = []
    ys = []
    for i in range(segments + 1):
        angle = math.radians(start + step * i)
        xs.append(center[0] + radius * math.cos(angle))
        ys.append(center[1] + radius * math.sin(angle))
    ax.plot(xs, ys, linewidth=line_width, color=color)


def _draw_circle(ax, center, radius: float, segments: int, line_width: float, color=None):
    _draw_arc(ax, center, radius, 0.0, 360.0, segments, line_width, color=color)


def _draw_ellipse(
    ax,
    center,
    major_axis,
    axis_ratio: float,
    start_angle: float,
    end_angle: float,
    segments: int,
    line_width: float,
    color=None,
):
    import math

    if segments < 16:
        segments = 16

    start = start_angle
    end = end_angle
    if end < start:
        end += math.tau

    mx = major_axis[0]
    my = major_axis[1]
    vx = -my * axis_ratio
    vy = mx * axis_ratio

    step = (end - start) / segments
    xs = []
    ys = []
    for i in range(segments + 1):
        t = start + step * i
        c = math.cos(t)
        s = math.sin(t)
        xs.append(center[0] + mx * c + vx * s)
        ys.append(center[1] + my * c + vy * s)
    ax.plot(xs, ys, linewidth=line_width, color=color)


def _draw_text(
    ax,
    insert,
    text: str,
    height: float,
    rotation_deg: float,
    color=None,
    background=None,
):
    if not text:
        return
    text = text.replace("\\P", "\n")
    size = max(6.0, abs(height) * 3.0)
    kwargs = {}
    if background is not None:
        kwargs["bbox"] = background
    ax.text(
        insert[0],
        insert[1],
        text,
        fontsize=size,
        rotation=rotation_deg,
        color=color,
        **kwargs,
    )


def _resolve_mtext_background_bbox(ax, dxf):
    flags = _as_int(dxf.get("background_flags"), default=0)
    if (flags & 0x01) == 0 and (flags & 0x02) == 0 and (flags & 0x10) == 0:
        return None

    bg_true_color = dxf.get("background_true_color")
    bg_color = _true_color_to_hex(bg_true_color)
    if bg_color is None:
        bg_index = _as_int(dxf.get("background_color_index"), default=0)
        if bg_index not in (0, 256, 257):
            bg_color = _aci_to_hex(bg_index)
    if bg_color is None and (flags & 0x02) != 0:
        try:
            bg_color = ax.get_facecolor()
        except Exception:
            bg_color = None
    if bg_color is None:
        bg_color = "#ffffff"

    alpha = _resolve_mtext_background_alpha(dxf.get("background_transparency"))
    style = {
        "facecolor": bg_color,
        "edgecolor": "none",
        "boxstyle": "square,pad=0.15",
    }
    if alpha is not None:
        style["alpha"] = alpha
    return style


def _resolve_mtext_background_alpha(value):
    if value is None:
        return None
    try:
        raw = int(value)
    except Exception:
        return None

    alpha_code = raw & 0xFF
    if alpha_code <= 0:
        return 1.0
    if alpha_code >= 255:
        return 0.0
    return max(0.0, min(1.0, 1.0 - (alpha_code / 255.0)))


def _draw_dimension(ax, dxf, line_width: float, color=None):
    dimtype = str(_dimension_value(dxf, "dimtype", "LINEAR")).upper()
    p13 = _safe_point(dxf.get("defpoint2"))
    p14 = _safe_point(dxf.get("defpoint3"))
    p10 = _safe_point(dxf.get("defpoint"))
    text_mid = _safe_point(_dimension_value(dxf, "text_midpoint"))
    insert = _safe_point(_dimension_value(dxf, "insert"))
    text = _dimension_value(dxf, "text", "")

    if p13 is None or p14 is None:
        return

    if dimtype == "DIAMETER":
        _draw_dimension_diameter(
            ax,
            dxf,
            p13=p13,
            p14=p14,
            text_mid=text_mid,
            line_width=line_width,
            color=color,
        )
        return

    if dimtype == "RADIUS":
        _draw_dimension_radius(
            ax,
            dxf,
            p13=p13,
            p14=p14,
            text_mid=text_mid,
            line_width=line_width,
            color=color,
        )
        return

    if p10 is None:
        p10 = _midpoint(p13, p14)

    dim_dir = _direction_from_angle(_dimension_value(dxf, "angle"))
    if dim_dir is None:
        dim_dir = _normalize2((p14[0] - p13[0], p14[1] - p13[1]))
    if dim_dir is None:
        return

    normal = (-dim_dir[1], dim_dir[0])
    oblique_deg = _dimension_value(dxf, "oblique_angle", 0.0) or 0.0
    ext_dir = _rotate2(normal, _deg_to_rad(oblique_deg))
    ext_dir = _normalize2(ext_dir) or normal

    i13 = _line_line_intersection_2d((p13[0], p13[1]), ext_dir, (p10[0], p10[1]), dim_dir)
    i14 = _line_line_intersection_2d((p14[0], p14[1]), ext_dir, (p10[0], p10[1]), dim_dir)
    if i13 is None:
        i13 = _project_to_line_2d((p13[0], p13[1]), (p10[0], p10[1]), dim_dir)
    if i14 is None:
        i14 = _project_to_line_2d((p14[0], p14[1]), (p10[0], p10[1]), dim_dir)
    if i13 is None or i14 is None:
        return

    dim_len = _distance2(i13, i14)
    ext_over = max(0.2, dim_len * 0.03)
    ext_w = max(0.5, line_width * 0.8)

    e13 = _extension_endpoint((p13[0], p13[1]), i13, ext_over, ext_dir)
    e14 = _extension_endpoint((p14[0], p14[1]), i14, ext_over, ext_dir)
    ax.plot([p13[0], e13[0]], [p13[1], e13[1]], linewidth=ext_w, color=color)
    ax.plot([p14[0], e14[0]], [p14[1], e14[1]], linewidth=ext_w, color=color)
    ax.plot([i13[0], i14[0]], [i13[1], i14[1]], linewidth=line_width, color=color)
    _draw_dim_ticks(ax, i13, i14, dim_dir, normal, dim_len, line_width, color=color)

    text = _resolve_dimension_text(dxf, text)

    text_pos = text_mid
    if _is_origin_point(text_pos) and _is_origin_point(insert):
        offset = max(0.5, dim_len * 0.05)
        text_pos = (
            (i13[0] + i14[0]) * 0.5 + normal[0] * offset,
            (i13[1] + i14[1]) * 0.5 + normal[1] * offset,
            0.0,
        )

    if text and text_pos is not None:
        height = _dimension_value(dxf, "char_height") or _dimension_value(
            dxf, "height"
        ) or max(0.8, dim_len * 0.06)
        rotation = _dimension_value(dxf, "text_rotation")
        if rotation is None:
            rotation = _dimension_value(dxf, "angle", 0.0)
        _draw_text(ax, text_pos, text, height, rotation, color=color)


def _draw_dimension_diameter(ax, dxf, p13, p14, text_mid, line_width: float, color=None):
    axis = _normalize2((p14[0] - p13[0], p14[1] - p13[1]))
    if axis is None:
        return
    normal = (-axis[1], axis[0])
    dim_len = _distance2((p13[0], p13[1]), (p14[0], p14[1]))
    if dim_len <= 1.0e-12:
        return

    ax.plot([p13[0], p14[0]], [p13[1], p14[1]], linewidth=line_width, color=color)
    _draw_dim_ticks(
        ax,
        (p13[0], p13[1]),
        (p14[0], p14[1]),
        axis,
        normal,
        dim_len,
        line_width,
        color=color,
    )

    text = _resolve_dimension_text(dxf, _dimension_value(dxf, "text", ""))
    if not text:
        return

    if _is_origin_point(text_mid):
        offset = max(0.5, dim_len * 0.06)
        text_mid = (
            (p13[0] + p14[0]) * 0.5 + normal[0] * offset,
            (p13[1] + p14[1]) * 0.5 + normal[1] * offset,
            0.0,
        )
    if text_mid is None:
        return

    height = _dimension_value(dxf, "char_height") or _dimension_value(dxf, "height") or max(
        0.8, dim_len * 0.06
    )
    rotation = _dimension_value(dxf, "text_rotation")
    if rotation is None:
        rotation = _dimension_value(dxf, "angle", 0.0)
    _draw_text(ax, text_mid, text, height, rotation, color=color)


def _draw_dimension_radius(ax, dxf, p13, p14, text_mid, line_width: float, color=None):
    axis = _normalize2((p14[0] - p13[0], p14[1] - p13[1]))
    if axis is None:
        return
    normal = (-axis[1], axis[0])
    dim_len = _distance2((p13[0], p13[1]), (p14[0], p14[1]))
    if dim_len <= 1.0e-12:
        return

    ax.plot([p13[0], p14[0]], [p13[1], p14[1]], linewidth=line_width, color=color)
    _draw_dim_single_tick(ax, (p14[0], p14[1]), axis, normal, dim_len, line_width, color=color)

    text = _resolve_dimension_text(dxf, _dimension_value(dxf, "text", ""))
    if not text:
        return

    if _is_origin_point(text_mid):
        offset = max(0.5, dim_len * 0.06)
        text_mid = (
            (p13[0] + p14[0]) * 0.5 + normal[0] * offset,
            (p13[1] + p14[1]) * 0.5 + normal[1] * offset,
            0.0,
        )
    if text_mid is None:
        return

    height = _dimension_value(dxf, "char_height") or _dimension_value(dxf, "height") or max(
        0.8, dim_len * 0.06
    )
    rotation = _dimension_value(dxf, "text_rotation")
    if rotation is None:
        rotation = _dimension_value(dxf, "angle", 0.0)
    _draw_text(ax, text_mid, text, height, rotation, color=color)


def _apply_equal_limits(ax):
    x0, x1 = ax.get_xlim()
    y0, y1 = ax.get_ylim()
    dx = x1 - x0
    dy = y1 - y0
    if dx <= 0 or dy <= 0:
        return
    span = max(dx, dy)
    cx = (x0 + x1) * 0.5
    cy = (y0 + y1) * 0.5
    half = span * 0.5
    ax.set_xlim(cx - half, cx + half)
    ax.set_ylim(cy - half, cy + half)


def _apply_auto_limits(ax, equal: bool, margin: float):
    points = _collect_axes_points(ax)
    if not points:
        ax.autoscale(True)
        if equal:
            _apply_equal_limits(ax)
            ax.set_aspect("equal", adjustable="box")
        return

    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    full = _bounds_from_xy(xs, ys)
    robust = _robust_bounds(xs, ys, q_low=0.02, q_high=0.98)
    chosen = _choose_bounds(full, robust)
    if chosen is None:
        chosen = full
    if chosen is None:
        ax.autoscale(True)
        return

    x0, x1, y0, y1 = _expand_bounds(chosen, margin=margin)
    if equal:
        x0, x1, y0, y1 = _square_bounds(x0, x1, y0, y1)
        ax.set_aspect("equal", adjustable="box")
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)


def _collect_axes_points(ax):
    import math

    points = []
    for line in ax.lines:
        xs = line.get_xdata()
        ys = line.get_ydata()
        for x, y in zip(xs, ys):
            try:
                xf = float(x)
                yf = float(y)
            except Exception:
                continue
            if math.isfinite(xf) and math.isfinite(yf):
                points.append((xf, yf))
    for text in ax.texts:
        x, y = text.get_position()
        try:
            xf = float(x)
            yf = float(y)
        except Exception:
            continue
        if math.isfinite(xf) and math.isfinite(yf):
            points.append((xf, yf))
    return points


def _bounds_from_xy(xs, ys):
    if not xs or not ys:
        return None
    return (min(xs), max(xs), min(ys), max(ys))


def _robust_bounds(xs, ys, q_low: float, q_high: float):
    if len(xs) < 16 or len(ys) < 16:
        return None
    x0 = _quantile(xs, q_low)
    x1 = _quantile(xs, q_high)
    y0 = _quantile(ys, q_low)
    y1 = _quantile(ys, q_high)
    if x1 <= x0 or y1 <= y0:
        return None
    return (x0, x1, y0, y1)


def _choose_bounds(full, robust):
    if full is None:
        return robust
    if robust is None:
        return full
    fx = max(1.0e-12, full[1] - full[0])
    fy = max(1.0e-12, full[3] - full[2])
    rx = max(1.0e-12, robust[1] - robust[0])
    ry = max(1.0e-12, robust[3] - robust[2])
    if (fx / rx) >= 1.6 or (fy / ry) >= 1.6:
        return robust
    return full


def _expand_bounds(bounds, margin: float):
    x0, x1, y0, y1 = bounds
    dx = max(1.0e-9, x1 - x0)
    dy = max(1.0e-9, y1 - y0)
    m = max(0.0, float(margin))
    return (x0 - dx * m, x1 + dx * m, y0 - dy * m, y1 + dy * m)


def _square_bounds(x0, x1, y0, y1):
    dx = x1 - x0
    dy = y1 - y0
    span = max(dx, dy)
    cx = (x0 + x1) * 0.5
    cy = (y0 + y1) * 0.5
    half = span * 0.5
    return (cx - half, cx + half, cy - half, cy + half)


def _quantile(values, q: float):
    data = sorted(float(v) for v in values)
    if not data:
        return 0.0
    if len(data) == 1:
        return data[0]
    qn = min(1.0, max(0.0, float(q)))
    pos = qn * (len(data) - 1)
    i = int(pos)
    frac = pos - i
    if i >= len(data) - 1:
        return data[-1]
    return data[i] * (1.0 - frac) + data[i + 1] * frac


def _to_xy(value):
    import math

    try:
        x = float(value[0])
        y = float(value[1])
    except Exception:
        return None
    if not math.isfinite(x) or not math.isfinite(y):
        return None
    return (x, y)


def _as_int(value, default: int = 0) -> int:
    try:
        return int(value)
    except Exception:
        return default


def _safe_point(value):
    import math

    if not isinstance(value, tuple) or len(value) < 2:
        return None
    x = float(value[0])
    y = float(value[1])
    z = float(value[2]) if len(value) > 2 else 0.0
    for v in (x, y, z):
        if not math.isfinite(v):
            return None
        if abs(v) > 1.0e12:
            return None
    return (x, y, z)


def _is_origin_point(point):
    if point is None:
        return True
    eps = 1.0e-12
    return abs(point[0]) < eps and abs(point[1]) < eps and abs(point[2]) < eps


def _deg_to_rad(value):
    import math

    return math.radians(float(value))


def _direction_from_angle(value):
    import math

    if value is None:
        return None
    try:
        angle = float(value)
    except Exception:
        return None
    if not math.isfinite(angle):
        return None
    rad = math.radians(angle)
    return (math.cos(rad), math.sin(rad))


def _normalize2(vec):
    import math

    vx, vy = vec
    length = math.hypot(vx, vy)
    if not math.isfinite(length) or length < 1.0e-12:
        return None
    return (vx / length, vy / length)


def _rotate2(vec, rad):
    import math

    c = math.cos(rad)
    s = math.sin(rad)
    return (vec[0] * c - vec[1] * s, vec[0] * s + vec[1] * c)


def _cross2(a, b):
    return a[0] * b[1] - a[1] * b[0]


def _line_line_intersection_2d(a, r, b, s):
    den = _cross2(r, s)
    if abs(den) < 1.0e-12:
        return None
    ba = (b[0] - a[0], b[1] - a[1])
    t = _cross2(ba, s) / den
    return (a[0] + t * r[0], a[1] + t * r[1])


def _project_to_line_2d(point, line_point, line_dir):
    d = _normalize2(line_dir)
    if d is None:
        return None
    px = point[0] - line_point[0]
    py = point[1] - line_point[1]
    t = px * d[0] + py * d[1]
    return (line_point[0] + t * d[0], line_point[1] + t * d[1])


def _distance2(a, b):
    import math

    return math.hypot(b[0] - a[0], b[1] - a[1])


def _extension_endpoint(base, intersection, overshoot, fallback_dir):
    vec = (intersection[0] - base[0], intersection[1] - base[1])
    direction = _normalize2(vec)
    if direction is None:
        direction = _normalize2(fallback_dir) or (0.0, 1.0)
    return (
        intersection[0] + direction[0] * overshoot,
        intersection[1] + direction[1] * overshoot,
    )


def _midpoint(a, b):
    return ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5)


def _resolve_dimension_text(dxf, text):
    measurement = _dimension_value(dxf, "actual_measurement")
    if (not text or text == "<>") and measurement is not None:
        return f"{measurement:g}"
    # Constraint-like labels (e.g. KEYwidth=...) are not the rendered dimension text.
    if text and "=" in text and measurement is not None:
        return f"{measurement:g}"
    return text


def _dimension_value(dxf, key, default=None):
    if key in dxf:
        return dxf.get(key, default)
    common = dxf.get("common")
    if isinstance(common, dict):
        return common.get(key, default)
    return default


def _draw_dim_ticks(ax, p1, p2, dim_dir, normal, dim_len, line_width, color=None):
    tick_len = max(0.25, dim_len * 0.03)
    dir1 = _normalize2((dim_dir[0] + normal[0], dim_dir[1] + normal[1]))
    dir2 = _normalize2((dim_dir[0] - normal[0], dim_dir[1] - normal[1]))
    tick_dir = dir1 or dir2
    if tick_dir is None:
        return
    hw = tick_len * 0.5
    for p in (p1, p2):
        ax.plot(
            [p[0] - tick_dir[0] * hw, p[0] + tick_dir[0] * hw],
            [p[1] - tick_dir[1] * hw, p[1] + tick_dir[1] * hw],
            linewidth=max(0.5, line_width * 0.9),
            color=color,
        )


def _draw_dim_single_tick(ax, p, dim_dir, normal, dim_len, line_width, color=None):
    tick_len = max(0.25, dim_len * 0.03)
    dir1 = _normalize2((dim_dir[0] + normal[0], dim_dir[1] + normal[1]))
    dir2 = _normalize2((dim_dir[0] - normal[0], dim_dir[1] - normal[1]))
    tick_dir = dir1 or dir2
    if tick_dir is None:
        return
    hw = tick_len * 0.5
    ax.plot(
        [p[0] - tick_dir[0] * hw, p[0] + tick_dir[0] * hw],
        [p[1] - tick_dir[1] * hw, p[1] + tick_dir[1] * hw],
        linewidth=max(0.5, line_width * 0.9),
        color=color,
    )
