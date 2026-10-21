"""Editable paper-space support: VIEWPORT entities and the paper of ``Layout1``.

Lengths of the layout are millimetres; viewport angles are degrees in the
editable dictionaries (the writer converts them to radians).
"""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

#: Editable VIEWPORT keys (``center``/``width``/``height`` on the sheet, the
#: model-space view, snap/grid, status flags, the viewport UCS, frozen layers).
FIELDS = {
    "center",
    "width",
    "height",
    "view_target",
    "view_direction",
    "view_twist_angle",
    "view_height",
    "view_center",
    "lens_length",
    "front_clip_z",
    "back_clip_z",
    "snap_angle",
    "snap_base",
    "snap_spacing",
    "grid_spacing",
    "circle_zoom",
    "status_flags",
    "style_sheet",
    "render_mode",
    "ucs_at_origin",
    "ucs_per_viewport",
    "ucs_origin",
    "ucs_x_axis",
    "ucs_y_axis",
    "ucs_elevation",
    "ucs_ortho_view_type",
    "frozen_layers",
}

#: Status flags (DXF 90) AutoCAD stores for the sheet's own viewport.
PAPER_VIEWPORT_STATUS = 0xC8020
#: Default status flags of a model viewport (UCS icon at the origin added).
MODEL_VIEWPORT_STATUS = 0xC8060
#: Status flag that locks the viewport zoom.
VIEWPORT_LOCKED = 0x4000

#: ``Drawing.layout`` keys and their defaults: an A3 landscape sheet, plotted
#: 1:1 in millimetres from its layout information, without margins.
LAYOUT_DEFAULTS: dict[str, Any] = {
    "name": "Layout1",
    "paper_width": 420.0,
    "paper_height": 297.0,
    "margins": (0.0, 0.0, 0.0, 0.0),
    "paper_size": "",
    "plot_origin": (0.0, 0.0),
    "paper_units": 1,
    "plot_rotation": 0,
    "plot_type": 5,
    "plot_layout_flags": 688,
    "plot_window": ((0.0, 0.0), (0.0, 0.0)),
    "scale_type": 16,
    "scale_numerator": 1.0,
    "scale_denominator": 1.0,
    "paper_image_origin": (0.0, 0.0),
    "page_setup_name": "",
    "printer": "",
    "plot_view_name": "",
    "style_sheet": "",
}
LAYOUT_FIELDS = frozenset(LAYOUT_DEFAULTS)
_LAYOUT_STRINGS = ("name", "paper_size", "page_setup_name", "printer", "plot_view_name", "style_sheet")
_LAYOUT_INTS = {"paper_units": 2, "plot_rotation": 3, "plot_type": 5, "plot_layout_flags": 0xFFFF, "scale_type": 32}

#: ISO A series (long side, short side) in millimetres.
ISO_PAPER = {
    "A0": (1189.0, 841.0),
    "A1": (841.0, 594.0),
    "A2": (594.0, 420.0),
    "A3": (420.0, 297.0),
    "A4": (297.0, 210.0),
}


def paper_size_name(width: float, height: float) -> str:
    """AutoCAD-style paper size name: ``ISO_A3_(420.00_x_297.00_MM)`` for an
    ISO A size in either orientation, ``User_(...)`` otherwise."""
    width, height = float(width), float(height)
    long_side, short_side = max(width, height), min(width, height)
    for name, (iso_long, iso_short) in ISO_PAPER.items():
        if abs(long_side - iso_long) <= 0.5 and abs(short_side - iso_short) <= 0.5:
            return f"ISO_{name}_({width:.2f}_x_{height:.2f}_MM)"
    return f"User_({width:.2f}_x_{height:.2f}_MM)"


def _pair(value: Any, what: str) -> tuple[float, float]:
    if not isinstance(value, (tuple, list)) or len(value) != 2:
        raise ValueError(f"{what} requires two coordinates")
    pair = (float(value[0]), float(value[1]))
    if not all(v == v and abs(v) != float("inf") for v in pair):
        raise ValueError(f"{what} must be finite")
    return pair


def _integer(value: Any, low: int, high: int, what: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise ValueError(f"{what} must be an integer in {low}..{high}")
    return value


def viewport_error(d: Mapping[str, Any], number, point, direction) -> None:
    """Raise ValueError for an invalid editable VIEWPORT dictionary."""
    point(d["center"])
    if number(d["width"]) <= 0 or number(d["height"]) <= 0:
        raise ValueError("VIEWPORT width and height must be positive")
    if number(d["view_height"]) <= 0:
        raise ValueError("VIEWPORT view height must be positive")
    _pair(d["view_center"], "view_center")
    for key in ("snap_base", "snap_spacing", "grid_spacing"):
        if key in d:
            _pair(d[key], key)
    point(d.get("view_target", (0.0, 0.0, 0.0)))
    direction(d.get("view_direction", (0.0, 0.0, 1.0)))
    point(d.get("ucs_origin", (0.0, 0.0, 0.0)))
    direction(d.get("ucs_x_axis", (1.0, 0.0, 0.0)))
    direction(d.get("ucs_y_axis", (0.0, 1.0, 0.0)))
    for key in ("view_twist_angle", "front_clip_z", "back_clip_z", "snap_angle", "ucs_elevation"):
        if key in d:
            number(d[key])
    if number(d.get("lens_length", 50.0)) < 0:
        raise ValueError("VIEWPORT lens length must not be negative")
    _integer(d.get("status_flags", 0), 0, 2**32 - 1, "status_flags")
    _integer(d.get("circle_zoom", 1000), 1, 65535, "circle_zoom")
    _integer(d.get("render_mode", 0), 0, 6, "render_mode")
    _integer(d.get("ucs_ortho_view_type", 0), 0, 6, "ucs_ortho_view_type")
    for key in ("ucs_at_origin", "ucs_per_viewport"):
        if key in d and not isinstance(d[key], bool):
            raise ValueError(f"{key} must be a boolean")
    style_sheet = d.get("style_sheet", "")
    if not isinstance(style_sheet, str) or "\0" in style_sheet:
        raise ValueError("style_sheet must be a string without NUL")
    frozen = d.get("frozen_layers", [])
    if not isinstance(frozen, (list, tuple)) or any(
        not isinstance(name, str) or not name for name in frozen
    ):
        raise ValueError("frozen_layers must be a list of layer names")
    if len({name.upper() for name in frozen}) != len(frozen):
        raise ValueError("frozen_layers names a layer twice")


def layout_row(layout: Mapping[str, Any]) -> dict[str, Any]:
    """Validate ``Drawing.layout`` and return the complete, normalised row."""
    unknown = set(layout) - LAYOUT_FIELDS
    if unknown:
        raise ValueError(f"unsupported layout attributes: {', '.join(sorted(unknown))}")
    row = {**LAYOUT_DEFAULTS, **layout}
    for key in _LAYOUT_STRINGS:
        if not isinstance(row[key], str) or "\0" in row[key]:
            raise ValueError(f"layout {key} must be a string without NUL")
    if not row["name"] or row["name"].upper() == "MODEL":
        raise ValueError("layout name must not be empty or Model")
    for key in ("paper_width", "paper_height", "scale_numerator", "scale_denominator"):
        row[key] = float(row[key])
        if not row[key] > 0 or row[key] == float("inf"):
            raise ValueError(f"layout {key} must be positive and finite")
    margins = row["margins"]
    if not isinstance(margins, (tuple, list)) or len(margins) != 4:
        raise ValueError("layout margins require (left, bottom, right, top)")
    margins = tuple(float(v) for v in margins)
    if (
        any(v < 0 or v != v for v in margins)
        or margins[0] + margins[2] >= row["paper_width"]
        or margins[1] + margins[3] >= row["paper_height"]
    ):
        raise ValueError("layout margins must be nonnegative and leave a printable area")
    row["margins"] = margins
    for key in ("plot_origin", "paper_image_origin"):
        row[key] = _pair(row[key], f"layout {key}")
    window = row["plot_window"]
    if not isinstance(window, (tuple, list)) or len(window) != 2:
        raise ValueError("layout plot_window requires ((x0, y0), (x1, y1))")
    row["plot_window"] = (_pair(window[0], "plot_window"), _pair(window[1], "plot_window"))
    for key, high in _LAYOUT_INTS.items():
        row[key] = _integer(row[key], 0, high, f"layout {key}")
    return row
