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
        dxftype = entity.dxftype
        if dxftype == "LINE":
            _draw_line(ax, entity.dxf["start"], entity.dxf["end"], line_width)
        elif dxftype == "POINT":
            _draw_point(ax, entity.dxf["location"], line_width)
        elif dxftype == "LWPOLYLINE":
            _draw_polyline(ax, entity.dxf.get("points", []), line_width)
        elif dxftype == "ARC":
            _draw_arc(
                ax,
                entity.dxf["center"],
                entity.dxf["radius"],
                entity.dxf["start_angle"],
                entity.dxf["end_angle"],
                arc_segments,
                line_width,
            )
        elif dxftype == "CIRCLE":
            _draw_circle(ax, entity.dxf["center"], entity.dxf["radius"], arc_segments, line_width)
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
            )
        elif dxftype == "TEXT":
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                entity.dxf.get("height", 1.0),
                entity.dxf.get("rotation", 0.0),
            )
        elif dxftype == "MTEXT":
            _draw_text(
                ax,
                entity.dxf.get("insert", (0.0, 0.0, 0.0)),
                entity.dxf.get("text", ""),
                entity.dxf.get("char_height", 1.0),
                entity.dxf.get("rotation", 0.0),
            )
        elif dxftype == "DIMENSION":
            _draw_dimension(ax, entity.dxf, line_width)

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


def _draw_line(ax, start, end, line_width: float):
    ax.plot([start[0], end[0]], [start[1], end[1]], linewidth=line_width)


def _draw_point(ax, location, line_width: float):
    size = max(2.0, line_width * 4.0)
    ax.plot([location[0]], [location[1]], marker="o", markersize=size, linewidth=0)


def _draw_polyline(ax, points, line_width: float):
    if not points:
        return
    xs = [pt[0] for pt in points]
    ys = [pt[1] for pt in points]
    ax.plot(xs, ys, linewidth=line_width)


def _draw_arc(
    ax,
    center,
    radius: float,
    start_angle: float,
    end_angle: float,
    segments: int,
    line_width: float,
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
    ax.plot(xs, ys, linewidth=line_width)


def _draw_circle(ax, center, radius: float, segments: int, line_width: float):
    _draw_arc(ax, center, radius, 0.0, 360.0, segments, line_width)


def _draw_ellipse(
    ax,
    center,
    major_axis,
    axis_ratio: float,
    start_angle: float,
    end_angle: float,
    segments: int,
    line_width: float,
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
    ax.plot(xs, ys, linewidth=line_width)


def _draw_text(ax, insert, text: str, height: float, rotation_deg: float):
    if not text:
        return
    text = text.replace("\\P", "\n")
    size = max(6.0, abs(height) * 3.0)
    ax.text(insert[0], insert[1], text, fontsize=size, rotation=rotation_deg)


def _draw_dimension(ax, dxf, line_width: float):
    p13 = dxf.get("defpoint2")
    p14 = dxf.get("defpoint3")
    p10 = dxf.get("defpoint")
    text_mid = dxf.get("text_midpoint")
    text = dxf.get("text", "")

    if p13 is not None and p14 is not None:
        _draw_line(ax, p13, p14, line_width)
    if p10 is not None and p13 is not None:
        _draw_line(ax, p10, p13, max(0.5, line_width * 0.8))
    if p10 is not None and p14 is not None:
        _draw_line(ax, p10, p14, max(0.5, line_width * 0.8))

    if (not text or text == "<>") and dxf.get("actual_measurement") is not None:
        text = f'{dxf["actual_measurement"]:g}'
    if text and text_mid is not None:
        _draw_text(ax, text_mid, text, 1.0, dxf.get("text_rotation", 0.0))


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
