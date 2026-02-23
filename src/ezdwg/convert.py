from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from . import raw
from .document import (
    Document,
    Layout,
    SUPPORTED_ENTITY_TYPES,
    TYPE_ALIASES,
    _present_supported_types,
    read,
)
from .entity import Entity


_POLYLINE_2D_SPLINE_CURVE_TYPES = {"QuadraticBSpline", "CubicBSpline", "Bezier"}
_BLOCK_EXCLUDED_ENTITY_TYPES = {
    "BLOCK",
    "ENDBLK",
    "SEQEND",
    "VERTEX_2D",
    "VERTEX_3D",
    "VERTEX_MESH",
    "VERTEX_PFACE",
    "VERTEX_PFACE_FACE",
}
_VERTEX_SEQUENCE_ENTITY_TYPES = {
    "VERTEX_2D",
    "VERTEX_3D",
    "VERTEX_MESH",
    "VERTEX_PFACE",
    "VERTEX_PFACE_FACE",
    "SEQEND",
}
_POLYLINE_OWNER_TYPES = {
    "POLYLINE_2D",
    "POLYLINE_3D",
    "POLYLINE_MESH",
    "POLYLINE_PFACE",
}
_WRITABLE_ENTITY_TYPES = {
    "LINE",
    "RAY",
    "XLINE",
    "POINT",
    "ARC",
    "CIRCLE",
    "ELLIPSE",
    "LWPOLYLINE",
    "POLYLINE_2D",
    "POLYLINE_3D",
    "POLYLINE_MESH",
    "POLYLINE_PFACE",
    "3DFACE",
    "SOLID",
    "TRACE",
    "SHAPE",
    "SPLINE",
    "TEXT",
    "ATTRIB",
    "ATTDEF",
    "MTEXT",
    "LEADER",
    "HATCH",
    "TOLERANCE",
    "MLINE",
    "INSERT",
    "MINSERT",
    "DIMENSION",
}
_DWG_WRITABLE_ENTITY_TYPES = {
    "LINE",
    "RAY",
    "XLINE",
    "POINT",
    "ARC",
    "CIRCLE",
    "LWPOLYLINE",
    "TEXT",
    "MTEXT",
}


@dataclass(frozen=True)
class ConvertResult:
    source_path: str
    output_path: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]


@dataclass(frozen=True)
class WriteResult:
    source_path: str
    output_path: str
    target_version: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]


def to_dwg(
    source: str | Document | Layout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    version: str = "AC1015",
    strict: bool = False,
) -> WriteResult:
    if version != "AC1015":
        raise ValueError(f"unsupported DWG write version: {version}")

    source_path, layout = _resolve_layout(source)
    source_entities = _resolve_dwg_export_entities(layout, types)

    total = 0
    written = 0
    skipped_by_type: dict[str, int] = {}
    line_rows: list[tuple[int, float, float, float, float, float, float]] = []
    ray_rows: list[tuple[int, tuple[float, float, float], tuple[float, float, float]]] = []
    xline_rows: list[tuple[int, tuple[float, float, float], tuple[float, float, float]]] = []
    point_rows: list[tuple[int, float, float, float, float]] = []
    arc_rows: list[tuple[int, float, float, float, float, float, float]] = []
    circle_rows: list[tuple[int, float, float, float, float]] = []
    lwpolyline_rows: list[
        tuple[int, int, list[tuple[float, float]], list[float], list[tuple[float, float]], float | None]
    ] = []
    text_rows: list[tuple[int, str, tuple[float, float, float], float, float]] = []
    mtext_rows: list[
        tuple[
            int,
            str,
            tuple[float, float, float],
            tuple[float, float, float],
            float,
            float,
            int,
            int,
        ]
    ] = []

    for entity in source_entities:
        total += 1
        if entity.dxftype == "LINE":
            row = _as_line_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            line_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "POINT":
            row = _as_point_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            point_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "RAY":
            row = _as_ray_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            ray_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "XLINE":
            row = _as_xline_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            xline_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "ARC":
            row = _as_arc_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            arc_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "CIRCLE":
            row = _as_circle_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            circle_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "LWPOLYLINE":
            row = _as_lwpolyline_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            lwpolyline_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "TEXT":
            row = _as_text_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            text_rows.append(row)
            written += 1
            continue
        if entity.dxftype == "MTEXT":
            row = _as_mtext_row(entity)
            if row is None:
                skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1
                continue
            mtext_rows.append(row)
            written += 1
            continue
        skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1

    skipped = total - written
    if strict and skipped > 0:
        summary = ", ".join(
            f"{dxftype}:{count}" for dxftype, count in sorted(skipped_by_type.items())
        )
        raise ValueError(f"failed to write {skipped} entities ({summary})")

    out_path = Path(output_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    raw.write_ac1015_dwg(
        str(out_path),
        line_rows,
        arc_rows,
        circle_rows,
        lwpolyline_rows,
        text_rows,
        mtext_rows,
        point_rows,
        ray_rows,
        xline_rows,
    )

    return WriteResult(
        source_path=source_path,
        output_path=str(out_path),
        target_version=version,
        total_entities=total,
        written_entities=written,
        skipped_entities=skipped,
        skipped_by_type=dict(sorted(skipped_by_type.items())),
    )


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


def to_dxf(
    source: str | Document | Layout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
    include_unsupported: bool = False,
    preserve_colors: bool = True,
    modelspace_only: bool = False,
) -> ConvertResult:
    ezdxf = _require_ezdxf()
    source_path, layout = _resolve_layout(source)

    dxf_doc = ezdxf.new(dxfversion=dxf_version)
    modelspace = dxf_doc.modelspace()

    source_entities = _resolve_export_entities(
        layout,
        types,
        include_unsupported=include_unsupported,
        include_styles=preserve_colors,
        modelspace_only=modelspace_only,
    )
    cached_entities_by_handle: dict[int, Entity] | None = None
    if types is None and not include_unsupported:
        cached_entities_by_handle = {}
        for entity in source_entities:
            try:
                cached_entities_by_handle[int(entity.handle)] = entity
            except Exception:
                continue

    insert_reference_entities = [
        entity for entity in source_entities if entity.dxftype in {"INSERT", "MINSERT"}
    ]
    if insert_reference_entities:
        insert_attributes_by_owner = _insert_attributes_by_owner(
            layout,
            include_styles=preserve_colors,
        )
        if insert_attributes_by_owner:
            source_entities = _attach_insert_attributes(
                source_entities, insert_attributes_by_owner
            )
        _populate_block_definitions(
            dxf_doc,
            layout,
            insert_attributes_by_owner=insert_attributes_by_owner,
            reference_entities=insert_reference_entities,
            cached_entities_by_handle=cached_entities_by_handle,
            include_styles=preserve_colors,
        )

    total = 0
    written = 0
    skipped_by_type: dict[str, int] = {}

    for entity in source_entities:
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


def _resolve_export_entities(
    layout: Layout,
    types: str | Iterable[str] | None,
    *,
    include_unsupported: bool = False,
    include_styles: bool = True,
    modelspace_only: bool = False,
) -> list[Entity]:
    query_types: str | Iterable[str] | None
    if types is not None:
        query_types = types
    elif include_unsupported:
        query_types = None
    else:
        present_types = set(_present_supported_types(layout.doc.decode_path))
        query_types = tuple(sorted(present_types & _WRITABLE_ENTITY_TYPES))
    selected_entities = list(layout.query(query_types, include_styles=include_styles))
    if modelspace_only:
        selected_entities = _filter_modelspace_entities(layout.doc.decode_path, selected_entities)
    return _materialize_export_entities(layout, selected_entities)


def _resolve_dwg_export_entities(
    layout: Layout,
    types: str | Iterable[str] | None,
) -> list[Entity]:
    query_types: str | Iterable[str] | None
    if types is not None:
        query_types = types
    else:
        present_types = set(_present_supported_types(layout.doc.decode_path))
        query_types = tuple(sorted(present_types & _DWG_WRITABLE_ENTITY_TYPES))
    selected_entities = list(layout.query(query_types))
    return _materialize_export_entities(layout, selected_entities)


def _filter_modelspace_entities(
    decode_path: str | None,
    entities: list[Entity],
) -> list[Entity]:
    if not decode_path or not entities:
        return entities
    modelspace_handles = _resolve_modelspace_entity_handles(decode_path)
    if modelspace_handles is None:
        return entities
    filtered: list[Entity] = []
    for entity in entities:
        try:
            if int(entity.handle) in modelspace_handles:
                filtered.append(entity)
        except Exception:
            continue
    if not filtered and entities:
        # Some drawings store modelspace ownership in ways this heuristic
        # cannot recover reliably yet. Keep behavior non-destructive.
        return entities
    return filtered


def _resolve_modelspace_entity_handles(decode_path: str) -> set[int] | None:
    try:
        header_rows = raw.list_object_headers_with_type(decode_path)
    except Exception:
        return None
    if not header_rows:
        return None

    block_name_by_handle = _resolve_block_name_by_handle(
        decode_path,
        header_rows,
    )
    if not block_name_by_handle:
        return None

    modelspace_block_handles = {
        int(handle)
        for handle, name in block_name_by_handle.items()
        if _is_modelspace_block_name(name)
    }
    if not modelspace_block_handles:
        return None

    sorted_rows = sorted(
        header_rows,
        key=lambda row: int(row[1]) if isinstance(row, tuple) and len(row) > 1 else 0,
    )
    current_block_handle: int | None = None
    handles: set[int] = set()
    for row in sorted_rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        raw_handle, _offset, _size, _code, raw_type_name, raw_type_class = row
        if str(raw_type_class).strip().upper() not in {"E", "ENTITY"}:
            continue
        try:
            handle = int(raw_handle)
        except Exception:
            continue
        type_name = str(raw_type_name).strip().upper()
        if type_name == "BLOCK":
            current_block_handle = handle
            continue
        if type_name == "ENDBLK":
            current_block_handle = None
            continue
        if current_block_handle in modelspace_block_handles:
            handles.add(handle)
    return handles


def _is_modelspace_block_name(name: str | None) -> bool:
    if not isinstance(name, str):
        return False
    token = name.strip().upper()
    return token in {"*MODEL_SPACE", "*MODEL SPACE", "MODELSPACE"}


def _materialize_export_entities(
    layout: Layout,
    selected_entities: list[Entity],
    *,
    allowed_owner_handles: set[int] | None = None,
    owners_by_handle: dict[int, Entity] | None = None,
) -> list[Entity]:
    if not selected_entities:
        return []

    export_entities: list[Entity] = []
    seen_handles: set[int] = set()
    owner_requests: list[tuple[int, str | None]] = []

    for entity in selected_entities:
        handle = int(entity.handle)
        if entity.dxftype in _BLOCK_EXCLUDED_ENTITY_TYPES:
            if entity.dxftype in _VERTEX_SEQUENCE_ENTITY_TYPES:
                owner_handle = entity.dxf.get("owner_handle")
                if owner_handle is None:
                    continue
                try:
                    owner_handle_int = int(owner_handle)
                except Exception:
                    continue
                if (
                    allowed_owner_handles is not None
                    and owner_handle_int not in allowed_owner_handles
                ):
                    continue
                owner_type = entity.dxf.get("owner_type")
                owner_requests.append(
                    (
                        owner_handle_int,
                        str(owner_type).strip().upper() if isinstance(owner_type, str) else None,
                    )
                )
            continue
        if handle in seen_handles:
            continue
        seen_handles.add(handle)
        export_entities.append(entity)

    if not owner_requests:
        return export_entities

    requested_owner_types = {
        owner_type for _, owner_type in owner_requests if owner_type in _POLYLINE_OWNER_TYPES
    }
    if not requested_owner_types:
        requested_owner_types = set(_POLYLINE_OWNER_TYPES)
    if owners_by_handle is None:
        resolved_owners_by_handle = _entities_by_handle(layout, requested_owner_types)
    else:
        resolved_owners_by_handle = {
            int(handle): entity
            for handle, entity in owners_by_handle.items()
            if entity.dxftype in requested_owner_types
        }
    for owner_handle, _owner_type in owner_requests:
        owner_entity = resolved_owners_by_handle.get(owner_handle)
        if owner_entity is None:
            continue
        if owner_entity.dxftype in _BLOCK_EXCLUDED_ENTITY_TYPES:
            continue
        handle = int(owner_entity.handle)
        if handle in seen_handles:
            continue
        if allowed_owner_handles is not None and handle not in allowed_owner_handles:
            continue
        seen_handles.add(handle)
        export_entities.append(owner_entity)

    return export_entities


def _insert_attributes_by_owner(
    layout: Layout,
    *,
    include_styles: bool = True,
) -> dict[int, list[Entity]]:
    try:
        attrib_entities = list(layout.query("ATTRIB", include_styles=include_styles))
    except Exception:
        return {}
    if not attrib_entities:
        return {}

    attrs_by_owner: dict[int, list[Entity]] = {}
    for entity in attrib_entities:
        owner_handle = entity.dxf.get("owner_handle")
        if owner_handle is None:
            continue
        try:
            owner_handle_int = int(owner_handle)
        except Exception:
            continue
        attrs_by_owner.setdefault(owner_handle_int, []).append(entity)

    for owner_handle, entities in attrs_by_owner.items():
        attrs_by_owner[owner_handle] = sorted(entities, key=lambda entry: int(entry.handle))
    return attrs_by_owner


def _attach_insert_attributes(
    selected_entities: list[Entity],
    insert_attributes_by_owner: dict[int, list[Entity]] | None,
) -> list[Entity]:
    if not selected_entities or not insert_attributes_by_owner:
        return selected_entities

    export_entities: list[Entity] = []
    for entity in selected_entities:
        if entity.dxftype not in {"INSERT", "MINSERT"}:
            export_entities.append(entity)
            continue
        try:
            handle = int(entity.handle)
        except Exception:
            export_entities.append(entity)
            continue
        attributes = insert_attributes_by_owner.get(handle)
        if not attributes:
            export_entities.append(entity)
            continue
        dxf = dict(entity.dxf)
        dxf["attributes"] = [dict(attribute.dxf) for attribute in attributes]
        export_entities.append(Entity(dxftype=entity.dxftype, handle=entity.handle, dxf=dxf))
    return export_entities


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


def _populate_block_definitions(
    dxf_doc: Any,
    layout: Layout,
    *,
    insert_attributes_by_owner: dict[int, list[Entity]] | None = None,
    reference_entities: list[Entity] | None = None,
    cached_entities_by_handle: dict[int, Entity] | None = None,
    include_styles: bool = True,
) -> None:
    if reference_entities is None:
        try:
            insert_entities = list(layout.query("INSERT"))
        except Exception:
            insert_entities = []
        try:
            minsert_entities = list(layout.query("MINSERT"))
        except Exception:
            minsert_entities = []
        if not insert_entities and not minsert_entities:
            return
        reference_entities = [*insert_entities, *minsert_entities]
    else:
        reference_entities = [
            entity for entity in reference_entities if entity.dxftype in {"INSERT", "MINSERT"}
        ]
        if not reference_entities:
            return

    referenced_names = {
        normalized_name
        for entity in reference_entities
        for normalized_name in [_normalize_block_name(entity.dxf.get("name"))]
        if normalized_name is not None
    }
    if not referenced_names:
        return

    decode_path = layout.doc.decode_path or layout.doc.path
    try:
        header_rows = raw.list_object_headers_with_type(decode_path)
    except Exception:
        return
    if not header_rows:
        return

    block_name_by_handle = _resolve_block_name_by_handle(decode_path, header_rows)
    if not block_name_by_handle:
        return

    block_members_by_name: dict[str, list[tuple[int, str]]] = {}
    current_block_name: str | None = None
    for row in header_rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        raw_handle, _offset, _size, _code, raw_type_name, raw_type_class = row
        type_class = str(raw_type_class).strip().upper()
        if type_class not in {"E", "ENTITY"}:
            continue
        try:
            handle = int(raw_handle)
        except Exception:
            continue
        type_name = str(raw_type_name).strip().upper()

        if type_name == "BLOCK":
            block_name = block_name_by_handle.get(handle)
            if isinstance(block_name, str) and block_name.strip() != "":
                current_block_name = block_name.strip()
                block_members_by_name.setdefault(current_block_name, [])
            else:
                current_block_name = None
            continue
        if type_name == "ENDBLK":
            current_block_name = None
            continue
        if current_block_name is None:
            continue
        block_members_by_name[current_block_name].append((handle, type_name))

    if not block_members_by_name:
        return

    # Start from directly referenced block names.
    selected_block_names = {
        name for name in referenced_names if name in block_members_by_name
    }
    if not selected_block_names:
        return

    has_member_candidates = any(block_members_by_name.get(name) for name in selected_block_names)
    insert_entities_by_handle: dict[int, Entity] = {}
    if has_member_candidates:
        if cached_entities_by_handle is not None:
            insert_entities_by_handle = {
                handle: entity
                for handle, entity in cached_entities_by_handle.items()
                if entity.dxftype in {"INSERT", "MINSERT"}
            }
        if not insert_entities_by_handle:
            if include_styles:
                insert_entities_by_handle = _entities_by_handle(layout, {"INSERT", "MINSERT"})
            else:
                insert_entities_by_handle = _entities_by_handle_no_styles(
                    layout,
                    {"INSERT", "MINSERT"},
                )
    selected_block_names = _collect_referenced_block_names(
        block_members_by_name,
        selected_block_names,
        insert_entities_by_handle,
    )

    if not selected_block_names:
        return

    all_member_types: set[str] = set()
    for block_name in selected_block_names:
        for _handle, raw_type_name in block_members_by_name.get(block_name, []):
            canonical = _canonical_entity_type(raw_type_name)
            if canonical in SUPPORTED_ENTITY_TYPES:
                all_member_types.add(canonical)
    if not all_member_types:
        return

    entities_by_handle: dict[int, Entity] = {}
    missing_member_types = set(all_member_types)
    if cached_entities_by_handle is not None:
        for handle, entity in cached_entities_by_handle.items():
            if entity.dxftype in all_member_types:
                entities_by_handle[handle] = entity
                missing_member_types.discard(entity.dxftype)
    if missing_member_types:
        if include_styles:
            entities_by_handle.update(_entities_by_handle(layout, missing_member_types))
        else:
            entities_by_handle.update(
                _entities_by_handle_no_styles(layout, missing_member_types)
            )
    if not entities_by_handle:
        return
    owner_entities_by_handle = {
        handle: entity
        for handle, entity in entities_by_handle.items()
        if entity.dxftype in _POLYLINE_OWNER_TYPES
    }
    owner_type_hints: set[str] = set()
    for block_name in selected_block_names:
        for handle, _raw_type_name in block_members_by_name.get(block_name, []):
            entity = entities_by_handle.get(int(handle))
            if entity is None or entity.dxftype not in _VERTEX_SEQUENCE_ENTITY_TYPES:
                continue
            owner_type = entity.dxf.get("owner_type")
            if not isinstance(owner_type, str):
                continue
            owner_type_token = owner_type.strip().upper()
            if owner_type_token in _POLYLINE_OWNER_TYPES:
                owner_type_hints.add(owner_type_token)
    if owner_type_hints:
        missing_owner_types = set(owner_type_hints)
        if cached_entities_by_handle is not None:
            for handle, entity in cached_entities_by_handle.items():
                if entity.dxftype in owner_type_hints:
                    owner_entities_by_handle[handle] = entity
                    missing_owner_types.discard(entity.dxftype)
        if missing_owner_types:
            if include_styles:
                owner_entities_by_handle.update(
                    _entities_by_handle(layout, missing_owner_types)
                )
            else:
                owner_entities_by_handle.update(
                    _entities_by_handle_no_styles(layout, missing_owner_types)
                )

    block_layouts: dict[str, Any] = {}
    for block_name in sorted(selected_block_names):
        block_layouts[block_name] = _ensure_block_layout(dxf_doc, block_name)

    for block_name in sorted(selected_block_names):
        block_layout = block_layouts[block_name]
        members = block_members_by_name.get(block_name, [])
        member_handles = {int(handle) for handle, _raw_type_name in members}
        selected_entities = [
            entity
            for handle, _raw_type_name in members
            for entity in [entities_by_handle.get(int(handle))]
            if entity is not None
        ]
        export_entities = _materialize_export_entities(
            layout,
            selected_entities,
            allowed_owner_handles=member_handles,
            owners_by_handle=owner_entities_by_handle,
        )
        export_entities = _attach_insert_attributes(export_entities, insert_attributes_by_owner)
        for entity in export_entities:
            _write_entity_to_modelspace(block_layout, entity)


def _resolve_block_name_by_handle(
    decode_path: str,
    header_rows: list[tuple[Any, ...]],
) -> dict[int, str]:
    # For smaller drawings, prefer exact BLOCK<->name mapping.
    if len(header_rows) <= 2048:
        exact_map = _resolve_block_name_by_handle_exact(decode_path)
        if exact_map:
            return exact_map

    block_handles_in_order: list[int] = []
    for row in header_rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        raw_handle, _offset, _size, _code, raw_type_name, raw_type_class = row
        if str(raw_type_class).strip().upper() not in {"E", "ENTITY"}:
            continue
        if str(raw_type_name).strip().upper() != "BLOCK":
            continue
        try:
            block_handles_in_order.append(int(raw_handle))
        except Exception:
            continue
    if not block_handles_in_order:
        return {}

    try:
        rows = raw.decode_block_header_names(decode_path, len(block_handles_in_order))
    except TypeError:
        # Backward compatibility for extension builds without optional limit support.
        try:
            rows = raw.decode_block_header_names(decode_path)
        except Exception:
            rows = []
    except Exception:
        rows = []

    by_header_handle: dict[int, str] = {}
    ordered_names: list[str] = []
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 2:
            continue
        raw_handle, raw_name = row[0], row[1]
        normalized_name = _normalize_block_name(raw_name)
        if normalized_name is None:
            continue
        ordered_names.append(normalized_name)
        try:
            by_header_handle[int(raw_handle)] = normalized_name
        except Exception:
            continue

    block_name_by_handle: dict[int, str] = {}
    for handle in block_handles_in_order:
        name = by_header_handle.get(handle)
        if name is not None:
            block_name_by_handle[handle] = name

    fallback_index = 0
    for handle in block_handles_in_order:
        if handle in block_name_by_handle:
            continue
        if fallback_index >= len(ordered_names):
            break
        block_name_by_handle[handle] = ordered_names[fallback_index]
        fallback_index += 1

    if block_name_by_handle:
        return block_name_by_handle

    # Fallback for environments that mock only decode_block_entity_names.
    return _resolve_block_name_by_handle_exact(decode_path)


def _resolve_block_name_by_handle_exact(decode_path: str) -> dict[int, str]:
    try:
        entity_rows = raw.decode_block_entity_names(decode_path)
    except Exception:
        return {}
    if not entity_rows:
        return {}

    result: dict[int, str] = {}
    for row in entity_rows:
        if not isinstance(row, tuple) or len(row) < 3:
            continue
        raw_handle, raw_type_name, raw_name = row[0], row[1], row[2]
        if str(raw_type_name).strip().upper() != "BLOCK":
            continue
        normalized_name = _normalize_block_name(raw_name)
        if normalized_name is None:
            continue
        try:
            result[int(raw_handle)] = normalized_name
        except Exception:
            continue
    return result


def _entities_by_handle(layout: Layout, types: set[str]) -> dict[int, Entity]:
    return _entities_by_handle_impl(layout, types, include_styles=True)


def _entities_by_handle_no_styles(layout: Layout, types: set[str]) -> dict[int, Entity]:
    return _entities_by_handle_impl(layout, types, include_styles=False)


def _entities_by_handle_impl(
    layout: Layout,
    types: set[str],
    *,
    include_styles: bool,
) -> dict[int, Entity]:
    result: dict[int, Entity] = {}

    # INSERT/MINSERT has a shared fast-path in Layout.query().
    if types == {"INSERT", "MINSERT"}:
        try:
            entities = layout.query("INSERT MINSERT", include_styles=include_styles)
        except TypeError:
            try:
                entities = layout.query("INSERT MINSERT")
            except Exception:
                entities = []
        except Exception:
            entities = []
        try:
            for entity in entities:
                try:
                    result[int(entity.handle)] = entity
                except Exception:
                    continue
        except Exception:
            return result
        return result

    for dxftype in sorted(types):
        try:
            entities = layout.query(dxftype, include_styles=include_styles)
        except TypeError:
            # Backward compatibility for tests/mocks that expose query(dxftype) only.
            try:
                entities = layout.query(dxftype)
            except Exception:
                continue
        except Exception:
            continue
        try:
            for entity in entities:
                try:
                    result[int(entity.handle)] = entity
                except Exception:
                    continue
        except Exception:
            continue
    return result


def _collect_referenced_block_names(
    block_members_by_name: dict[str, list[tuple[int, str]]],
    referenced_names: set[str],
    insert_entities_by_handle: dict[int, Entity],
) -> set[str]:
    selected_block_names: set[str] = set()
    pending_names: list[str] = [name for name in referenced_names if name in block_members_by_name]
    pending_name_set: set[str] = set(pending_names)
    while pending_names:
        name = pending_names.pop()
        pending_name_set.discard(name)
        if name in selected_block_names:
            continue
        selected_block_names.add(name)
        for handle, raw_type_name in block_members_by_name.get(name, []):
            if _canonical_entity_type(raw_type_name) not in {"INSERT", "MINSERT"}:
                continue
            insert_entity = insert_entities_by_handle.get(int(handle))
            if insert_entity is None:
                continue
            nested_name = _normalize_block_name(insert_entity.dxf.get("name"))
            if nested_name is None:
                continue
            if nested_name not in block_members_by_name:
                continue
            if nested_name in selected_block_names or nested_name in pending_name_set:
                continue
            pending_names.append(nested_name)
            pending_name_set.add(nested_name)
    return selected_block_names


def _normalize_block_name(name: Any) -> str | None:
    if not isinstance(name, str):
        return None
    normalized = name.strip()
    if not normalized:
        return None
    return normalized


def _ensure_block_layout(dxf_doc: Any, name: str) -> Any:
    try:
        block_layout = dxf_doc.blocks.get(name)
        if block_layout is not None:
            return block_layout
    except Exception:
        pass
    return dxf_doc.blocks.new(name=name)


def _canonical_entity_type(raw_type_name: str) -> str:
    token = str(raw_type_name).strip().upper()
    if token.startswith("DIM_"):
        return "DIMENSION"
    return TYPE_ALIASES.get(token, token)


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

    if dxftype == "RAY":
        modelspace.add_ray(
            _point3(dxf.get("start")),
            _point3(dxf.get("unit_vector")),
            dxfattribs=dxfattribs,
        )
        return True

    if dxftype == "XLINE":
        modelspace.add_xline(
            _point3(dxf.get("start")),
            _point3(dxf.get("unit_vector")),
            dxfattribs=dxfattribs,
        )
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

    if dxftype == "ATTDEF":
        return _write_attdef(modelspace, dxf, dxfattribs)

    if dxftype in {"TEXT", "ATTRIB"}:
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
        name = _normalize_block_name(dxf.get("name"))
        if name is not None:
            insert = _point3(dxf.get("insert"))
            row_count = max(1, int(dxf.get("row_count", 1)))
            column_count = max(1, int(dxf.get("column_count", 1)))
            attributes = list(dxf.get("attributes") or [])
            if attributes and (row_count > 1 or column_count > 1):
                try:
                    return _write_minsert_expanded(
                        modelspace,
                        name,
                        insert,
                        dxf,
                        dxfattribs,
                        attributes,
                    )
                except Exception:
                    pass
            try:
                ref = modelspace.add_blockref(name, insert, dxfattribs=dxfattribs)
                ref.dxf.xscale = float(dxf.get("xscale", 1.0))
                ref.dxf.yscale = float(dxf.get("yscale", 1.0))
                ref.dxf.zscale = float(dxf.get("zscale", 1.0))
                ref.dxf.rotation = float(dxf.get("rotation", 0.0))
                ref.dxf.column_count = column_count
                ref.dxf.row_count = row_count
                ref.dxf.column_spacing = float(dxf.get("column_spacing", 0.0))
                ref.dxf.row_spacing = float(dxf.get("row_spacing", 0.0))
                _write_insert_attributes(ref, attributes)
                return True
            except Exception:
                pass
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "INSERT":
        name = _normalize_block_name(dxf.get("name"))
        if name is not None:
            insert = _point3(dxf.get("insert"))
            try:
                ref = modelspace.add_blockref(name, insert, dxfattribs=dxfattribs)
                ref.dxf.xscale = float(dxf.get("xscale", 1.0))
                ref.dxf.yscale = float(dxf.get("yscale", 1.0))
                ref.dxf.zscale = float(dxf.get("zscale", 1.0))
                ref.dxf.rotation = float(dxf.get("rotation", 0.0))
                _write_insert_attributes(ref, list(dxf.get("attributes") or []))
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


def _write_minsert_expanded(
    modelspace: Any,
    name: str,
    insert: tuple[float, float, float],
    dxf: dict[str, Any],
    dxfattribs: dict[str, Any],
    attributes: list[Any],
) -> bool:
    row_count = max(1, int(dxf.get("row_count", 1)))
    column_count = max(1, int(dxf.get("column_count", 1)))
    column_spacing = float(dxf.get("column_spacing", 0.0))
    row_spacing = float(dxf.get("row_spacing", 0.0))
    rotation_deg = float(dxf.get("rotation", 0.0))
    rotation = math.radians(rotation_deg)
    cos_r = math.cos(rotation)
    sin_r = math.sin(rotation)

    col_dx = column_spacing * cos_r
    col_dy = column_spacing * sin_r
    row_dx = -row_spacing * sin_r
    row_dy = row_spacing * cos_r

    written = 0
    for row in range(row_count):
        for column in range(column_count):
            offset = (
                column * col_dx + row * row_dx,
                column * col_dy + row * row_dy,
                0.0,
            )
            cell_insert = (
                insert[0] + offset[0],
                insert[1] + offset[1],
                insert[2] + offset[2],
            )
            try:
                ref = modelspace.add_blockref(name, cell_insert, dxfattribs=dxfattribs)
                ref.dxf.xscale = float(dxf.get("xscale", 1.0))
                ref.dxf.yscale = float(dxf.get("yscale", 1.0))
                ref.dxf.zscale = float(dxf.get("zscale", 1.0))
                ref.dxf.rotation = rotation_deg
                shifted_attributes = _shift_attribute_positions(attributes, offset)
                _write_insert_attributes(ref, shifted_attributes)
                written += 1
            except Exception:
                continue
    return written > 0


def _shift_attribute_positions(attributes: list[Any], offset: tuple[float, float, float]) -> list[Any]:
    if offset == (0.0, 0.0, 0.0):
        return attributes
    shifted: list[Any] = []
    for attribute in attributes:
        if not isinstance(attribute, dict):
            shifted.append(attribute)
            continue
        attribute_dxf = dict(attribute)
        for key in ("insert", "align_point"):
            point = attribute_dxf.get(key)
            if not isinstance(point, (list, tuple)) or len(point) < 3:
                continue
            try:
                attribute_dxf[key] = (
                    float(point[0]) + offset[0],
                    float(point[1]) + offset[1],
                    float(point[2]) + offset[2],
                )
            except Exception:
                continue
        shifted.append(attribute_dxf)
    return shifted


def _write_insert_attributes(insert_ref: Any, attributes: list[Any]) -> None:
    if not attributes:
        return
    for attribute in attributes:
        if not isinstance(attribute, dict):
            continue
        tag = attribute.get("tag")
        if not isinstance(tag, str):
            continue
        tag_value = tag.strip()
        if tag_value == "":
            continue
        text = attribute.get("text")
        attrib_dxfattribs = _entity_dxfattribs(attribute)
        height = attribute.get("height")
        if height is not None:
            try:
                attrib_dxfattribs["height"] = float(height)
            except Exception:
                pass
        rotation = attribute.get("rotation")
        if rotation is not None:
            try:
                attrib_dxfattribs["rotation"] = float(rotation)
            except Exception:
                pass
        try:
            insert_ref.add_attrib(
                tag_value,
                "" if text is None else str(text),
                insert=_point3(attribute.get("insert")),
                dxfattribs=attrib_dxfattribs or None,
            )
        except Exception:
            continue


def _write_attdef(modelspace: Any, dxf: dict[str, Any], dxfattribs: dict[str, Any]) -> bool:
    tag = dxf.get("tag")
    if not isinstance(tag, str):
        return _write_text_like(modelspace, dxf, dxfattribs)
    tag_value = tag.strip()
    if tag_value == "":
        return _write_text_like(modelspace, dxf, dxfattribs)

    text = str(dxf.get("text", "") or "")
    insert = _point3(dxf.get("insert"))
    height = dxf.get("height")
    rotation = dxf.get("rotation")
    try:
        attdef = modelspace.add_attdef(
            tag=tag_value,
            insert=insert,
            text=text,
            height=float(height) if height is not None else None,
            rotation=float(rotation) if rotation is not None else None,
            dxfattribs=dxfattribs or None,
        )
    except Exception:
        return _write_text_like(modelspace, dxf, dxfattribs)

    prompt = dxf.get("prompt")
    if isinstance(prompt, str):
        try:
            attdef.dxf.prompt = prompt
        except Exception:
            pass
    attribute_flags = dxf.get("attribute_flags")
    if attribute_flags is not None:
        try:
            attdef.dxf.flags = int(attribute_flags)
        except Exception:
            pass
    lock_position = dxf.get("lock_position")
    if lock_position is not None:
        try:
            attdef.dxf.lock_position = 1 if bool(lock_position) else 0
        except Exception:
            pass
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
