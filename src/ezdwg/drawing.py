"""Editable, in-memory model-space drawings for the native AC1015 writer."""

from __future__ import annotations

import fnmatch
import math
import re
from collections.abc import Iterable, Iterator, Mapping, Sequence
from copy import deepcopy
from os import PathLike, fspath
from typing import TYPE_CHECKING, Any

from ._write_preflight import (
    ATTRIBUTES,
    WriteDiagnostic,
    geometry_error,
    prepare_strings,
)
from ._write_spline import FIELDS as _SPLINE_FIELDS
from .entity import Entity

if TYPE_CHECKING:
    from .convert import WriteResult
    from .document import Document

_TABLE_KINDS = {"DOCUMENT", "HEADER", "LAYER", "LTYPE", "STYLE", "BLOCK"}
_COMMON = {
    "layer",
    "color_index",
    "linetype",
    "linetype_scale",
    "lineweight",
    "invisible",
}
_NORMAL = {"extrusion", "thickness"}
_FIELDS = {
    "SPLINE": _SPLINE_FIELDS,
    "LINE": {"start", "end"} | _NORMAL,
    "POINT": {"location", "x_axis_angle"} | _NORMAL,
    "RAY": {"start", "unit_vector"},
    "XLINE": {"start", "unit_vector"},
    "ARC": {"center", "radius", "start_angle", "end_angle"} | _NORMAL,
    "CIRCLE": {"center", "radius"} | _NORMAL,
    "SOLID": {"points"} | _NORMAL,
    "TRACE": {"points"} | _NORMAL,
    "3DFACE": {"points", "invisible_edge_flags"},
    "ELLIPSE": {
        "center",
        "major_axis",
        "extrusion",
        "axis_ratio",
        "start_angle",
        "end_angle",
    },
    "LWPOLYLINE": {
        "points",
        "bulges",
        "widths",
        "const_width",
        "closed",
        "flags",
        "elevation",
    }
    | _NORMAL,
    "TEXT": {
        "text",
        "insert",
        "height",
        "rotation",
        "align_point",
        "oblique",
        "width",
        "text_generation_flag",
        "halign",
        "valign",
        "style",
    }
    | _NORMAL,
    "MTEXT": {
        "text",
        "insert",
        "text_direction",
        "char_height",
        "rect_width",
        "attachment_point",
        "drawing_direction",
        "line_spacing_style",
        "line_spacing_factor",
        "extrusion",
        "style",
    },
}
_FIELDS["INSERT"] = {
    "name",
    "insert",
    "xscale",
    "yscale",
    "zscale",
    "rotation",
    "extrusion",
}
_FIELDS["ATTDEF"] = _FIELDS["TEXT"] | {"tag", "prompt", "attribute_flags"}
_FIELDS["ATTRIB"] = _FIELDS["TEXT"] | {"tag", "attribute_flags"}
_FIELDS["POLYLINE_2D"] = {
    "points",
    "bulges",
    "widths",
    "width_start",
    "width_end",
    "flags",
    "closed",
    "curve_type",
    "elevation",
} | _NORMAL
_FIELDS["POLYLINE_3D"] = {"points", "flags", "closed", "curve_type"}
_POINTS = {"start", "end", "location", "center", "insert", "align_point"}
_VECTORS = {
    "extrusion",
    "unit_vector",
    "text_direction",
    "major_axis",
    "start_tangent",
    "end_tangent",
}


def new(version: str = "AC1015") -> Drawing:
    """Create an empty drawing without an input file."""
    return Drawing(version)


def _normalize(data: Mapping[str, Any]) -> dict[str, Any]:
    result = deepcopy(dict(data))
    for key in _POINTS | _VECTORS:
        value = result.get(key)
        if isinstance(value, (list, tuple)):
            # Positions accept 2D shorthand; normals/directions require 3D.
            result[key] = tuple(value) + (
                (0.0,) if key in _POINTS and len(value) == 2 else ()
            )
    for key in ("points", "widths", "control_points", "fit_points"):
        if isinstance(result.get(key), (list, tuple)):
            result[key] = [tuple(p) for p in result[key]]
    return result


def _check_entity(entity: Entity) -> None:
    allowed = _FIELDS.get(entity.dxftype)
    if allowed is None:
        raise ValueError(f"unsupported editable entity: {entity.dxftype}")
    unknown = entity.dxf.keys() - allowed - _COMMON
    if unknown:
        raise ValueError(
            f"unsupported {entity.dxftype} attributes: {', '.join(sorted(unknown))}"
        )
    error = geometry_error(entity)
    if error:
        raise ValueError(f"{entity.dxftype}: {error}")
    if entity.dxftype == "LWPOLYLINE":
        if any(len(p) != 2 for p in entity.dxf["points"]):
            raise ValueError(
                "LWPOLYLINE points require exactly two coordinates; use elevation"
            )
        if int(entity.dxf.get("flags", 0)) & ~(0x80 | 0x200):
            raise ValueError(
                "use closed=True for LWPOLYLINE closure (native flags only)"
            )


class Drawing:
    """Independent editable drawing, with named writer table dictionaries.

    ``layers``, ``linetypes`` and ``text_styles`` map names to property dicts.
    ``header`` stores AC1015 units/precision and global linetype scale. These
    tables are validated on save, including unused definitions and references.
    """

    def __init__(self, version: str = "AC1015") -> None:
        if version != "AC1015":
            raise ValueError(f"unsupported DWG write version: {version}")
        self.version = version
        self.source_path: str | None = None
        self.path: str | None = None
        self.layers: dict[str, dict[str, Any]] = {"0": {"color_index": 7}}
        self.linetypes: dict[str, dict[str, Any]] = {}
        self.text_styles: dict[str, dict[str, Any]] = {"Standard": {"font": "txt"}}
        self.header: dict[str, Any] = {
            "insunits": 0,
            "lunits": 2,
            "luprec": 4,
            "aunits": 0,
            "auprec": 0,
            "ltscale": 1.0,
        }
        self._next_handle = 1
        self._attributes: dict[int, EditableLayout] = {}
        self.blocks = BlockTable(self)
        self._layout = EditableLayout(self)
        self._diagnostics: tuple[WriteDiagnostic, ...] = ()

    @classmethod
    def from_document(cls, doc: Document, *, strict: bool = True) -> Drawing:
        """Snapshot model space, block definitions and their attached attributes."""
        from ._drawing_import import import_drawing

        return import_drawing(doc, strict=strict)

    @property
    def diagnostics(self) -> tuple[WriteDiagnostic, ...]:
        """Import losses still relevant to entities that have not been deleted."""
        return tuple(
            d
            for d in self._diagnostics
            if d.dxftype in _TABLE_KINDS or self._find_handle(d.handle) is not None
        )

    def _layouts(self) -> Iterator[EditableLayout]:
        yield self._layout
        yield from self.blocks.values()
        yield from self._attributes.values()

    def _find_handle(self, handle: int | None) -> Entity | None:
        return next(
            (
                layout._entities[handle]
                for layout in self._layouts()
                if handle in layout._entities
            ),
            None,
        )

    def _write_selection(
        self, entities: list[Entity], *, include_all_blocks: bool = False
    ):
        """Include attached attributes and the transitive block dependency closure."""
        selected = {e.handle: e for e in entities if e.dxftype != "ATTRIB"}
        # Keep orphan ATTRIBs visible so validation never silently loses them.
        selected.update((e.handle, e) for e in entities if e.dxftype == "ATTRIB")
        pending = (
            list(self.blocks)
            if include_all_blocks
            else [e.dxf.get("name") for e in entities if e.dxftype == "INSERT"]
        )
        blocks = {}
        while pending:
            name = pending.pop()
            if not isinstance(name, str) or name not in self.blocks:
                raise ValueError(f"undefined block: {name}")
            block = self.blocks[name]
            if block.name in blocks:
                continue
            blocks[block.name] = {
                "name": block.name,
                "base_point": tuple(block.base_point),
                "description": block.description,
            }
            for e in block:
                selected[e.handle] = e
                if e.dxftype == "INSERT":
                    pending.append(e.dxf.get("name"))
        for handle in tuple(selected):
            if handle in self._attributes:
                selected.update((e.handle, e) for e in self._attributes[handle])
        owners = {}
        for name in blocks:
            owners.update(
                (e.handle, name) for e in self.blocks[name] if e.handle in selected
            )
        for owner, layout in self._attributes.items():
            owners.update((e.handle, owner) for e in layout if e.handle in selected)
        return list(selected.values()), blocks, owners

    def modelspace(self) -> EditableLayout:
        return self._layout

    def save(
        self, output_path: str | PathLike[str] | None = None, *, strict: bool = True
    ) -> WriteResult:
        """Validate, write to a temporary sibling, then atomically replace output.

        Without a path, reuse the last successful save or the imported source.
        A failed save leaves the existing destination and ``path`` unchanged.
        """
        from .convert import to_dwg

        destination = (
            output_path if output_path is not None else self.path or self.source_path
        )
        if destination is None:
            raise ValueError("a new drawing requires an output path for its first save")
        result = to_dwg(self, fspath(destination), version=self.version, strict=strict)
        self.path = result.output_path
        return result

    def export_dwg(
        self, output_path: str | PathLike[str], *, strict: bool = True
    ) -> WriteResult:
        return self.save(output_path, strict=strict)

    def _prepare_write(self, entities: list[Entity], block_rows=None):
        selected = {e.handle for e in entities}
        diagnostics = [
            d
            for d in self.diagnostics
            if d.dxftype in _TABLE_KINDS or d.handle in selected
        ]
        attrs, prepared = {}, []
        for entity in entities:
            allowed = _FIELDS.get(entity.dxftype, set()) | _COMMON
            for key in entity.dxf.keys() - allowed:
                diagnostics.append(
                    WriteDiagnostic(
                        entity.handle,
                        entity.dxftype,
                        key,
                        "unsupported editable attribute",
                        "dropped",
                    )
                )
            data = _normalize({k: v for k, v in entity.dxf.items() if k in allowed})
            copy = Entity(entity.dxftype, entity.handle, data)
            # Direct dictionary mutations receive the same validation as update().
            if entity.dxftype in _FIELDS:
                try:
                    _check_entity(copy)
                except ValueError as exc:
                    diagnostics.append(
                        WriteDiagnostic(
                            entity.handle,
                            entity.dxftype,
                            "geometry",
                            str(exc),
                            "skipped",
                        )
                    )
                    copy = Entity(entity.dxftype, entity.handle, {})
            prepared.append(copy)
            attrs[entity.handle] = {
                k: v
                for k, v in data.items()
                if k in _COMMON | ATTRIBUTES | {"style"} and v is not None
            }
            # New entities default to ByLayer (the raw tuple API defaults to ACI 7).
            attrs[entity.handle].setdefault("color_index", 256)

        def rows(table):
            result = {}
            for name, props in table.items():
                if "name" in props:
                    raise ValueError("table name belongs in the dictionary key")
                result[name] = {"name": name, **deepcopy(props)}
            return result

        return prepare_strings(
            prepared,
            attrs,
            rows(self.layers),
            rows(self.linetypes),
            rows(self.text_styles),
            deepcopy(self.header),
            diagnostics,
            block_rows,
        )


class EditableLayout:
    """Persistent model-space entities with stable, unique integer handles."""

    def __init__(
        self,
        doc: Drawing,
        name: str = "MODELSPACE",
        *,
        attribute_owner: int | None = None,
    ) -> None:
        self.doc = doc
        self._name = name
        self._attribute_owner = attribute_owner
        self._entities: dict[int, Entity] = {}

    @property
    def name(self) -> str:
        return self._name

    def __len__(self) -> int:
        return len(self._entities)

    def __iter__(self) -> Iterator[Entity]:
        return self.query()

    def query(
        self, types: str | Iterable[str] | None = None, *, include_styles: bool = True
    ) -> Iterator[Entity]:
        """Yield live entities in insertion order, optionally matching type globs.

        Styles are always part of the in-memory data; ``include_styles`` exists
        for compatibility with read-only Layout.query().
        """
        tokens = (
            re.split(r"[,\s]+", types.strip())
            if isinstance(types, str)
            else list(types or ())
        )
        tokens = [t.strip().upper() for t in tokens if t.strip()]
        for entity in tuple(self._entities.values()):
            if (
                not tokens
                or "ALL" in tokens
                or any(fnmatch.fnmatchcase(entity.dxftype, t) for t in tokens)
            ):
                yield entity

    def iter_entities(
        self, types: str | Iterable[str] | None = None, **kwargs
    ) -> Iterator[Entity]:
        return self.query(types, **kwargs)

    def get(self, handle: int) -> Entity:
        if handle in self._entities:
            return self._entities[handle]
        for owner in self._entities:
            if (
                owner in self.doc._attributes
                and handle in self.doc._attributes[owner]._entities
            ):
                return self.doc._attributes[owner]._entities[handle]
        raise KeyError(handle)

    def add_entity(
        self, dxftype: str, dxf: Mapping[str, Any], *, handle: int | None = None
    ) -> Entity:
        """Copy geometry/attributes and allocate a handle, or validate an explicit one."""
        kind = dxftype.upper()
        if (kind == "ATTRIB") != (self._attribute_owner is not None):
            raise ValueError("ATTRIB must be attached with add_attrib to an INSERT")
        if handle is None:
            handle = self.doc._next_handle
        if (
            isinstance(handle, bool)
            or not isinstance(handle, int)
            or not 0 < handle < 2**64
        ):
            raise ValueError("handle must be a positive 64-bit integer")
        if self.doc._find_handle(handle) is not None:
            raise ValueError(f"duplicate entity handle: {handle}")
        data = {
            "layer": "0",
            "color_index": 256,
            "linetype": "BYLAYER",
            **_normalize(dxf),
        }
        entity = Entity(kind, handle, data)
        _check_entity(entity)
        self._entities[handle] = entity
        self.doc._next_handle = max(self.doc._next_handle, handle + 1)
        return entity

    def _resolve(self, entity: Entity | int) -> Entity:
        if isinstance(entity, Entity):
            try:
                if self.get(entity.handle) is entity:
                    return entity
            except KeyError:
                pass
            raise ValueError("entity does not belong to this layout")
        return self.get(entity)

    def update(self, entity: Entity | int, **attributes: Any) -> Entity:
        """Validate an update before changing the live entity in place."""
        current = self._resolve(entity)
        data = _normalize({**current.dxf, **attributes})
        _check_entity(Entity(current.dxftype, current.handle, data))
        current.dxf.clear()
        current.dxf.update(data)
        return current

    def delete(self, entity: Entity | int) -> None:
        handle = self._resolve(entity).handle
        if handle not in self._entities:
            for owner in self._entities:
                attrs = self.doc._attributes.get(owner)
                if attrs and handle in attrs._entities:
                    attrs.delete(handle)
                    return
        deleted = {handle}
        attrs = self.doc._attributes.pop(handle, None)
        if attrs:
            deleted.update(attrs._entities)
        del self._entities[handle]
        self.doc._diagnostics = tuple(
            d
            for d in self.doc._diagnostics
            if d.dxftype in _TABLE_KINDS or d.handle not in deleted
        )

    def add_blockref(
        self,
        name: str,
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        *,
        scale: Sequence[float] = (1.0, 1.0, 1.0),
        rotation: float = 0.0,
        **attributes: Any,
    ) -> Entity:
        if len(scale) != 3:
            raise ValueError("INSERT scale requires three values")
        return self.add_entity(
            "INSERT",
            {
                "name": name,
                "insert": insert,
                "xscale": scale[0],
                "yscale": scale[1],
                "zscale": scale[2],
                "rotation": rotation,
                **attributes,
            },
        )

    def add_attdef(
        self,
        tag: str,
        text: str = "",
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        height: float = 1.0,
        *,
        prompt: str = "",
        **attributes: Any,
    ) -> Entity:
        return self.add_entity(
            "ATTDEF",
            {
                "tag": tag,
                "text": text,
                "insert": insert,
                "height": height,
                "prompt": prompt,
                **attributes,
            },
        )

    def add_attrib(
        self,
        blockref: Entity | int,
        tag: str,
        text: str,
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        height: float = 1.0,
        **attributes: Any,
    ) -> Entity:
        parent = self._resolve(blockref)
        if parent.dxftype != "INSERT":
            raise ValueError("ATTRIB owner must be INSERT")
        layout = self.doc._attributes.setdefault(
            parent.handle, EditableLayout(self.doc, attribute_owner=parent.handle)
        )
        return layout.add_entity(
            "ATTRIB",
            {
                "tag": tag,
                "text": text,
                "insert": insert,
                "height": height,
                **attributes,
            },
        )

    def attribs(self, blockref: Entity | int) -> tuple[Entity, ...]:
        parent = self._resolve(blockref)
        if parent.dxftype != "INSERT":
            raise ValueError("ATTRIB owner must be INSERT")
        return tuple(self.doc._attributes.get(parent.handle, ()))

    def add_line(
        self, start: Sequence[float], end: Sequence[float], **attributes: Any
    ) -> Entity:
        return self.add_entity("LINE", {"start": start, "end": end, **attributes})

    def add_point(self, location: Sequence[float], **attributes: Any) -> Entity:
        return self.add_entity("POINT", {"location": location, **attributes})

    def add_circle(
        self, center: Sequence[float], radius: float, **attributes: Any
    ) -> Entity:
        return self.add_entity(
            "CIRCLE", {"center": center, "radius": radius, **attributes}
        )

    def add_spline(
        self,
        control_points: Sequence[Sequence[float]],
        degree: int = 3,
        *,
        knots: Sequence[float] | None = None,
        weights: Sequence[float] | None = None,
        **attributes: Any,
    ) -> Entity:
        """Add a WCS control spline; omitted knots use a clamped uniform vector."""
        if (
            isinstance(degree, bool)
            or not isinstance(degree, int)
            or not 1 <= degree <= 10
        ):
            raise ValueError("SPLINE degree must be in 1..10")
        points = list(control_points)
        if knots is None:
            spans = len(points) - degree
            knots = (
                [0.0] * degree
                + list(range(max(0, spans + 1)))
                + [float(spans)] * degree
            )
        return self.add_entity(
            "SPLINE",
            {
                "scenario": 1,
                "degree": degree,
                "control_points": points,
                "knots": list(knots),
                "weights": list(weights) if weights is not None else [],
                "rational": weights is not None and len(weights) > 0,
                **attributes,
            },
        )

    def add_spline_fit(
        self,
        fit_points: Sequence[Sequence[float]],
        **attributes: Any,
    ) -> Entity:
        """Add an open cubic fit spline with optional WCS endpoint tangents."""
        return self.add_entity(
            "SPLINE",
            {
                "scenario": 2,
                "degree": 3,
                "fit_points": list(fit_points),
                **attributes,
            },
        )

    def add_solid(self, points: Sequence[Sequence[float]], **attributes: Any) -> Entity:
        """Add 3/4 OCS XYZ corners in stored order (quad perimeter: 0,1,3,2)."""
        return self.add_entity("SOLID", {"points": points, **attributes})

    def add_trace(self, points: Sequence[Sequence[float]], **attributes: Any) -> Entity:
        """Add 3/4 OCS XYZ corners in stored order (quad perimeter: 0,1,3,2)."""
        return self.add_entity("TRACE", {"points": points, **attributes})

    def add_3dface(
        self, points: Sequence[Sequence[float]], **attributes: Any
    ) -> Entity:
        """Add 3/4 WCS XYZ corners in perimeter order; edge flags use bits 1,2,4,8."""
        return self.add_entity("3DFACE", {"points": points, **attributes})

    def add_ellipse(
        self,
        center: Sequence[float],
        major_axis: Sequence[float] = (1.0, 0.0, 0.0),
        axis_ratio: float = 1.0,
        start_angle: float = 0.0,
        end_angle: float = math.tau,
        **attributes: Any,
    ) -> Entity:
        """Add an ellipse with WCS geometry and start/end parameters in radians."""
        return self.add_entity(
            "ELLIPSE",
            {
                "center": center,
                "major_axis": major_axis,
                "axis_ratio": axis_ratio,
                "start_angle": start_angle,
                "end_angle": end_angle,
                **attributes,
            },
        )

    def add_arc(
        self,
        center: Sequence[float],
        radius: float,
        start_angle: float,
        end_angle: float,
        **attributes: Any,
    ) -> Entity:
        """Add an arc with start/end angles in degrees."""
        return self.add_entity(
            "ARC",
            {
                "center": center,
                "radius": radius,
                "start_angle": start_angle,
                "end_angle": end_angle,
                **attributes,
            },
        )

    def add_ray(
        self, start: Sequence[float], unit_vector: Sequence[float], **attributes: Any
    ) -> Entity:
        return self.add_entity(
            "RAY", {"start": start, "unit_vector": unit_vector, **attributes}
        )

    def add_xline(
        self, start: Sequence[float], unit_vector: Sequence[float], **attributes: Any
    ) -> Entity:
        return self.add_entity(
            "XLINE", {"start": start, "unit_vector": unit_vector, **attributes}
        )

    def add_polyline2d(
        self, points: Sequence[Sequence[float]], **attributes: Any
    ) -> Entity:
        """Add an ordinary legacy 2D POLYLINE with OCS XY vertices."""
        return self.add_entity("POLYLINE_2D", {"points": points, **attributes})

    def add_polyline3d(
        self, points: Sequence[Sequence[float]], **attributes: Any
    ) -> Entity:
        """Add an ordinary legacy 3D POLYLINE with WCS XYZ vertices."""
        return self.add_entity("POLYLINE_3D", {"points": points, **attributes})

    def add_lwpolyline(
        self, points: Sequence[Sequence[float]], **attributes: Any
    ) -> Entity:
        return self.add_entity("LWPOLYLINE", {"points": points, **attributes})

    def add_text(
        self,
        text: str,
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        height: float = 1.0,
        **attributes: Any,
    ) -> Entity:
        return self.add_entity(
            "TEXT", {"text": text, "insert": insert, "height": height, **attributes}
        )

    def add_mtext(
        self,
        text: str,
        insert: Sequence[float] = (0.0, 0.0, 0.0),
        char_height: float = 1.0,
        **attributes: Any,
    ) -> Entity:
        """Use text for the raw MTEXT string (including formatting codes)."""
        return self.add_entity(
            "MTEXT",
            {"text": text, "insert": insert, "char_height": char_height, **attributes},
        )

    def export_dwg(
        self, output_path: str | PathLike[str], *, strict: bool = True
    ) -> WriteResult:
        from .convert import to_dwg

        return to_dwg(self, fspath(output_path), strict=strict)


class BlockLayout(EditableLayout):
    """Block contents in local coordinates, with a mutable base point and description."""

    def __init__(
        self, doc: Drawing, name: str, base_point: Sequence[float], description: str
    ) -> None:
        super().__init__(doc, name)
        self.base_point = tuple(base_point) + ((0.0,) if len(base_point) == 2 else ())
        self.description = description


class BlockTable(Mapping[str, BlockLayout]):
    """Case-insensitive named block definitions. Use new() and delete()."""

    def __init__(self, doc: Drawing) -> None:
        self.doc = doc
        self._blocks: dict[str, BlockLayout] = {}

    def __getitem__(self, name: str) -> BlockLayout:
        return self._blocks[name.upper()]

    def __iter__(self) -> Iterator[str]:
        return (b.name for b in self._blocks.values())

    def __len__(self) -> int:
        return len(self._blocks)

    def new(
        self,
        name: str,
        *,
        base_point: Sequence[float] = (0.0, 0.0, 0.0),
        description: str = "",
    ) -> BlockLayout:
        if not isinstance(name, str) or not name or name in self:
            raise ValueError(f"empty or duplicate block name: {name}")
        if name.upper().startswith(("*MODEL_SPACE", "*PAPER_SPACE")):
            raise ValueError("reserved block name")
        block = BlockLayout(self.doc, name, base_point, description)
        self._blocks[name.upper()] = block
        return block

    def delete(self, name: str) -> None:
        block = self[name]
        if any(
            e.dxftype == "INSERT" and str(e.dxf.get("name", "")).upper() == name.upper()
            for layout in self.doc._layouts()
            for e in layout
        ):
            raise ValueError(f"block is still referenced: {name}")
        for e in tuple(block):
            block.delete(e)
        del self._blocks[name.upper()]
