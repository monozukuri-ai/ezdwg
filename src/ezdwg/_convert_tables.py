"""DXF table entries for ``to_dxf`` and the names its entities use to reference them."""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Any

from . import _style_tables, document, raw


@dataclass
class DxfTableContext:
    # DWG STYLE handle -> ezdxf text style name
    style_name_by_handle: dict[int, str] = field(default_factory=dict)
    # DWG STYLE handle -> ezdxf entry handle (text and shape-file styles)
    style_dxf_handle_by_handle: dict[int, str] = field(default_factory=dict)
    mtext_style_handle_by_entity: dict[int, int] = field(default_factory=dict)
    # DWG LTYPE handle -> ezdxf linetype name
    linetype_name_by_handle: dict[int, str] = field(default_factory=dict)
    linetype_by_entity: dict[int, tuple[str | None, int | None, float]] = field(
        default_factory=dict
    )
    # DWG LAYER handle -> ezdxf linetype name
    linetype_name_by_layer: dict[int, str] = field(default_factory=dict)
    # Set until the MTEXT style map is decoded on its first use.
    _mtext_path: str | None = field(default=None, repr=False)


def _usable_name(name: Any) -> bool:
    """A non-blank table name without control characters, which ezdxf cannot save."""
    return isinstance(name, str) and bool(name.strip()) and all(ord(c) >= 32 for c in name)


def _number(value: Any, default: float, valid=lambda v: True) -> float:
    if isinstance(value, (int, float)) and math.isfinite(value) and valid(value):
        return float(value)
    return default


def _oblique_degrees(radians: Any) -> float:
    """DWG radians as DXF degrees in -85..85 (AutoCAD's range), else 0.0."""
    degrees = math.degrees(_number(radians, 0.0))
    if not math.isfinite(degrees):
        return 0.0
    # A negative angle can be stored as its positive turn (345 for -15).
    degrees = (degrees + 180.0) % 360.0 - 180.0
    return degrees if abs(degrees) <= 85.0 else 0.0


def prepare_dxf_styles(dxf_doc: Any, path: str, context: DxfTableContext) -> None:
    """Write every decoded STYLE row to ``dxf_doc`` and record the names in ``context``.

    A row that cannot be written is skipped; its entities keep ezdxf's Standard.
    """
    context._mtext_path = path
    try:
        rows = _style_tables.text_style_rows(path)
    except Exception:
        return
    fonts: dict[int, tuple[str, int]] = {}
    if any(row.get("extended_data") for row in rows.values()):
        try:
            fonts = _style_tables.text_style_font_xdata(path)
        except Exception:
            pass
    styles = dxf_doc.styles
    written: set[str] = set()
    for handle, row in rows.items():
        try:
            font = _one_line(row.get("font"))
            if row.get("shape"):
                if font:
                    entry = styles.find_shx(font) or styles.add_shx(font)
                    context.style_dxf_handle_by_handle[handle] = entry.dxf.handle
                continue
            name = row.get("name")
            if not _usable_name(name):
                continue
            if name in styles:
                entry = styles.get(name)
            else:
                entry = styles.add(name, font=font)
            context.style_name_by_handle[handle] = entry.dxf.name
            context.style_dxf_handle_by_handle[handle] = entry.dxf.handle
            key = name.lower()
            # Names are case-insensitive: the first row of a duplicate name wins.
            if key not in written:
                written.add(key)
                entry.update_dxf_attribs(
                    {
                        "font": font,
                        "bigfont": _one_line(row.get("bigfont")),
                        "flags": 4 if row.get("vertical") else 0,
                        "height": _number(row.get("height"), 0.0, lambda v: v >= 0),
                        "width": _number(row.get("width"), 1.0, lambda v: v > 0),
                        "oblique": _oblique_degrees(row.get("oblique")),
                        "generation_flags": int(_number(row.get("generation"), 0)) & 6,
                        "last_height": _number(row.get("last_height"), 2.5, lambda v: v > 0),
                    }
                )
                family, flags = fonts.get(handle, (None, None))
                family = _one_line(family) if isinstance(family, str) else ""
                if family and isinstance(flags, int):
                    entry.set_xdata("ACAD", [(1000, family), (1071, flags)])
        except Exception:
            continue


def _finite(*values: Any) -> bool:
    return all(
        isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)
        for v in values
    )


def _element_tags(element: Any, style_handles: dict[int, str]) -> list[tuple[int, Any]] | None:
    """DXF tags 74..9 of one complex element, or ``None`` when a value is unusable."""
    _length, flags, shape, x, y, scale, rotation, style, text = element
    if not isinstance(flags, int) or not flags & 6:
        return [(74, 0)]
    degrees = math.degrees(rotation) if _finite(rotation) else math.nan
    in_range = isinstance(shape, int) and 0 <= shape < 0x8000 and 0 <= flags < 0x8000
    if not (in_range and _finite(x, y, scale, degrees)):
        return None
    is_text = bool(flags & 2)
    if is_text and not _one_line(text):
        return None
    tags: list[tuple[int, Any]] = [(74, flags), (75, 0 if is_text else shape)]
    dxf_style = style_handles.get(style) if isinstance(style, int) else None
    if dxf_style is not None:
        tags.append((340, dxf_style))
    tags += [(46, float(scale)), (50, degrees), (44, float(x)), (45, float(y))]
    if is_text:
        tags.append((9, _one_line(text)))
    return tags


def _one_line(text: Any) -> str:
    """``text`` without control characters, which would break the DXF tag stream."""
    return "".join(c for c in str(text or "") if c >= " ")


def _linetype_pattern(row: dict, style_handles: dict[int, str] | None) -> Any:
    """The ezdxf pattern of an LTYPE row, plain dashes when ``style_handles`` is ``None``."""
    from ezdxf.entities.ltype import LinetypePattern
    from ezdxf.lldxf.tags import Tags
    from ezdxf.lldxf.types import DXFTag

    length, dashes = row.get("pattern_length"), row.get("dashes")
    if dashes is None or not _finite(length, *dashes):
        return None
    plain = [[(74, 0)] for _ in dashes]
    elements = row.get("elements") if style_handles is not None else None
    per_dash = plain
    if isinstance(elements, (list, tuple)) and len(elements) == len(dashes):
        try:
            per_dash = [_element_tags(e, style_handles) for e in elements]
        except (TypeError, ValueError):
            per_dash = plain
        if any(tags is None for tags in per_dash):
            per_dash = plain
    tags = [(72, 65), (73, len(dashes)), (40, float(length))]
    for dash, extra in zip(dashes, per_dash):
        tags += [(49, float(dash)), *extra]
    return LinetypePattern(Tags(DXFTag(code, value) for code, value in tags))


def prepare_dxf_linetypes(dxf_doc: Any, path: str, context: DxfTableContext) -> None:
    """Write every decoded LTYPE row to ``dxf_doc`` and record the names in ``context``.

    Also fills the entity and layer linetype maps. A row that cannot be written is
    skipped; entities and layers that use it keep their default linetype.
    """
    try:
        rows = _style_tables.linetype_rows(path)
    except Exception:
        rows = {}
    # ezdxf drops a linetype with a 340 group from R12 output, so R12 gets plain dashes.
    style_handles = context.style_dxf_handle_by_handle if dxf_doc.dxfversion > "AC1009" else None
    linetypes = dxf_doc.linetypes
    for handle, row in rows.items():
        try:
            name = row.get("name")
            if not _usable_name(name):
                continue
            if name not in linetypes:
                pattern = _linetype_pattern(row, style_handles)
                if pattern is None:
                    continue
                entry = linetypes.add(name, [0.0], description=_one_line(row.get("description")))
                entry.pattern_tags = pattern
            entry = linetypes.get(name)
            description = _one_line(row.get("description"))
            # ezdxf's built-in entries have no description; the first DWG one fills it.
            if name.upper() in _ENTITY_LINETYPES and description and not entry.dxf.description:
                entry.dxf.description = description
            context.linetype_name_by_handle[handle] = entry.dxf.name
        except Exception:
            continue
    try:
        context.linetype_by_entity = document._entity_linetype_map(path)
        layers = document._linetype_tables(path)[1]
    except Exception:
        return
    for layer, linetype in layers.items():
        name = context.linetype_name_by_handle.get(linetype)
        if name is not None and name.upper() not in ("BYLAYER", "BYBLOCK"):
            context.linetype_name_by_layer[layer] = name


_ENTITY_LINETYPES = {"BYLAYER": None, "BYBLOCK": "BYBLOCK", "CONTINUOUS": "Continuous"}


def entity_linetype(
    context: DxfTableContext | None, entity_handle: int | None
) -> tuple[str | None, float | None]:
    """``(linetype, ltscale)`` of an entity; ``None`` leaves the DXF default (BYLAYER, 1.0)."""
    if context is None or entity_handle is None:
        return None, None
    row = context.linetype_by_entity.get(entity_handle)
    if row is None:
        return None, None
    name, linetype_handle, scale = row
    scale = _style_tables.clean_linetype_scale(scale)
    ltscale = float(scale) if scale != 1.0 else None
    if name is None:
        return None, ltscale
    if name in _ENTITY_LINETYPES:
        return _ENTITY_LINETYPES[name], ltscale
    return context.linetype_name_by_handle.get(linetype_handle), ltscale


def _mtext_style_handles(context: DxfTableContext) -> dict[int, int]:
    path, context._mtext_path = context._mtext_path, None
    if path is not None:
        try:
            context.mtext_style_handle_by_entity.update(
                (int(handle), int(style))
                for handle, style, _spacing, _factor in raw.decode_mtext_write_properties(path)
                if style is not None
            )
        except Exception:
            pass
    return context.mtext_style_handle_by_entity


def text_style_name(
    context: DxfTableContext | None, entity_handle: int | None, dxf: dict
) -> str | None:
    """The ezdxf style name of an entity, or ``None`` to leave ``style`` unset."""
    if context is None or not context.style_name_by_handle:
        return None
    if "style_handle" in dxf:
        handle = dxf["style_handle"]
    elif entity_handle is not None:
        # MTEXT carries its style handle only when the colors were decoded.
        handle = _mtext_style_handles(context).get(entity_handle)
    else:
        return None
    try:
        return context.style_name_by_handle.get(int(handle))
    except (TypeError, ValueError):
        return None
