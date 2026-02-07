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
):
    plt = _require_matplotlib()
    if ax is None:
        _, ax = plt.subplots()

    for entity in layout.query(types):
        color = _resolve_dwg_color(entity.dxf)
        if color is None:
            color = _next_entity_color(ax)
        dxftype = entity.dxftype
        if dxftype == "LINE":
            _draw_line(ax, entity.dxf["start"], entity.dxf["end"], line_width, color=color)
        elif dxftype == "POINT":
            _draw_point(ax, entity.dxf["location"], line_width, color=color)
        elif dxftype == "LWPOLYLINE":
            _draw_polyline(ax, entity.dxf.get("points", []), line_width, color=color)
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
        elif dxftype == "TEXT":
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
            )
        elif dxftype == "DIMENSION":
            _draw_dimension(ax, entity.dxf, line_width, color=color)

    if title:
        ax.set_title(title)
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


def _next_entity_color(ax):
    try:
        return ax._get_lines.get_next_color()
    except Exception:
        return "C0"


def _resolve_dwg_color(dxf):
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

    true_color = dxf.get("resolved_true_color")
    if true_color is None:
        true_color = dxf.get("true_color")
    color = _true_color_to_hex(true_color)
    if color is not None:
        return color

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
        7: (255, 255, 255),
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


def _draw_polyline(ax, points, line_width: float, color=None):
    if not points:
        return
    xs = [pt[0] for pt in points]
    ys = [pt[1] for pt in points]
    ax.plot(xs, ys, linewidth=line_width, color=color)


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


def _draw_text(ax, insert, text: str, height: float, rotation_deg: float, color=None):
    if not text:
        return
    text = text.replace("\\P", "\n")
    size = max(6.0, abs(height) * 3.0)
    ax.text(insert[0], insert[1], text, fontsize=size, rotation=rotation_deg, color=color)


def _draw_dimension(ax, dxf, line_width: float, color=None):
    p13 = _safe_point(dxf.get("defpoint2"))
    p14 = _safe_point(dxf.get("defpoint3"))
    p10 = _safe_point(dxf.get("defpoint"))
    text_mid = _safe_point(dxf.get("text_midpoint"))
    insert = _safe_point(dxf.get("insert"))
    text = dxf.get("text", "")

    if p13 is None or p14 is None:
        return

    if p10 is None:
        p10 = _midpoint(p13, p14)

    dim_dir = _direction_from_angle(dxf.get("angle"))
    if dim_dir is None:
        dim_dir = _normalize2((p14[0] - p13[0], p14[1] - p13[1]))
    if dim_dir is None:
        return

    normal = (-dim_dir[1], dim_dir[0])
    oblique_deg = dxf.get("oblique_angle", 0.0) or 0.0
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
        height = dxf.get("char_height") or dxf.get("height") or max(0.8, dim_len * 0.06)
        rotation = dxf.get("text_rotation")
        if rotation is None:
            rotation = dxf.get("angle", 0.0)
        _draw_text(ax, text_pos, text, height, rotation, color=color)


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
    measurement = dxf.get("actual_measurement")
    if (not text or text == "<>") and measurement is not None:
        return f"{measurement:g}"
    # Constraint-like labels (e.g. KEYwidth=...) are not the rendered dimension text.
    if text and "=" in text and measurement is not None:
        return f"{measurement:g}"
    return text


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
