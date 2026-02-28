from __future__ import annotations

import json
import math
import weakref
from dataclasses import dataclass, field
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
_BLOCK_REFERENCE_ENTITY_TYPES = {"INSERT", "MINSERT", "DIMENSION"}
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
_MAX_COORD_ABS = 1.0e12
_BLOCK_INSERT_SAFETY_CACHE: weakref.WeakKeyDictionary[Any, dict[str, tuple[bool, bool, float | None]]] = weakref.WeakKeyDictionary()
_LAYOUT_PSEUDO_ALIAS_CACHE: weakref.WeakKeyDictionary[Any, dict[str, str]] = weakref.WeakKeyDictionary()
_BLOCK_LOCAL_Y_SPAN_CACHE: weakref.WeakKeyDictionary[Any, dict[str, float | None]] = weakref.WeakKeyDictionary()
_DIM_BLOCK_POLICIES = {"smart", "legacy"}
_OPEN30_REMAP_SCALE_MIN = 30.0
_OPEN30_REMAP_SCALE_MAX = 120.0
_LAYOUT_PSEUDO_MODELSPACE_ALIAS_PREFIX = "__EZDWG_LAYOUT_ALIAS_MODEL_SPACE"


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


@dataclass
class _DimensionWriteContext:
    written_block_refs: set[
        tuple[str, tuple[float, float, float], tuple[float, float, float], float]
    ] = field(default_factory=set)


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
    explode_dimensions: bool = True,
    flatten_inserts: bool = False,
    dim_block_policy: str = "smart",
) -> ConvertResult:
    ezdxf = _require_ezdxf()
    source_path, layout = _resolve_layout(source)
    normalized_dim_block_policy = _normalize_dim_block_policy(dim_block_policy)

    dxf_doc = ezdxf.new(dxfversion=dxf_version)
    modelspace = dxf_doc.modelspace()
    dimension_write_context = _DimensionWriteContext()

    source_entities = _resolve_export_entities(
        layout,
        types,
        include_unsupported=include_unsupported,
        include_styles=preserve_colors,
        modelspace_only=modelspace_only,
    )
    if _has_problematic_i_inserts(source_entities):
        available_block_names = _available_block_names(layout.doc.decode_path or layout.doc.path)
        if "_Open30" in available_block_names:
            source_entities = [
                _normalize_problematic_insert_name(
                    entity,
                    available_block_names=available_block_names,
                )
                for entity in source_entities
            ]
    source_entities = _deduplicate_layout_pseudo_inserts_by_handle(source_entities)
    cached_entities_by_handle: dict[int, Entity] | None = None
    if types is None and not include_unsupported:
        cached_entities_by_handle = {}
        for entity in source_entities:
            try:
                cached_entities_by_handle[int(entity.handle)] = entity
            except Exception:
                continue

    layer_styles_by_handle = _layer_styles_by_handle(layout.doc.decode_path or layout.doc.path)
    layer_name_by_handle = _prepare_dxf_layers(dxf_doc, layer_styles_by_handle)

    block_reference_entities = [
        entity
        for entity in source_entities
        if entity.dxftype in _BLOCK_REFERENCE_ENTITY_TYPES
        and _referenced_block_name_from_entity(entity) is not None
    ]
    if block_reference_entities:
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
            reference_entities=block_reference_entities,
            cached_entities_by_handle=cached_entities_by_handle,
            include_styles=preserve_colors,
            explode_dimensions=explode_dimensions,
            layer_name_by_handle=layer_name_by_handle,
            dim_block_policy=normalized_dim_block_policy,
        )

    total = 0
    written = 0
    skipped_by_type: dict[str, int] = {}

    for entity in source_entities:
        total += 1
        if _write_entity_to_modelspace(
            modelspace,
            entity,
            explode_dimensions=explode_dimensions,
            layer_name_by_handle=layer_name_by_handle,
            dim_block_policy=normalized_dim_block_policy,
            dimension_context=dimension_write_context,
        ):
            written += 1
            continue
        skipped_by_type[entity.dxftype] = skipped_by_type.get(entity.dxftype, 0) + 1

    skipped = total - written
    if strict and skipped > 0:
        summary = ", ".join(
            f"{dxftype}:{count}" for dxftype, count in sorted(skipped_by_type.items())
        )
        raise ValueError(f"failed to convert {skipped} entities ({summary})")

    if flatten_inserts:
        _flatten_modelspace_inserts(modelspace)

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


def _flatten_modelspace_inserts(modelspace: Any, *, max_depth: int = 8) -> None:
    # Flatten nested block references for CAD viewers that do not reliably
    # evaluate deep INSERT hierarchies.
    try:
        initial_entities = list(modelspace)
    except Exception:
        return
    original_entity_ids = {
        id(entity)
        for entity in initial_entities
        if _ezdxf_entity_type(entity) != "INSERT"
    }
    reference_bbox = _collect_reference_bbox(initial_entities)
    changed_any = False

    for _ in range(max_depth):
        try:
            inserts = list(modelspace.query("INSERT"))
        except Exception:
            break
        if not inserts:
            break
        exploded = 0
        dropped = 0
        for insert in inserts:
            block_name = _normalize_block_name(getattr(getattr(insert, "dxf", None), "name", None))
            if block_name is not None and _is_layout_pseudo_block_name(block_name):
                # Keep layout pseudo references intact. Exploding or deleting
                # them drops viewport-driven copies in some drawings.
                continue
            if _prepare_insert_for_flatten(modelspace, insert):
                try:
                    modelspace.delete_entity(insert)
                    dropped += 1
                except Exception:
                    pass
                continue
            try:
                insert.explode()
                exploded += 1
            except Exception:
                continue
        if exploded <= 0 and dropped <= 0:
            break
        changed_any = True

    if changed_any and reference_bbox is not None:
        _prune_flatten_outlier_entities(modelspace, original_entity_ids, reference_bbox)
        _prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)


def _ezdxf_entity_type(entity: Any) -> str:
    try:
        token = entity.dxftype()
    except Exception:
        try:
            token = entity.dxftype
        except Exception:
            return ""
        if callable(token):
            try:
                token = token()
            except Exception:
                return ""
    return str(token).strip().upper()


def _entity_xy_points(entity: Any) -> list[tuple[float, float]]:
    token = _ezdxf_entity_type(entity)
    dxf = getattr(entity, "dxf", None)
    if dxf is None:
        return []

    points: list[tuple[float, float]] = []

    def _push(x: Any, y: Any) -> None:
        try:
            x_val = float(x)
            y_val = float(y)
        except Exception:
            return
        if not (math.isfinite(x_val) and math.isfinite(y_val)):
            return
        points.append((x_val, y_val))

    try:
        if token == "LINE":
            _push(dxf.start.x, dxf.start.y)
            _push(dxf.end.x, dxf.end.y)
            return points
        if token == "POINT":
            _push(dxf.location.x, dxf.location.y)
            return points
        if token in {"ARC", "CIRCLE"}:
            _push(dxf.center.x, dxf.center.y)
            return points
        if token == "LWPOLYLINE":
            for point in entity.get_points("xy"):
                if len(point) >= 2:
                    _push(point[0], point[1])
            return points
        if token in {"TEXT", "MTEXT"}:
            _push(dxf.insert.x, dxf.insert.y)
            return points
        if token == "DIMENSION":
            _push(getattr(dxf.defpoint, "x", 0.0), getattr(dxf.defpoint, "y", 0.0))
            _push(getattr(dxf.text_midpoint, "x", 0.0), getattr(dxf.text_midpoint, "y", 0.0))
            return points
        if token in {"INSERT", "MINSERT"}:
            _push(dxf.insert.x, dxf.insert.y)
            return points
    except Exception:
        return points
    return points


def _collect_reference_bbox(entities: list[Any]) -> tuple[float, float, float, float] | None:
    xs: list[float] = []
    ys: list[float] = []
    for entity in entities:
        if _ezdxf_entity_type(entity) == "INSERT":
            continue
        for x, y in _entity_xy_points(entity):
            xs.append(x)
            ys.append(y)
    if not xs or not ys:
        return None
    return (min(xs), max(xs), min(ys), max(ys))


def _prune_flatten_outlier_entities(
    modelspace: Any,
    original_entity_ids: set[int],
    reference_bbox: tuple[float, float, float, float],
) -> None:
    min_x, max_x, min_y, max_y = reference_bbox
    # Keep a modest safety margin around original modelspace content.
    # Large margins let through mis-normalized exploded fragments; tight bounds
    # still preserve nearby annotation helpers around the drawing extents.
    margin_x = max(3000.0, (max_x - min_x) * 0.05)
    margin_y = max(1500.0, (max_y - min_y) * 0.05)
    window = (
        min_x - margin_x,
        max_x + margin_x,
        min_y - margin_y,
        max_y + margin_y,
    )
    window_min_x, window_max_x, window_min_y, window_max_y = window

    try:
        entities = list(modelspace)
    except Exception:
        return

    for entity in entities:
        if id(entity) in original_entity_ids:
            continue
        if _ezdxf_entity_type(entity) == "INSERT":
            continue
        points = _entity_xy_points(entity)
        if not points:
            continue
        outside = True
        for x, y in points:
            if (
                window_min_x <= x <= window_max_x
                and window_min_y <= y <= window_max_y
            ):
                outside = False
                break
        if not outside:
            continue
        try:
            modelspace.delete_entity(entity)
        except Exception:
            continue


def _entity_center_bbox(entity: Any) -> tuple[float, float, float, float, float, float] | None:
    points = _entity_xy_points(entity)
    if not points:
        return None
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    min_x = min(xs)
    max_x = max(xs)
    min_y = min(ys)
    max_y = max(ys)
    center_x = (min_x + max_x) * 0.5
    center_y = (min_y + max_y) * 0.5
    return (center_x, center_y, min_x, max_x, min_y, max_y)


def _cluster_entity_indices(
    centers: list[tuple[float, float]],
    *,
    radius: float,
) -> list[list[int]]:
    if not centers:
        return []
    if radius <= 0.0:
        return [[index] for index in range(len(centers))]

    cell_size = radius
    grid: dict[tuple[int, int], list[int]] = {}
    for index, (center_x, center_y) in enumerate(centers):
        grid_key = (
            int(math.floor(center_x / cell_size)),
            int(math.floor(center_y / cell_size)),
        )
        grid.setdefault(grid_key, []).append(index)

    parent = list(range(len(centers)))
    comp_size = [1] * len(centers)
    radius_squared = radius * radius

    def _find(value: int) -> int:
        while parent[value] != value:
            parent[value] = parent[parent[value]]
            value = parent[value]
        return value

    def _union(left: int, right: int) -> None:
        left_root = _find(left)
        right_root = _find(right)
        if left_root == right_root:
            return
        if comp_size[left_root] < comp_size[right_root]:
            left_root, right_root = right_root, left_root
        parent[right_root] = left_root
        comp_size[left_root] += comp_size[right_root]

    for index, (center_x, center_y) in enumerate(centers):
        grid_x = int(math.floor(center_x / cell_size))
        grid_y = int(math.floor(center_y / cell_size))
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for neighbor in grid.get((grid_x + dx, grid_y + dy), []):
                    if neighbor <= index:
                        continue
                    neighbor_x, neighbor_y = centers[neighbor]
                    if (
                        (center_x - neighbor_x) * (center_x - neighbor_x)
                        + (center_y - neighbor_y) * (center_y - neighbor_y)
                    ) <= radius_squared:
                        _union(index, neighbor)

    by_root: dict[int, list[int]] = {}
    for index in range(len(centers)):
        root = _find(index)
        by_root.setdefault(root, []).append(index)
    return list(by_root.values())


def _prune_flatten_tiny_generated_clusters(
    modelspace: Any,
    original_entity_ids: set[int],
) -> None:
    try:
        entities = list(modelspace)
    except Exception:
        return

    metadata: list[tuple[Any, bool, float, float, float, float, float, float]] = []
    centers: list[tuple[float, float]] = []
    for entity in entities:
        if _ezdxf_entity_type(entity) == "INSERT":
            continue
        center_bbox = _entity_center_bbox(entity)
        if center_bbox is None:
            continue
        center_x, center_y, min_x, max_x, min_y, max_y = center_bbox
        is_original = id(entity) in original_entity_ids
        metadata.append(
            (
                entity,
                is_original,
                center_x,
                center_y,
                min_x,
                max_x,
                min_y,
                max_y,
            )
        )
        centers.append((center_x, center_y))

    if not metadata:
        return

    components = _cluster_entity_indices(centers, radius=500.0)
    if not components:
        return

    major_regions: list[tuple[float, float, float, float]] = []
    for component in components:
        if len(component) < 250:
            continue
        min_x = min(metadata[index][4] for index in component)
        max_x = max(metadata[index][5] for index in component)
        min_y = min(metadata[index][6] for index in component)
        max_y = max(metadata[index][7] for index in component)
        major_regions.append((min_x, max_x, min_y, max_y))
    if not major_regions:
        return

    major_margin = 250.0
    for component in components:
        if len(component) > 8:
            continue
        if any(metadata[index][1] for index in component):
            continue
        min_x = min(metadata[index][4] for index in component)
        max_x = max(metadata[index][5] for index in component)
        min_y = min(metadata[index][6] for index in component)
        max_y = max(metadata[index][7] for index in component)
        if (max_x - min_x) > 1200.0 or (max_y - min_y) > 1200.0:
            continue
        center_x = (min_x + max_x) * 0.5
        center_y = (min_y + max_y) * 0.5
        inside_major = False
        for major_min_x, major_max_x, major_min_y, major_max_y in major_regions:
            if (
                (major_min_x - major_margin) <= center_x <= (major_max_x + major_margin)
                and (major_min_y - major_margin) <= center_y <= (major_max_y + major_margin)
            ):
                inside_major = True
                break
        if inside_major:
            continue
        for index in component:
            entity = metadata[index][0]
            try:
                modelspace.delete_entity(entity)
            except Exception:
                continue


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
    seen_entity_keys: set[tuple[str, int]] = set()
    first_dxf_by_key: dict[tuple[str, int], Any] = {}
    seen_frozen_dxf_by_key: dict[tuple[str, int], set[str]] = {}
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
        if not _append_unique_export_entity(
            export_entities,
            entity,
            seen_entity_keys=seen_entity_keys,
            first_dxf_by_key=first_dxf_by_key,
            seen_frozen_dxf_by_key=seen_frozen_dxf_by_key,
        ):
            continue

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
        if allowed_owner_handles is not None and handle not in allowed_owner_handles:
            continue
        _append_unique_export_entity(
            export_entities,
            owner_entity,
            seen_entity_keys=seen_entity_keys,
            first_dxf_by_key=first_dxf_by_key,
            seen_frozen_dxf_by_key=seen_frozen_dxf_by_key,
        )

    return export_entities


def _append_unique_export_entity(
    export_entities: list[Entity],
    entity: Entity,
    *,
    seen_entity_keys: set[tuple[str, int]],
    first_dxf_by_key: dict[tuple[str, int], Any],
    seen_frozen_dxf_by_key: dict[tuple[str, int], set[str]],
) -> bool:
    key = (str(entity.dxftype).strip().upper(), int(entity.handle))

    frozen_signatures = seen_frozen_dxf_by_key.get(key)
    if frozen_signatures is not None:
        signature = _freeze_dxf_value(entity.dxf)
        if signature in frozen_signatures:
            return False
        frozen_signatures.add(signature)
        export_entities.append(entity)
        return True

    if key not in seen_entity_keys:
        seen_entity_keys.add(key)
        first_dxf_by_key[key] = entity.dxf
        export_entities.append(entity)
        return True

    first_dxf = first_dxf_by_key.pop(key, None)
    first_signature = _freeze_dxf_value(first_dxf)
    frozen_signatures = {first_signature}
    seen_frozen_dxf_by_key[key] = frozen_signatures

    signature = _freeze_dxf_value(entity.dxf)
    if signature in frozen_signatures:
        return False
    frozen_signatures.add(signature)
    export_entities.append(entity)
    return True


def _freeze_dxf_value(value: Any) -> str:
    normalized = _normalize_dxf_value_for_dedup(value)
    return json.dumps(normalized, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _normalize_dxf_value_for_dedup(value: Any) -> Any:
    if isinstance(value, dict):
        out: dict[str, Any] = {}
        for key in sorted(value, key=lambda item: str(item)):
            out[str(key)] = _normalize_dxf_value_for_dedup(value[key])
        return out
    if isinstance(value, (list, tuple)):
        return [_normalize_dxf_value_for_dedup(item) for item in value]
    if isinstance(value, float):
        if not math.isfinite(value):
            return str(value)
        return round(value, 12)
    if value is None or isinstance(value, (str, int, bool)):
        return value
    return str(value)


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
    explode_dimensions: bool = True,
    layer_name_by_handle: dict[int, str] | None = None,
    dim_block_policy: str = "smart",
) -> None:
    if reference_entities is None:
        reference_entities = []
        try:
            insert_entities = list(layout.query("INSERT"))
        except Exception:
            insert_entities = []
        reference_entities.extend(insert_entities)
        try:
            minsert_entities = list(layout.query("MINSERT"))
        except Exception:
            minsert_entities = []
        reference_entities.extend(minsert_entities)
        try:
            dimension_entities = list(layout.query("DIMENSION"))
        except Exception:
            dimension_entities = []
        reference_entities.extend(dimension_entities)
        if not reference_entities:
            return
    else:
        reference_entities = [
            entity
            for entity in reference_entities
            if entity.dxftype in _BLOCK_REFERENCE_ENTITY_TYPES
        ]
        if not reference_entities:
            return

    referenced_names = {
        normalized_name
        for entity in reference_entities
        for normalized_name in [_referenced_block_name_from_entity(entity)]
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

    block_name_by_handle = _resolve_block_name_by_handle(
        decode_path,
        header_rows,
        referenced_names=referenced_names,
    )
    if not block_name_by_handle:
        return

    sorted_header_rows = sorted(
        header_rows,
        key=lambda row: int(row[1]) if isinstance(row, tuple) and len(row) > 1 else 0,
    )
    block_members_by_name = _collect_block_members_by_name(
        sorted_header_rows,
        block_name_by_handle,
    )

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
    if _has_unresolved_selected_block_targets(
        selected_block_names,
        block_members_by_name,
        insert_entities_by_handle,
    ):
        # Fallback to exact BLOCK<->name mapping only when the fast map cannot
        # resolve nested INSERT targets required by selected blocks.
        exact_block_name_by_handle = _resolve_block_name_by_handle(decode_path, header_rows)
        if exact_block_name_by_handle:
            block_name_by_handle = exact_block_name_by_handle
            block_members_by_name = _collect_block_members_by_name(
                sorted_header_rows,
                block_name_by_handle,
            )
            selected_block_names = {
                name for name in referenced_names if name in block_members_by_name
            }
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

    reference_graph = _build_block_reference_graph(
        block_members_by_name,
        selected_block_names,
        insert_entities_by_handle,
    )
    recursive_targets_by_block = _collect_recursive_targets(reference_graph)

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
        prefer_open30 = _block_prefers_open30_arrowhead(export_entities)
        open30_consumed = False
        for entity in export_entities:
            recursive_target_name = (
                _normalize_block_name(entity.dxf.get("name"))
                if entity.dxftype in {"INSERT", "MINSERT"}
                else None
            )
            normalized_entity = _normalize_recursive_block_insert(
                entity,
                block_name=block_name,
                known_block_names=selected_block_names,
                recursive_target_names=recursive_targets_by_block.get(block_name),
                prefer_open30=prefer_open30 and not open30_consumed,
            )
            if normalized_entity is None:
                continue
            if recursive_target_name == block_name:
                normalized_target_name = _normalize_block_name(normalized_entity.dxf.get("name"))
                if normalized_target_name == "_Open30":
                    open30_consumed = True
            _write_entity_to_modelspace(
                block_layout,
                normalized_entity,
                explode_dimensions=explode_dimensions,
                layer_name_by_handle=layer_name_by_handle,
                dim_block_policy=dim_block_policy,
            )


def _collect_block_members_by_name(
    sorted_header_rows: list[tuple[Any, ...]],
    block_name_by_handle: dict[int, str],
) -> dict[str, list[tuple[int, str]]]:
    # Collect each BLOCK definition independently first, then choose one
    # representative definition per name. This avoids blindly taking the first
    # definition (often a minimal placeholder in some DWG variants).
    candidates_by_name: dict[str, list[tuple[int, str]]] = {}
    candidate_scores: dict[str, tuple[int, int]] = {}
    current_block_name: str | None = None
    current_members: list[tuple[int, str]] = []
    current_block_offset: int | None = None

    def _commit_current_candidate() -> None:
        nonlocal current_block_name, current_members
        if current_block_name is None:
            return
        member_count = len(current_members)
        non_point_count = sum(
            1 for _member_handle, member_type in current_members if member_type != "POINT"
        )
        score = (member_count, non_point_count)
        previous_score = candidate_scores.get(current_block_name)
        if previous_score is None or score > previous_score:
            candidate_scores[current_block_name] = score
            candidates_by_name[current_block_name] = list(current_members)

    for row in sorted_header_rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        raw_handle, raw_offset, _size, _code, raw_type_name, raw_type_class = row
        if str(raw_type_class).strip().upper() not in {"E", "ENTITY"}:
            continue
        try:
            handle = int(raw_handle)
        except Exception:
            continue
        try:
            offset = int(raw_offset)
        except Exception:
            offset = None
        type_name = str(raw_type_name).strip().upper()

        if type_name == "BLOCK":
            # Some R2010+ drawings contain shadow BLOCK records at the same
            # stream offset before any members. Keep the first mapped name to
            # avoid dropping its members (for example `_Open30`).
            if (
                current_block_name is not None
                and not current_members
                and current_block_offset is not None
                and offset is not None
                and current_block_offset == offset
            ):
                continue
            _commit_current_candidate()
            block_name = block_name_by_handle.get(handle)
            if isinstance(block_name, str) and block_name.strip() != "":
                current_block_name = block_name.strip()
                current_members = []
                current_block_offset = offset
            else:
                current_block_name = None
                current_members = []
                current_block_offset = None
            continue

        if type_name == "ENDBLK":
            _commit_current_candidate()
            current_block_name = None
            current_members = []
            current_block_offset = None
            continue

        if current_block_name is None:
            continue
        current_members.append((handle, type_name))

    _commit_current_candidate()
    return candidates_by_name


def _block_prefers_open30_arrowhead(entities: list[Entity]) -> bool:
    for entity in entities:
        if entity.dxftype not in {"TEXT", "MTEXT", "ATTRIB"}:
            continue
        text = str(entity.dxf.get("text") or "")
        if "CH" in text.upper():
            return True
    return False


def _build_block_reference_graph(
    block_members_by_name: dict[str, list[tuple[int, str]]],
    selected_block_names: set[str],
    insert_entities_by_handle: dict[int, Entity],
) -> dict[str, set[str]]:
    graph: dict[str, set[str]] = {name: set() for name in selected_block_names}
    for source_name in selected_block_names:
        for handle, raw_type_name in block_members_by_name.get(source_name, []):
            if _canonical_entity_type(raw_type_name) not in {"INSERT", "MINSERT"}:
                continue
            insert_entity = insert_entities_by_handle.get(int(handle))
            if insert_entity is None:
                continue
            target_name = _normalize_block_name(insert_entity.dxf.get("name"))
            if target_name is None:
                continue
            if target_name not in selected_block_names:
                continue
            graph[source_name].add(target_name)
    return graph


def _graph_reaches(
    graph: dict[str, set[str]],
    source: str,
    target: str,
) -> bool:
    if source == target:
        return True
    visited: set[str] = set()
    stack: list[str] = [source]
    while stack:
        current = stack.pop()
        if current in visited:
            continue
        visited.add(current)
        for neighbor in graph.get(current, set()):
            if neighbor == target:
                return True
            if neighbor not in visited:
                stack.append(neighbor)
    return False


def _collect_recursive_targets(
    graph: dict[str, set[str]],
) -> dict[str, set[str]]:
    recursive_targets: dict[str, set[str]] = {}
    for source, targets in graph.items():
        cyclic_targets = {
            target for target in targets if _graph_reaches(graph, target, source)
        }
        if cyclic_targets:
            recursive_targets[source] = cyclic_targets
    return recursive_targets


def _normalize_recursive_block_insert(
    entity: Entity,
    *,
    block_name: str,
    known_block_names: set[str],
    recursive_target_names: set[str] | None = None,
    prefer_open30: bool = False,
) -> Entity | None:
    if entity.dxftype not in {"INSERT", "MINSERT"}:
        return entity

    target_name = _normalize_block_name(entity.dxf.get("name"))
    is_recursive_target = target_name == block_name or (
        target_name is not None
        and recursive_target_names is not None
        and target_name in recursive_target_names
    )
    if not is_recursive_target:
        return entity

    fallback_names: tuple[str, ...]
    if block_name.startswith("*D"):
        # Anonymous dimension blocks often carry arrowhead INSERTs that can be
        # mis-resolved to self-references in best-effort decoding paths.
        if prefer_open30:
            fallback_names = ("_Open30", "_Small", "_CLOSEDFILLED")
        else:
            fallback_names = ("_Small", "_Open30", "_CLOSEDFILLED")
    else:
        fallback_names = ()

    for fallback in fallback_names:
        if fallback in known_block_names and fallback != block_name:
            remapped = dict(entity.dxf)
            remapped["name"] = fallback
            return Entity(dxftype=entity.dxftype, handle=entity.handle, dxf=remapped)

    # Drop unresolved recursive INSERTs to avoid cyclic block graphs.
    return None


def _has_problematic_i_inserts(entities: list[Entity]) -> bool:
    for entity in entities:
        if entity.dxftype not in {"INSERT", "MINSERT"}:
            continue
        if _normalize_block_name(entity.dxf.get("name")) == "i":
            return True
    return False


def _deduplicate_layout_pseudo_inserts_by_handle(entities: list[Entity]) -> list[Entity]:
    if not entities:
        return []
    candidate_indices: list[int] = []
    for index, entity in enumerate(entities):
        if entity.dxftype not in {"INSERT", "MINSERT"}:
            continue
        name = _normalize_block_name(entity.dxf.get("name"))
        if name is None or not _is_layout_pseudo_block_name(name):
            continue
        if not _should_preserve_layout_pseudo_insert(name, entity.dxf):
            continue
        candidate_indices.append(index)

    if len(candidate_indices) <= 1:
        return list(entities)

    # Prefer the earliest decoded pseudo layout candidate. Later variants are
    # often fallback artifacts with unstable rotation/placement.
    keep_index = candidate_indices[0]
    candidate_index_set = set(candidate_indices)
    result: list[Entity] = []
    for index, entity in enumerate(entities):
        if index in candidate_index_set and index != keep_index:
            continue
        result.append(entity)
    return result


def _available_block_names(decode_path: str) -> set[str]:
    try:
        rows = raw.decode_block_header_names(decode_path)
    except Exception:
        return set()
    names: set[str] = set()
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 2:
            continue
        normalized = _normalize_block_name(row[1])
        if normalized is not None:
            names.add(normalized)
    return names


def _normalize_problematic_insert_name(
    entity: Entity,
    *,
    available_block_names: set[str],
) -> Entity:
    if entity.dxftype not in {"INSERT", "MINSERT"}:
        return entity
    name = _normalize_block_name(entity.dxf.get("name"))
    if name != "i" or "_Open30" not in available_block_names:
        return entity
    if not _looks_like_open30_insert(entity.dxf):
        return entity
    remapped = dict(entity.dxf)
    remapped["name"] = "_Open30"
    return Entity(dxftype=entity.dxftype, handle=entity.handle, dxf=remapped)


def _looks_like_open30_insert(dxf: dict[str, Any]) -> bool:
    xscale = abs(_finite_float(dxf.get("xscale", 1.0), 1.0))
    yscale = abs(_finite_float(dxf.get("yscale", 1.0), 1.0))
    zscale = abs(_finite_float(dxf.get("zscale", 1.0), 1.0))
    scales = (xscale, yscale, zscale)
    min_scale = min(scales)
    max_scale = max(scales)
    if min_scale < _OPEN30_REMAP_SCALE_MIN or max_scale > _OPEN30_REMAP_SCALE_MAX:
        return False
    tolerance = max(1.0e-6, max_scale * 1.0e-4)
    return (max_scale - min_scale) <= tolerance


def _resolve_block_name_by_handle(
    decode_path: str,
    header_rows: list[tuple[Any, ...]],
    *,
    referenced_names: set[str] | None = None,
) -> dict[int, str]:
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

    block_handles_set = set(block_handles_in_order)

    if referenced_names is None:
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
    else:
        try:
            rows = raw.decode_block_header_names(decode_path)
        except Exception:
            rows = []

    by_header_handle: dict[int, str] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 2:
            continue
        raw_handle, raw_name = row[0], row[1]
        normalized_name = _normalize_block_name(raw_name)
        if normalized_name is None:
            continue
        try:
            by_header_handle[int(raw_handle)] = normalized_name
        except Exception:
            continue

    header_map: dict[int, str] = {
        handle: name
        for handle, name in by_header_handle.items()
        if handle in block_handles_set
    }

    # Fast-path for heavy drawings: when BLOCK_HEADER names cover all block
    # declarations and all directly referenced names are present, skip the
    # expensive exact BLOCK entity-name decode.
    if referenced_names is not None:
        candidate_map = dict(header_map)
        if len(candidate_map) < len(block_handles_in_order):
            ordered_names = [
                normalized_name
                for row in rows
                if isinstance(row, tuple) and len(row) >= 2
                for normalized_name in [_normalize_block_name(row[1])]
                if normalized_name is not None
            ]
            for index, handle in enumerate(block_handles_in_order):
                if handle in candidate_map:
                    continue
                if index >= len(ordered_names):
                    continue
                candidate_map[handle] = ordered_names[index]
        candidate_names = set(candidate_map.values())
        if referenced_names.issubset(candidate_names):
            return candidate_map

    # Prefer exact BLOCK<->name mapping from BLOCK entity names when available.
    exact_map = _resolve_block_name_by_handle_exact(decode_path)
    block_name_by_handle: dict[int, str] = {
        handle: name
        for handle, name in exact_map.items()
        if handle in block_handles_set
    }
    if len(block_name_by_handle) >= len(block_handles_in_order):
        return block_name_by_handle

    for handle in block_handles_in_order:
        if handle in block_name_by_handle:
            continue
        name = header_map.get(handle)
        if name is not None:
            block_name_by_handle[handle] = name

    if block_name_by_handle:
        return block_name_by_handle

    # Fallback for environments that mock only decode_block_entity_names.
    return exact_map


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
    pending_names: list[str] = [
        name
        for name in referenced_names
        if name in block_members_by_name
    ]
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
            if _is_layout_pseudo_block_name(nested_name):
                continue
            if nested_name not in block_members_by_name:
                continue
            if nested_name in selected_block_names or nested_name in pending_name_set:
                continue
            pending_names.append(nested_name)
            pending_name_set.add(nested_name)
    return selected_block_names


def _has_unresolved_selected_block_targets(
    selected_block_names: set[str],
    block_members_by_name: dict[str, list[tuple[int, str]]],
    insert_entities_by_handle: dict[int, Entity],
) -> bool:
    for source_name in selected_block_names:
        for handle, raw_type_name in block_members_by_name.get(source_name, []):
            if _canonical_entity_type(raw_type_name) not in {"INSERT", "MINSERT"}:
                continue
            try:
                insert_entity = insert_entities_by_handle.get(int(handle))
            except Exception:
                continue
            if insert_entity is None:
                continue
            target_name = _normalize_block_name(insert_entity.dxf.get("name"))
            if target_name is None:
                continue
            if _is_layout_pseudo_block_name(target_name):
                continue
            if target_name not in block_members_by_name:
                return True
    return False


def _referenced_block_name_from_entity(entity: Entity) -> str | None:
    if entity.dxftype in {"INSERT", "MINSERT"}:
        name = _normalize_block_name(entity.dxf.get("name"))
        if name is not None and _is_layout_pseudo_block_name(name):
            if _should_preserve_layout_pseudo_insert(name, entity.dxf):
                return name
            return None
        return name
    if entity.dxftype == "DIMENSION":
        return _dimension_anonymous_block_name(entity.dxf)
    return None


def _dimension_anonymous_block_name(dxf: dict[str, Any]) -> str | None:
    name = _normalize_block_name(dxf.get("anonymous_block_name"))
    if name is None:
        return None
    if not name.upper().startswith("*D"):
        return None
    return name


def _normalize_block_name(name: Any) -> str | None:
    if not isinstance(name, str):
        return None
    normalized = name.strip()
    if not normalized:
        return None
    return normalized


def _is_layout_pseudo_block_name(name: str) -> bool:
    upper = name.upper()
    return upper.startswith("*MODEL_SPACE") or upper.startswith("*PAPER_SPACE")


def _should_preserve_layout_pseudo_insert(name: str, dxf: dict[str, Any]) -> bool:
    # Keep only modelspace clone-like references. Paper space pseudo inserts
    # often behave as viewport artifacts and should stay skipped.
    if not name.upper().startswith("*MODEL_SPACE"):
        return False
    return _looks_like_open30_insert(dxf)


def _layout_pseudo_alias_name(name: str) -> str:
    normalized = "".join(ch if ch.isalnum() else "_" for ch in name.upper()).strip("_")
    if normalized == "":
        normalized = "LAYOUT"
    return f"__EZDWG_LAYOUT_ALIAS_{normalized}"


def _is_layout_pseudo_modelspace_alias_name(name: str) -> bool:
    return name.upper().startswith(_LAYOUT_PSEUDO_MODELSPACE_ALIAS_PREFIX)


def _ensure_layout_pseudo_block_alias(doc: Any, source_name: str) -> str | None:
    if doc is None:
        return None
    by_source = _LAYOUT_PSEUDO_ALIAS_CACHE.get(doc)
    if by_source is None:
        by_source = {}
        _LAYOUT_PSEUDO_ALIAS_CACHE[doc] = by_source
    cached = by_source.get(source_name)
    if cached:
        try:
            if doc.blocks.get(cached) is not None:
                return cached
        except Exception:
            pass

    try:
        source_block = doc.blocks.get(source_name)
    except Exception:
        source_block = None
    if source_block is None:
        return None

    alias_name = _layout_pseudo_alias_name(source_name)
    try:
        alias_block = doc.blocks.get(alias_name)
    except Exception:
        alias_block = None
    if alias_block is None:
        alias_block = doc.blocks.new(name=alias_name)
        for entity in source_block:
            if _ezdxf_entity_type(entity) == "INSERT":
                nested_name = _normalize_block_name(getattr(getattr(entity, "dxf", None), "name", None))
                if nested_name is not None:
                    if _is_layout_pseudo_block_name(nested_name):
                        continue
                    if nested_name.upper().startswith("*D"):
                        # Nested anonymous dimension graphics inside layout
                        # pseudo blocks frequently expand to scattered
                        # outliers after flattening.
                        continue
            try:
                alias_block.add_entity(entity.copy())
            except Exception:
                continue
    by_source[source_name] = alias_name
    return alias_name


def _cached_block_local_y_span(modelspace: Any, block_name: str) -> float | None:
    doc = getattr(modelspace, "doc", None)
    if doc is None:
        return None
    by_name = _BLOCK_LOCAL_Y_SPAN_CACHE.get(doc)
    if by_name is None:
        by_name = {}
        _BLOCK_LOCAL_Y_SPAN_CACHE[doc] = by_name
    if block_name in by_name:
        return by_name[block_name]

    y_values: list[float] = []
    try:
        block = doc.blocks.get(block_name)
    except Exception:
        block = None
    if block is not None:
        for entity in block:
            for _x, y in _entity_xy_points(entity):
                y_values.append(float(y))
    if not y_values:
        by_name[block_name] = None
        return None

    span = max(y_values) - min(y_values)
    if not math.isfinite(span) or span <= 0.0:
        by_name[block_name] = None
        return None
    by_name[block_name] = float(span)
    return float(span)


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


def _write_entity_to_modelspace(
    modelspace: Any,
    entity: Entity,
    *,
    explode_dimensions: bool = True,
    layer_name_by_handle: dict[int, str] | None = None,
    dim_block_policy: str = "smart",
    dimension_context: _DimensionWriteContext | None = None,
) -> bool:
    try:
        return _write_entity_to_modelspace_unsafe(
            modelspace,
            entity,
            explode_dimensions=explode_dimensions,
            layer_name_by_handle=layer_name_by_handle,
            dim_block_policy=dim_block_policy,
            dimension_context=dimension_context,
        )
    except Exception:
        return False


def _write_entity_to_modelspace_unsafe(
    modelspace: Any,
    entity: Entity,
    *,
    explode_dimensions: bool = True,
    layer_name_by_handle: dict[int, str] | None = None,
    dim_block_policy: str = "smart",
    dimension_context: _DimensionWriteContext | None = None,
) -> bool:
    dxftype = entity.dxftype
    dxf = entity.dxf
    dxfattribs = _entity_dxfattribs(dxf, layer_name_by_handle=layer_name_by_handle)
    is_modelspace_layout = bool(getattr(modelspace, "is_modelspace", False))

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
        if _distinct_xy_count(points) < 2:
            # Degenerate width-only polylines can produce invalid extents in
            # downstream renderers; keep conversion stable by dropping them.
            return True
        bulges = list(dxf.get("bulges", []) or [])
        widths = list(dxf.get("widths", []) or [])
        vertices = []
        for i, point in enumerate(points):
            start_width = 0.0
            end_width = 0.0
            if i < len(widths):
                width = widths[i]
                if isinstance(width, (list, tuple)) and len(width) >= 2:
                    start_width = _finite_float(width[0], 0.0)
                    end_width = _finite_float(width[1], 0.0)
            bulge = _finite_float(bulges[i], 0.0) if i < len(bulges) else 0.0
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
            interpolated_points = [
                _point3(point) for point in list(dxf.get("interpolated_points") or [])
            ]
            if len(interpolated_points) >= 2:
                if _distinct_xy_count(interpolated_points) < 2:
                    return True
                modelspace.add_lwpolyline(
                    [(point[0], point[1], 0.0, 0.0, 0.0) for point in interpolated_points],
                    format="xyseb",
                    close=bool(dxf.get("closed", False)),
                    dxfattribs=dxfattribs,
                )
                return True
            if len(interpolated_points) == 1:
                modelspace.add_point(interpolated_points[0], dxfattribs=dxfattribs)
                return True
            # Keep placeholder POLYLINE records from being reported as hard skips.
            return True
        if _distinct_xy_count(points) < 2:
            return True
        bulges = list(dxf.get("bulges", []) or [])
        widths = list(dxf.get("widths", []) or [])
        closed = bool(dxf.get("closed", False))
        # Keep explicit terminal duplicate vertices for open polylines:
        # some drawings represent the last segment this way even when the
        # closed flag is not set.
        if closed and len(points) > 1 and points[0] == points[-1]:
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
                    start_width = _finite_float(width[0], 0.0)
                    end_width = _finite_float(width[1], 0.0)
            bulge = _finite_float(bulges[i], 0.0) if i < len(bulges) else 0.0
            vertices.append((point[0], point[1], start_width, end_width, bulge))
        modelspace.add_lwpolyline(
            vertices,
            format="xyseb",
            close=closed,
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
            if _is_layout_pseudo_block_name(name):
                if not _should_preserve_layout_pseudo_insert(name, dxf):
                    return True
                alias_name = _ensure_layout_pseudo_block_alias(
                    getattr(modelspace, "doc", None),
                    name,
                )
                if alias_name is None:
                    return True
                name = alias_name
            insert = _point3(dxf.get("insert"))
            xscale = _finite_float(dxf.get("xscale", 1.0), 1.0)
            yscale = _finite_float(dxf.get("yscale", 1.0), 1.0)
            zscale = _finite_float(dxf.get("zscale", 1.0), 1.0)
            rotation = _finite_float(dxf.get("rotation", 0.0), 0.0)
            if is_modelspace_layout:
                if _should_skip_anonymous_dimension_block_insert(
                    modelspace,
                    name,
                    insert,
                    xscale,
                    yscale,
                    zscale,
                    rotation,
                    dim_block_policy=dim_block_policy,
                    dimension_context=dimension_context,
                ):
                    return True
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
                ref.dxf.xscale = xscale
                ref.dxf.yscale = yscale
                ref.dxf.zscale = zscale
                ref.dxf.rotation = rotation
                ref.dxf.column_count = column_count
                ref.dxf.row_count = row_count
                ref.dxf.column_spacing = _finite_float(dxf.get("column_spacing", 0.0), 0.0)
                ref.dxf.row_spacing = _finite_float(dxf.get("row_spacing", 0.0), 0.0)
                _write_insert_attributes(ref, attributes)
                return True
            except Exception:
                pass
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "INSERT":
        name = _normalize_block_name(dxf.get("name"))
        if name is not None:
            if _is_layout_pseudo_block_name(name):
                if not _should_preserve_layout_pseudo_insert(name, dxf):
                    return True
                alias_name = _ensure_layout_pseudo_block_alias(
                    getattr(modelspace, "doc", None),
                    name,
                )
                if alias_name is None:
                    return True
                name = alias_name
            insert = _point3(dxf.get("insert"))
            xscale = _finite_float(dxf.get("xscale", 1.0), 1.0)
            yscale = _finite_float(dxf.get("yscale", 1.0), 1.0)
            zscale = _finite_float(dxf.get("zscale", 1.0), 1.0)
            rotation = _finite_float(dxf.get("rotation", 0.0), 0.0)
            if is_modelspace_layout:
                if _should_skip_anonymous_dimension_block_insert(
                    modelspace,
                    name,
                    insert,
                    xscale,
                    yscale,
                    zscale,
                    rotation,
                    dim_block_policy=dim_block_policy,
                    dimension_context=dimension_context,
                ):
                    return True
            try:
                ref = modelspace.add_blockref(name, insert, dxfattribs=dxfattribs)
                ref.dxf.xscale = xscale
                ref.dxf.yscale = yscale
                ref.dxf.zscale = zscale
                ref.dxf.rotation = rotation
                _write_insert_attributes(ref, list(dxf.get("attributes") or []))
                return True
            except Exception:
                # Block definitions are not exported yet. Keep insert location visible.
                pass
        # Block name is absent or unresolved block definition is unavailable.
        modelspace.add_point(_point3(dxf.get("insert")), dxfattribs=dxfattribs)
        return True

    if dxftype == "DIMENSION":
        return _write_dimension_native(
            modelspace,
            dxf,
            dxfattribs,
            explode_dimensions=explode_dimensions,
            dim_block_policy=dim_block_policy,
            dimension_context=dimension_context,
        )

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


def _write_dimension_native(
    modelspace: Any,
    dxf: dict[str, Any],
    dxfattribs: dict[str, Any],
    *,
    explode_dimensions: bool = True,
    dim_block_policy: str = "smart",
    dimension_context: _DimensionWriteContext | None = None,
) -> bool:
    normalized_dim_block_policy = _normalize_dim_block_policy(dim_block_policy)

    def _finalize_and_track(dim: Any) -> bool:
        written = _finalize_dimension(
            modelspace,
            dim,
            dxfattribs=dxfattribs,
            explode_dimensions=explode_dimensions,
        )
        if written:
            _remember_written_dimension_block_reference(dimension_context, dxf)
        return written

    dimtype = str(dxf.get("dimtype") or "").upper()
    if dimtype.startswith("DIM_"):
        dimtype = dimtype[4:]
    anonymous_block_name = _dimension_anonymous_block_name(dxf)
    # Generic "*D" names are frequently reused across unrelated anonymous
    # dimension graphics in best-effort decode paths. Prefer native geometry
    # generation for those to avoid collapsing many dimensions onto one block.
    prefer_native_first = anonymous_block_name in {None, "*D"}
    if not prefer_native_first and _write_dimension_block_fallback(
        modelspace,
        dxf,
        dxfattribs,
        dim_block_policy=normalized_dim_block_policy,
        dimension_context=dimension_context,
    ):
        return True
    if _is_placeholder_dimension_payload(dxf):
        # Keep conversion stable for minimally decoded DIM placeholders:
        # do not generate synthetic zero-length geometry.
        if _write_dimension_block_fallback(
            modelspace,
            dxf,
            dxfattribs,
            dim_block_policy=normalized_dim_block_policy,
            dimension_context=dimension_context,
        ):
            return True
        return True
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
            return _finalize_and_track(dim)

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
            return _finalize_and_track(dim)

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
            return _finalize_and_track(dim)

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
            return _finalize_and_track(dim)

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
            return _finalize_and_track(dim)
    except Exception:
        # Keep conversion robust and avoid generating synthetic geometry lines.
        pass

    if _write_dimension_block_fallback(
        modelspace,
        dxf,
        dxfattribs,
        dim_block_policy=normalized_dim_block_policy,
        dimension_context=dimension_context,
    ):
        return True
    return _write_dimension_text_fallback(modelspace, dxf, dxfattribs)


def _dimension_block_reference_transform(
    dxf: dict[str, Any],
) -> tuple[tuple[float, float, float], float, float, float, float]:
    insert_value = dxf.get("insert")
    if insert_value is None:
        insert_value = dxf.get("text_midpoint")
    if insert_value is None:
        insert_value = dxf.get("defpoint")
    insert = _point3(insert_value)

    scale = dxf.get("insert_scale")
    xscale = 1.0
    yscale = 1.0
    zscale = 1.0
    if isinstance(scale, (list, tuple)) and len(scale) >= 3:
        xscale = _finite_float(scale[0], 1.0)
        yscale = _finite_float(scale[1], 1.0)
        zscale = _finite_float(scale[2], 1.0)
    rotation = _finite_float(dxf.get("insert_rotation"), 0.0)
    return (insert, xscale, yscale, zscale, rotation)


def _write_dimension_block_fallback(
    modelspace: Any,
    dxf: dict[str, Any],
    dxfattribs: dict[str, Any],
    *,
    dim_block_policy: str = "smart",
    dimension_context: _DimensionWriteContext | None = None,
) -> bool:
    normalized_dim_block_policy = _normalize_dim_block_policy(dim_block_policy)
    name = _dimension_anonymous_block_name(dxf)
    if name is None:
        return False

    insert, xscale, yscale, zscale, rotation = _dimension_block_reference_transform(dxf)
    if bool(getattr(modelspace, "is_modelspace", False)):
        is_empty, has_nested_dim_insert, local_center_abs = _cached_block_insert_safety_info(
            modelspace,
            name,
        )
        if is_empty:
            return False
        if has_nested_dim_insert:
            return False
        if (
            normalized_dim_block_policy == "legacy"
            and name.upper().startswith("*D")
            and max(abs(xscale), abs(yscale), abs(zscale)) >= 10.0
            and local_center_abs is not None
            and local_center_abs > 1000.0
        ):
            return False

    try:
        ref = modelspace.add_blockref(name, insert, dxfattribs=dxfattribs)
    except Exception:
        return False

    ref.dxf.xscale = xscale
    ref.dxf.yscale = yscale
    ref.dxf.zscale = zscale
    ref.dxf.rotation = rotation
    _remember_written_dimension_block_reference(dimension_context, dxf)
    return True


def _is_placeholder_dimension_payload(dxf: dict[str, Any]) -> bool:
    zero = (0.0, 0.0, 0.0)
    defpoint = _point3(dxf.get("defpoint"))
    defpoint2 = _point3(dxf.get("defpoint2"))
    defpoint3 = _point3(dxf.get("defpoint3"))
    text_midpoint = _point3(dxf.get("text_midpoint"))
    if not (
        defpoint == zero
        and defpoint2 == zero
        and defpoint3 == zero
        and text_midpoint == zero
    ):
        return False

    insert_value = dxf.get("insert")
    if insert_value is not None and _point3(insert_value) != zero:
        return False

    text = str(dxf.get("text") or "").strip()
    if text not in {"", "<>"}:
        return False
    return dxf.get("actual_measurement") is None


def _finalize_dimension(
    modelspace: Any,
    dim: Any,
    *,
    dxfattribs: dict[str, Any],
    explode_dimensions: bool,
) -> bool:
    dim.render()
    if not explode_dimensions:
        return True

    dimension_entity = getattr(dim, "dimension", None)
    if dimension_entity is None:
        return True

    try:
        virtual_entities = list(dimension_entity.virtual_entities())
    except Exception:
        return True

    if not virtual_entities:
        return True

    written = 0
    for virtual_entity in virtual_entities:
        if virtual_entity.dxftype() == "POINT":
            # DEFPOINT-like helper markers are usually not rendered in CAD viewers.
            continue
        try:
            clone = virtual_entity.copy()
        except Exception:
            continue
        if "color" in dxfattribs and hasattr(clone.dxf, "color"):
            try:
                clone.dxf.color = int(dxfattribs["color"])
            except Exception:
                pass
        if "true_color" in dxfattribs and hasattr(clone.dxf, "true_color"):
            try:
                clone.dxf.true_color = int(dxfattribs["true_color"])
            except Exception:
                pass
        try:
            modelspace.add_entity(clone)
            written += 1
        except Exception:
            continue

    if written <= 0:
        return True

    try:
        modelspace.delete_entity(dimension_entity)
    except Exception:
        pass
    return True


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


def _entity_dxfattribs(
    dxf: dict[str, Any],
    *,
    layer_name_by_handle: dict[int, str] | None = None,
) -> dict[str, Any]:
    attribs: dict[str, Any] = {}
    if isinstance(layer_name_by_handle, dict):
        layer_handle = dxf.get("layer_handle")
        try:
            layer_handle_int = int(layer_handle) if layer_handle is not None else None
        except Exception:
            layer_handle_int = None
        if layer_handle_int is not None:
            layer_name = layer_name_by_handle.get(layer_handle_int)
            if isinstance(layer_name, str) and layer_name:
                attribs["layer"] = layer_name

    raw_color_index = dxf.get("color_index")
    color = _to_valid_aci(raw_color_index)
    if color is None and raw_color_index is None:
        color = _to_valid_aci(dxf.get("resolved_color_index"))
    if color is not None:
        attribs["color"] = color

    raw_true_color = dxf.get("true_color")
    true_color = _to_valid_true_color(raw_true_color)
    if true_color is None and raw_true_color is None:
        true_color = _to_valid_true_color(dxf.get("resolved_true_color"))
    if true_color is not None:
        attribs["true_color"] = true_color
    return attribs


def _layer_styles_by_handle(decode_path: str | None) -> dict[int, tuple[int, int | None]]:
    if not decode_path:
        return {}
    try:
        rows = raw.decode_layer_colors(decode_path)
    except Exception:
        return {}
    styles: dict[int, tuple[int, int | None]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 3:
            continue
        try:
            handle = int(row[0])
            index = int(row[1])
        except Exception:
            continue
        true_color = row[2]
        try:
            true_color_int = int(true_color) if true_color is not None else None
        except Exception:
            true_color_int = None
        styles[handle] = (index, true_color_int)
    return styles


def _prepare_dxf_layers(
    dxf_doc: Any,
    layer_styles_by_handle: dict[int, tuple[int, int | None]],
) -> dict[int, str]:
    mapping: dict[int, str] = {0: "0"}
    for handle in sorted(layer_styles_by_handle):
        if handle <= 0:
            continue
        name = f"LAYER_{handle:X}"
        mapping[handle] = name
        dxfattribs: dict[str, Any] = {}
        style = layer_styles_by_handle.get(handle)
        if style is not None:
            index, true_color = style
            color = _to_valid_aci(index)
            if color is not None:
                dxfattribs["color"] = color
            resolved_true = _to_valid_true_color(true_color)
            if resolved_true is not None:
                dxfattribs["true_color"] = resolved_true
        try:
            dxf_doc.layers.new(name=name, dxfattribs=dxfattribs or None)
        except Exception:
            continue
    return mapping


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


def _validate_coord(value: Any) -> float:
    coord = float(value)
    if not math.isfinite(coord):
        raise ValueError(f"invalid coordinate value: {value!r}")
    if abs(coord) > _MAX_COORD_ABS:
        raise ValueError(f"coordinate out of supported range: {coord!r}")
    return coord


def _finite_float(value: Any, default: float) -> float:
    try:
        parsed = float(value)
    except Exception:
        return float(default)
    if not math.isfinite(parsed):
        return float(default)
    if abs(parsed) > _MAX_COORD_ABS:
        return float(default)
    return parsed


def _distinct_xy_count(points: list[tuple[float, float, float]]) -> int:
    return len({(float(point[0]), float(point[1])) for point in points})


def _normalize_dim_block_policy(policy: str) -> str:
    token = str(policy or "").strip().lower()
    if token in {"", "smart", "auto", "default"}:
        return "smart"
    if token in _DIM_BLOCK_POLICIES:
        return token
    allowed = ", ".join(sorted(_DIM_BLOCK_POLICIES))
    raise ValueError(f"unsupported dim-block policy: {policy!r} (expected one of: {allowed})")


def _quantize_dim_block_value(value: float) -> float:
    return round(float(value), 9)


def _normalized_angle_degrees(value: float) -> float:
    normalized = float(value) % 360.0
    if abs(normalized) <= 1.0e-9:
        return 0.0
    if abs(normalized - 360.0) <= 1.0e-9:
        return 0.0
    return normalized


def _anonymous_dimension_block_ref_key(
    block_name: str,
    insert: tuple[float, float, float],
    xscale: float,
    yscale: float,
    zscale: float,
    rotation: float,
) -> tuple[str, tuple[float, float, float], tuple[float, float, float], float] | None:
    if not block_name.upper().startswith("*D"):
        return None
    return (
        block_name.strip().upper(),
        (
            _quantize_dim_block_value(insert[0]),
            _quantize_dim_block_value(insert[1]),
            _quantize_dim_block_value(insert[2]),
        ),
        (
            _quantize_dim_block_value(xscale),
            _quantize_dim_block_value(yscale),
            _quantize_dim_block_value(zscale),
        ),
        _quantize_dim_block_value(_normalized_angle_degrees(rotation)),
    )


def _remember_written_dimension_block_reference(
    dimension_context: _DimensionWriteContext | None,
    dxf: dict[str, Any],
) -> None:
    if dimension_context is None:
        return
    block_name = _dimension_anonymous_block_name(dxf)
    if block_name is None:
        return
    insert, xscale, yscale, zscale, rotation = _dimension_block_reference_transform(dxf)
    key = _anonymous_dimension_block_ref_key(block_name, insert, xscale, yscale, zscale, rotation)
    if key is None:
        return
    dimension_context.written_block_refs.add(key)


def _should_skip_anonymous_dimension_block_insert(
    modelspace: Any,
    block_name: str,
    insert: tuple[float, float, float],
    xscale: float,
    yscale: float,
    zscale: float,
    rotation: float,
    *,
    dim_block_policy: str,
    dimension_context: _DimensionWriteContext | None,
) -> bool:
    is_empty, has_nested_dim_insert, local_center_abs = _cached_block_insert_safety_info(
        modelspace,
        block_name,
    )
    if is_empty:
        return True
    if not block_name.upper().startswith("*D"):
        return False

    normalized_policy = _normalize_dim_block_policy(dim_block_policy)
    if has_nested_dim_insert:
        return True
    if _is_placeholder_anonymous_dimension_insert(
        block_name,
        insert,
        xscale,
        yscale,
        zscale,
        rotation,
    ):
        return True

    if normalized_policy == "legacy":
        if (
            max(abs(xscale), abs(yscale), abs(zscale)) >= 10.0
            and local_center_abs is not None
            and local_center_abs > 1000.0
        ):
            return True
        return False

    if dimension_context is None:
        return False

    key = _anonymous_dimension_block_ref_key(
        block_name,
        insert,
        xscale,
        yscale,
        zscale,
        rotation,
    )
    if key is None:
        return False
    return key in dimension_context.written_block_refs


def _prepare_insert_for_flatten(modelspace: Any, insert: Any) -> bool:
    try:
        block_name = _normalize_block_name(getattr(insert.dxf, "name", None))
    except Exception:
        block_name = None
    if block_name is None:
        return False
    if _is_layout_pseudo_block_name(block_name):
        return True

    try:
        insert_point = (
            float(getattr(insert.dxf.insert, "x", 0.0)),
            float(getattr(insert.dxf.insert, "y", 0.0)),
            float(getattr(insert.dxf.insert, "z", 0.0)),
        )
    except Exception:
        insert_point = (0.0, 0.0, 0.0)
    xscale = _finite_float(getattr(insert.dxf, "xscale", 1.0), 1.0)
    yscale = _finite_float(getattr(insert.dxf, "yscale", 1.0), 1.0)
    zscale = _finite_float(getattr(insert.dxf, "zscale", 1.0), 1.0)
    rotation = _finite_float(getattr(insert.dxf, "rotation", 0.0), 0.0)

    is_empty, has_nested_dim_insert, local_center_abs = _cached_block_insert_safety_info(
        modelspace,
        block_name,
    )
    if is_empty:
        return True

    if block_name.upper().startswith("*D"):
        if has_nested_dim_insert:
            return True
        if _is_placeholder_anonymous_dimension_insert(
            block_name,
            insert_point,
            xscale,
            yscale,
            zscale,
            rotation,
        ):
            return True

    # Flattening applies INSERT transforms to contained entities. For blocks
    # that already carry world-like coordinates, large scales produce extreme
    # outliers and collapse the visible drawing in lightweight renderers.
    # Normalize scale before explode instead of dropping potentially useful
    # geometry.
    if (
        max(abs(xscale), abs(yscale), abs(zscale)) >= 10.0
        and local_center_abs is not None
        and local_center_abs > 1000.0
    ):
        try:
            insert.dxf.xscale = 1.0
            insert.dxf.yscale = 1.0
            insert.dxf.zscale = 1.0
            if _is_layout_pseudo_modelspace_alias_name(block_name):
                # Layout pseudo aliases represent viewport-like snapshots.
                # Some files carry 90/270-degree rotations that should be
                # normalized to the landscape orientation before explode.
                local_y_span = _cached_block_local_y_span(modelspace, block_name)
                normalized_rotation = _normalized_angle_degrees(rotation)
                if (
                    local_y_span is not None
                    and 45.0 <= normalized_rotation <= 135.0
                ):
                    normalized_insert_y = insert_point[1]
                    if local_center_abs is not None and local_center_abs > 1000.0:
                        # Layout pseudo aliases often embed world coordinates in
                        # block-local space; keeping insert.y shifts the exploded
                        # copy upward by that offset.
                        normalized_insert_y = 0.0
                    insert.dxf.insert = (
                        insert_point[0] - local_y_span,
                        normalized_insert_y,
                        insert_point[2],
                    )
                    insert.dxf.rotation = 0.0
                elif (
                    local_y_span is not None
                    and 225.0 <= normalized_rotation <= 315.0
                ):
                    insert.dxf.insert = (
                        insert_point[0],
                        insert_point[1] - local_y_span,
                        insert_point[2],
                    )
                    insert.dxf.rotation = 180.0
        except Exception:
            # If normalization is unavailable, keep the entity so explode()
            # can still attempt best-effort expansion.
            pass
        return False
    return False


def _cached_block_insert_safety_info(
    modelspace: Any,
    block_name: str,
) -> tuple[bool, bool, float | None]:
    doc = getattr(modelspace, "doc", None)
    if doc is None:
        return (False, False, None)

    by_name = _BLOCK_INSERT_SAFETY_CACHE.get(doc)
    if by_name is None:
        by_name = {}
        _BLOCK_INSERT_SAFETY_CACHE[doc] = by_name
    cached = by_name.get(block_name)
    if cached is not None:
        return cached

    try:
        block = doc.blocks.get(block_name)
    except Exception:
        info = (False, False, None)
        by_name[block_name] = info
        return info
    if block is None:
        info = (False, False, None)
        by_name[block_name] = info
        return info

    entities = list(block)
    is_empty = len(entities) == 0
    has_nested_dim_insert = False
    local_center_abs: float | None = None
    min_x = math.inf
    min_y = math.inf
    max_x = -math.inf
    max_y = -math.inf

    def _push_xy(x: Any, y: Any) -> None:
        nonlocal min_x, min_y, max_x, max_y
        try:
            x_val = float(x)
            y_val = float(y)
        except Exception:
            return
        if not (math.isfinite(x_val) and math.isfinite(y_val)):
            return
        min_x = min(min_x, x_val)
        min_y = min(min_y, y_val)
        max_x = max(max_x, x_val)
        max_y = max(max_y, y_val)

    for entity in entities:
        dxftype = entity.dxftype()
        dxf = entity.dxf
        if dxftype in {"INSERT", "MINSERT"}:
            nested_name = _normalize_block_name(getattr(dxf, "name", None))
            if nested_name is not None and nested_name.upper().startswith("*D"):
                has_nested_dim_insert = True
            _push_xy(getattr(dxf.insert, "x", 0.0), getattr(dxf.insert, "y", 0.0))
            continue
        if dxftype == "LINE":
            _push_xy(getattr(dxf.start, "x", 0.0), getattr(dxf.start, "y", 0.0))
            _push_xy(getattr(dxf.end, "x", 0.0), getattr(dxf.end, "y", 0.0))
            continue
        if dxftype == "POINT":
            _push_xy(getattr(dxf.location, "x", 0.0), getattr(dxf.location, "y", 0.0))
            continue
        if dxftype in {"ARC", "CIRCLE"}:
            _push_xy(getattr(dxf.center, "x", 0.0), getattr(dxf.center, "y", 0.0))
            continue
        if dxftype in {"TEXT", "MTEXT"}:
            _push_xy(getattr(dxf.insert, "x", 0.0), getattr(dxf.insert, "y", 0.0))
            continue
        if dxftype == "LWPOLYLINE":
            try:
                for point in entity.get_points("xy"):
                    if len(point) >= 2:
                        _push_xy(point[0], point[1])
            except Exception:
                continue

    if math.isfinite(min_x) and math.isfinite(min_y):
        center_x = (min_x + max_x) * 0.5
        center_y = (min_y + max_y) * 0.5
        local_center_abs = max(abs(center_x), abs(center_y))

    info = (is_empty, has_nested_dim_insert, local_center_abs)
    by_name[block_name] = info
    return info


def _is_placeholder_anonymous_dimension_insert(
    block_name: str,
    insert: tuple[float, float, float],
    xscale: float,
    yscale: float,
    zscale: float,
    rotation: float,
) -> bool:
    if not block_name.upper().startswith("*D"):
        return False
    near_zero = 1.0e-9
    if (
        abs(insert[0]) <= near_zero
        and abs(insert[1]) <= near_zero
        and abs(insert[2]) <= near_zero
        and abs(xscale - 1.0) <= near_zero
        and abs(yscale - 1.0) <= near_zero
        and abs(zscale - 1.0) <= near_zero
        and abs(rotation) <= near_zero
    ):
        return True
    return False


def _point3(value: Any) -> tuple[float, float, float]:
    if value is None:
        return (0.0, 0.0, 0.0)
    if isinstance(value, (list, tuple)):
        if len(value) >= 3:
            return (
                _validate_coord(value[0]),
                _validate_coord(value[1]),
                _validate_coord(value[2]),
            )
        if len(value) >= 2:
            return (
                _validate_coord(value[0]),
                _validate_coord(value[1]),
                0.0,
            )
    raise ValueError(f"invalid point value: {value!r}")


def _point2(value: Any) -> tuple[float, float]:
    if value is None:
        raise ValueError("invalid point value: None")
    if isinstance(value, (list, tuple)):
        if len(value) >= 2:
            return (_validate_coord(value[0]), _validate_coord(value[1]))
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
