"""Editable IMAGE support: raster references and their IMAGEDEF table."""

from __future__ import annotations

import math
from typing import Any

FIELDS = {
    "image_def",
    "insert",
    "u_vector",
    "v_vector",
    "size",
    "display_props",
    "clipping",
    "brightness",
    "contrast",
    "fade",
    "clip_type",
    "clip_vertices",
    # Reader-only keys kept by an editable import.
    "image_def_handle",
    "reactor_handle",
    "class_version",
    "clip_mode",
    "file_path",
}
DEF_FIELDS = {"file_path", "size", "is_loaded", "resolution_units", "pixel_size"}


def image_error(d: dict[str, Any], number, point, direction) -> None:
    """Raise ValueError for an invalid editable IMAGE dictionary."""
    name = d.get("image_def")
    if not isinstance(name, str) or not name:
        raise ValueError("image_def must name an entry of Drawing.image_defs")
    point(d["insert"])
    u, v = point(d["u_vector"]), point(d["v_vector"])
    direction(u)
    direction(v)
    cross = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    if cross == (0.0, 0.0, 0.0):
        raise ValueError("u_vector and v_vector must not be parallel")
    size = d["size"]
    if len(size) != 2 or number(size[0]) <= 0 or number(size[1]) <= 0:
        raise ValueError("size must be two positive pixel counts")
    props = d.get("display_props", 3)
    if isinstance(props, bool) or not isinstance(props, int) or not 0 <= props <= 15:
        raise ValueError("display_props must be an integer 0..15")
    for key in ("brightness", "contrast", "fade"):
        value = d.get(key, 50 if key != "fade" else 0)
        if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 100:
            raise ValueError(f"{key} must be an integer 0..100")
    clip_type = d.get("clip_type", 1)
    if clip_type not in (1, 2):
        raise ValueError("clip_type must be 1 (rectangle) or 2 (polygon)")
    vertices = d.get("clip_vertices") or []
    if clip_type == 1 and vertices and len(vertices) != 2:
        raise ValueError("a rectangular clip boundary has exactly two corners")
    if clip_type == 2 and len(vertices) < 3:
        raise ValueError("a polygonal clip boundary needs at least three vertices")
    for vertex in vertices:
        if len(vertex) < 2:
            raise ValueError("clip vertices are (x, y) pixel coordinates")
        number(vertex[0])
        number(vertex[1])


def image_def_error(name: str, d: dict[str, Any]) -> None:
    if not isinstance(name, str) or not name:
        raise ValueError("image definition names must be non-empty strings")
    unknown = set(d) - DEF_FIELDS
    if unknown:
        raise ValueError(f"unsupported image definition keys: {', '.join(sorted(unknown))}")
    path = d.get("file_path")
    if not isinstance(path, str) or not path:
        raise ValueError(f"image definition {name!r} requires file_path")
    size = d.get("size")
    if not isinstance(size, (tuple, list)) or len(size) != 2:
        raise ValueError(f"image definition {name!r} requires size=(width, height) in pixels")
    for value in size:
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) or value <= 0:
            raise ValueError(f"image definition {name!r}: size must be positive")
    pixel = d.get("pixel_size", (1.0, 1.0))
    if not isinstance(pixel, (tuple, list)) or len(pixel) != 2:
        raise ValueError(f"image definition {name!r}: pixel_size must be (width, height)")
    for value in pixel:
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) or value <= 0:
            raise ValueError(f"image definition {name!r}: pixel_size must be positive")
    if d.get("resolution_units", 0) not in (0, 2, 5):
        raise ValueError(f"image definition {name!r}: resolution_units must be 0, 2 or 5")
    if not isinstance(d.get("is_loaded", True), bool):
        raise ValueError(f"image definition {name!r}: is_loaded must be a boolean")
