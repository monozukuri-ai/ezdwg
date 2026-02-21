from __future__ import annotations

import fnmatch
import math
import re
from functools import lru_cache
from dataclasses import dataclass
from typing import Iterable, Iterator

from . import raw
from .entity import Entity

SUPPORTED_VERSIONS = {"AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"}
SUPPORTED_ENTITY_TYPES = (
    "LINE",
    "LWPOLYLINE",
    "POLYLINE_2D",
    "VERTEX_2D",
    "POLYLINE_3D",
    "VERTEX_3D",
    "POLYLINE_MESH",
    "VERTEX_MESH",
    "POLYLINE_PFACE",
    "VERTEX_PFACE",
    "VERTEX_PFACE_FACE",
    "SEQEND",
    "3DFACE",
    "SOLID",
    "TRACE",
    "SHAPE",
    "3DSOLID",
    "BODY",
    "VIEWPORT",
    "OLEFRAME",
    "OLE2FRAME",
    "REGION",
    "RAY",
    "XLINE",
    "ARC",
    "CIRCLE",
    "ELLIPSE",
    "SPLINE",
    "POINT",
    "TEXT",
    "ATTRIB",
    "ATTDEF",
    "MTEXT",
    "LEADER",
    "HATCH",
    "TOLERANCE",
    "MLINE",
    "BLOCK",
    "ENDBLK",
    "INSERT",
    "MINSERT",
    "DIMENSION",
)

TYPE_ALIASES = {
    "DIM_LINEAR": "DIMENSION",
    "DIM_RADIUS": "DIMENSION",
    "DIM_DIAMETER": "DIMENSION",
    "DIM_ORDINATE": "DIMENSION",
    "DIM_ALIGNED": "DIMENSION",
    "DIM_ANG3PT": "DIMENSION",
    "DIM_ANG2LN": "DIMENSION",
}

_BULK_PRIMITIVE_TYPES = {"LINE", "ARC", "CIRCLE"}
_EXPLICIT_ONLY_ENTITY_TYPES = {
    "BLOCK",
    "ENDBLK",
    "SEQEND",
    "VERTEX_2D",
    "VERTEX_3D",
    "VERTEX_MESH",
    "VERTEX_PFACE",
    "VERTEX_PFACE_FACE",
}
_POLYLINE_2D_INTERPOLATION_SEGMENTS = 8
_POLYLINE_2D_SPLINE_CURVE_TYPES = {"QuadraticBSpline", "CubicBSpline", "Bezier"}
_ACIS_ROLE_HINTS = {
    0x214: "acis-link-table",
    0x221: "acis-header",
    0x222: "acis-payload-chunk",
    0x223: "acis-payload-chunk",
    0x224: "acis-payload-chunk",
    0x225: "acis-payload-chunk",
}


def read(path: str) -> "Document":
    version = raw.detect_version(path)
    if version not in SUPPORTED_VERSIONS:
        raise ValueError(f"unsupported DWG version: {version}")
    return Document(path=path, version=version)


@dataclass(frozen=True)
class Document:
    path: str
    version: str
    decode_path: str | None = None
    decode_version: str | None = None

    def __post_init__(self) -> None:
        if self.decode_path is None:
            object.__setattr__(self, "decode_path", self.path)
        if self.decode_version is None:
            object.__setattr__(self, "decode_version", self.version)

    def modelspace(self) -> "Layout":
        return Layout(self, "MODELSPACE")

    def plot(self, *args, **kwargs):
        from .render import plot

        return plot(self, *args, **kwargs)

    def export_dxf(self, output_path: str, **kwargs):
        from .convert import to_dxf

        return to_dxf(self, output_path, **kwargs)

    @property
    def raw(self):
        return raw


@dataclass(frozen=True)
class Layout:
    doc: Document
    name: str

    def iter_entities(self, types: str | Iterable[str] | None = None) -> Iterator[Entity]:
        return self.query(types)

    def query(self, types: str | Iterable[str] | None = None) -> Iterator[Entity]:
        type_set = _normalize_types(types, self.doc.decode_path)
        bulk_rows = None
        if sum(1 for dxftype in type_set if dxftype in _BULK_PRIMITIVE_TYPES) >= 2:
            bulk_rows = _line_arc_circle_rows(self.doc.decode_path)
        for dxftype in type_set:
            yield from self._iter_type(dxftype, bulk_rows=bulk_rows)

    def plot(self, *args, **kwargs):
        from .render import plot

        return plot(self, *args, **kwargs)

    def export_dxf(self, output_path: str, **kwargs):
        from .convert import to_dxf

        return to_dxf(self, output_path, **kwargs)

    def _iter_type(
        self,
        dxftype: str,
        *,
        bulk_rows: tuple[
            list[tuple[int, float, float, float, float, float, float]],
            list[tuple[int, float, float, float, float, float, float]],
            list[tuple[int, float, float, float, float]],
        ]
        | None = None,
    ) -> Iterator[Entity]:
        decode_path = self.doc.decode_path
        entity_style_map = _entity_style_map(decode_path)
        layer_color_map = _layer_color_map(decode_path)
        layer_color_overrides = _layer_color_overrides(
            self.doc.decode_version, entity_style_map, layer_color_map
        )
        if dxftype == "LINE":
            if bulk_rows is not None:
                line_rows = bulk_rows[0]
            else:
                line_rows = list(raw.decode_line_entities(decode_path))
            line_supplementary_handles = _line_supplementary_handles(
                line_rows, entity_style_map, layer_color_overrides
            )
            for handle, sx, sy, sz, ex, ey, ez in line_rows:
                dxf = _attach_entity_color(
                    handle,
                    {
                        "start": (sx, sy, sz),
                        "end": (ex, ey, ez),
                    },
                    entity_style_map,
                    layer_color_map,
                    layer_color_overrides,
                    dxftype="LINE",
                )
                if handle in line_supplementary_handles:
                    dxf["resolved_color_index"] = 9
                    dxf["resolved_true_color"] = None
                yield Entity(
                    dxftype="LINE",
                    handle=handle,
                    dxf=dxf,
                )
            return

        if dxftype == "ARC":
            arc_rows = bulk_rows[1] if bulk_rows is not None else raw.decode_arc_entities(decode_path)
            for handle, cx, cy, cz, radius, start_angle, end_angle in arc_rows:
                start_deg = math.degrees(start_angle)
                end_deg = math.degrees(end_angle)
                yield Entity(
                    dxftype="ARC",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "center": (cx, cy, cz),
                            "radius": radius,
                            "start_angle": start_deg,
                            "end_angle": end_deg,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="ARC",
                    ),
                )
            return

        if dxftype == "LWPOLYLINE":
            for (
                handle,
                flags,
                points,
                bulges,
                widths,
                const_width,
            ) in raw.decode_lwpolyline_entities(decode_path):
                points3d = [(x, y, 0.0) for x, y in points]
                bulges_list = list(bulges)
                if len(bulges_list) < len(points3d):
                    bulges_list.extend([0.0] * (len(points3d) - len(bulges_list)))
                elif len(bulges_list) > len(points3d):
                    bulges_list = bulges_list[: len(points3d)]

                widths_list = list(widths)
                if not widths_list and const_width is not None and points3d:
                    widths_list = [(const_width, const_width)] * len(points3d)
                if len(widths_list) < len(points3d):
                    widths_list.extend([(0.0, 0.0)] * (len(points3d) - len(widths_list)))
                elif len(widths_list) > len(points3d):
                    widths_list = widths_list[: len(points3d)]
                yield Entity(
                    dxftype="LWPOLYLINE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": points3d,
                            "flags": flags,
                            "closed": bool(flags & 1),
                            "bulges": bulges_list,
                            "widths": widths_list,
                            "const_width": const_width,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="LWPOLYLINE",
                    ),
                )
            return

        if dxftype == "POLYLINE_2D":
            interpreted_map = _polyline_2d_interpreted_map(decode_path)
            interpolated_points_map = _polyline_2d_interpolated_points_map(
                decode_path, interpreted_map
            )
            polyline_sequence_map, _, _ = _polyline_sequence_relationships(decode_path)
            for handle, flags, vertices in raw.decode_polyline_2d_with_vertex_data(decode_path):
                points = []
                bulges = []
                widths = []
                tangent_dirs = []
                vertex_flags = []
                for vertex in vertices:
                    (
                        x,
                        y,
                        z,
                        start_width,
                        end_width,
                        bulge,
                        tangent_dir,
                        vertex_flag,
                    ) = vertex
                    points.append((x, y, z))
                    bulges.append(bulge)
                    widths.append((start_width, end_width))
                    tangent_dirs.append(tangent_dir)
                    vertex_flags.append(int(vertex_flag))

                flags_info = _polyline_2d_flags_info(int(flags))
                interpreted = interpreted_map.get(int(handle))
                curve_type = None
                curve_type_label = None
                if interpreted is not None:
                    flags_info.update(
                        {
                            "closed": bool(interpreted.get("closed", flags_info["closed"])),
                            "curve_fit": bool(interpreted.get("curve_fit", flags_info["curve_fit"])),
                            "spline_fit": bool(interpreted.get("spline_fit", flags_info["spline_fit"])),
                            "is_3d_polyline": bool(
                                interpreted.get("is_3d_polyline", flags_info["is_3d_polyline"])
                            ),
                            "is_3d_mesh": bool(
                                interpreted.get("is_3d_mesh", flags_info["is_3d_mesh"])
                            ),
                            "is_closed_mesh": bool(
                                interpreted.get("is_closed_mesh", flags_info["is_closed_mesh"])
                            ),
                            "is_polyface_mesh": bool(
                                interpreted.get("is_polyface_mesh", flags_info["is_polyface_mesh"])
                            ),
                            "continuous_linetype": bool(
                                interpreted.get(
                                    "continuous_linetype",
                                    flags_info["continuous_linetype"],
                                )
                            ),
                        }
                    )
                    if interpreted.get("curve_type") is not None:
                        curve_type = int(interpreted["curve_type"])
                    if interpreted.get("curve_type_label") is not None:
                        curve_type_label = str(interpreted["curve_type_label"])

                closed = bool(flags_info["closed"])
                if closed and len(points) > 1:
                    points = _strip_duplicate_closure_point(points)
                    if bulges:
                        bulges.pop()
                    if widths:
                        widths.pop()
                    if tangent_dirs:
                        tangent_dirs.pop()
                    if vertex_flags:
                        vertex_flags.pop()

                should_interpolate = _polyline_2d_should_interpolate(
                    bool(flags_info["curve_fit"]),
                    bool(flags_info["spline_fit"]),
                    curve_type_label,
                )
                interpolated_points = list(interpolated_points_map.get(int(handle), []))
                interpolation_applied = bool(interpolated_points)
                sequence_info = polyline_sequence_map.get(int(handle), {})

                yield Entity(
                    dxftype="POLYLINE_2D",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": points,
                            "flags": int(flags),
                            "closed": closed,
                            "bulges": bulges,
                            "widths": widths,
                            "tangent_dirs": tangent_dirs,
                            "vertex_flags": vertex_flags,
                            "curve_type": curve_type,
                            "curve_type_label": curve_type_label,
                            "curve_fit": bool(flags_info["curve_fit"]),
                            "spline_fit": bool(flags_info["spline_fit"]),
                            "is_3d_polyline": bool(flags_info["is_3d_polyline"]),
                            "is_3d_mesh": bool(flags_info["is_3d_mesh"]),
                            "is_closed_mesh": bool(flags_info["is_closed_mesh"]),
                            "is_polyface_mesh": bool(flags_info["is_polyface_mesh"]),
                            "continuous_linetype": bool(flags_info["continuous_linetype"]),
                            "should_interpolate": should_interpolate,
                            "interpolation_applied": interpolation_applied,
                            "interpolated_points": interpolated_points,
                            "vertex_handles": list(sequence_info.get("vertex_handles", [])),
                            "seqend_handle": sequence_info.get("seqend_handle"),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_2D",
                    ),
                )
            return

        if dxftype == "VERTEX_2D":
            _, vertex_owner_map, _ = _polyline_sequence_relationships(decode_path)
            for (
                handle,
                flags,
                x,
                y,
                z,
                start_width,
                end_width,
                bulge,
                tangent_dir,
            ) in raw.decode_vertex_2d_entities(decode_path):
                owner_handle, owner_type = vertex_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="VERTEX_2D",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "position": (x, y, z),
                            "flags": int(flags),
                            "start_width": float(start_width),
                            "end_width": float(end_width),
                            "bulge": float(bulge),
                            "tangent_dir": float(tangent_dir),
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VERTEX_2D",
                    ),
                )
            return

        if dxftype == "POLYLINE_3D":
            polyline_sequence_map, _, _ = _polyline_sequence_relationships(decode_path)
            for handle, flags_70_bits, closed, points in raw.decode_polyline_3d_with_vertices(
                decode_path
            ):
                sequence_info = polyline_sequence_map.get(int(handle), {})
                yield Entity(
                    dxftype="POLYLINE_3D",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": list(points),
                            "flags": int(flags_70_bits),
                            "closed": bool(closed),
                            "vertex_handles": list(sequence_info.get("vertex_handles", [])),
                            "seqend_handle": sequence_info.get("seqend_handle"),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_3D",
                    ),
                )
            return

        if dxftype == "VERTEX_3D":
            _, vertex_owner_map, _ = _polyline_sequence_relationships(decode_path)
            for handle, flags, x, y, z in raw.decode_vertex_3d_entities(decode_path):
                owner_handle, owner_type = vertex_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="VERTEX_3D",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "position": (x, y, z),
                            "flags": int(flags),
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VERTEX_3D",
                    ),
                )
            return

        if dxftype == "POLYLINE_MESH":
            polyline_sequence_map, _, _ = _polyline_sequence_relationships(decode_path)
            for (
                handle,
                flags,
                m_vertex_count,
                n_vertex_count,
                closed,
                points,
            ) in raw.decode_polyline_mesh_with_vertices(decode_path):
                sequence_info = polyline_sequence_map.get(int(handle), {})
                yield Entity(
                    dxftype="POLYLINE_MESH",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": list(points),
                            "flags": int(flags),
                            "m_vertex_count": int(m_vertex_count),
                            "n_vertex_count": int(n_vertex_count),
                            "closed": bool(closed),
                            "vertex_handles": list(sequence_info.get("vertex_handles", [])),
                            "seqend_handle": sequence_info.get("seqend_handle"),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_MESH",
                    ),
                )
            return

        if dxftype == "VERTEX_MESH":
            _, vertex_owner_map, _ = _polyline_sequence_relationships(decode_path)
            for handle, flags, x, y, z in raw.decode_vertex_mesh_entities(decode_path):
                owner_handle, owner_type = vertex_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="VERTEX_MESH",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "position": (x, y, z),
                            "flags": int(flags),
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VERTEX_MESH",
                    ),
                )
            return

        if dxftype == "POLYLINE_PFACE":
            polyline_sequence_map, _, _ = _polyline_sequence_relationships(decode_path)
            for (
                handle,
                num_vertices,
                num_faces,
                vertices,
                faces,
            ) in raw.decode_polyline_pface_with_faces(decode_path):
                sequence_info = polyline_sequence_map.get(int(handle), {})
                yield Entity(
                    dxftype="POLYLINE_PFACE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "num_vertices": int(num_vertices),
                            "num_faces": int(num_faces),
                            "vertices": list(vertices),
                            "faces": list(faces),
                            "vertex_handles": list(sequence_info.get("vertex_handles", [])),
                            "face_handles": list(sequence_info.get("face_handles", [])),
                            "seqend_handle": sequence_info.get("seqend_handle"),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_PFACE",
                    ),
                )
            return

        if dxftype == "VERTEX_PFACE":
            _, vertex_owner_map, _ = _polyline_sequence_relationships(decode_path)
            for handle, flags, x, y, z in raw.decode_vertex_pface_entities(decode_path):
                owner_handle, owner_type = vertex_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="VERTEX_PFACE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "position": (x, y, z),
                            "flags": int(flags),
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VERTEX_PFACE",
                    ),
                )
            return

        if dxftype == "VERTEX_PFACE_FACE":
            _, vertex_owner_map, _ = _polyline_sequence_relationships(decode_path)
            for handle, index1, index2, index3, index4 in raw.decode_vertex_pface_face_entities(
                decode_path
            ):
                owner_handle, owner_type = vertex_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="VERTEX_PFACE_FACE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "indices": (
                                int(index1),
                                int(index2),
                                int(index3),
                                int(index4),
                            ),
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VERTEX_PFACE_FACE",
                    ),
                )
            return

        if dxftype == "SEQEND":
            _, _, seqend_owner_map = _polyline_sequence_relationships(decode_path)
            for handle in _entity_handles_by_type_name(decode_path, "SEQEND"):
                owner_handle, owner_type = seqend_owner_map.get(int(handle), (None, None))
                yield Entity(
                    dxftype="SEQEND",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "owner_handle": owner_handle,
                            "owner_type": owner_type,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="SEQEND",
                    ),
                )
            return

        if dxftype == "3DFACE":
            for handle, p1, p2, p3, p4, invisible_edge_flags in raw.decode_3dface_entities(
                decode_path
            ):
                yield Entity(
                    dxftype="3DFACE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": [p1, p2, p3, p4],
                            "invisible_edge_flags": int(invisible_edge_flags),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="3DFACE",
                    ),
                )
            return

        if dxftype == "SOLID":
            for handle, p1, p2, p3, p4, thickness, extrusion in raw.decode_solid_entities(
                decode_path
            ):
                yield Entity(
                    dxftype="SOLID",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": [p1, p2, p3, p4],
                            "thickness": thickness,
                            "extrusion": extrusion,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="SOLID",
                    ),
                )
            return

        if dxftype == "TRACE":
            for handle, p1, p2, p3, p4, thickness, extrusion in raw.decode_trace_entities(
                decode_path
            ):
                yield Entity(
                    dxftype="TRACE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": [p1, p2, p3, p4],
                            "thickness": thickness,
                            "extrusion": extrusion,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="TRACE",
                    ),
                )
            return

        if dxftype == "SHAPE":
            for (
                handle,
                insertion,
                scale,
                rotation,
                width_factor,
                oblique,
                thickness,
                shape_no,
                extrusion,
                shapefile_handle,
            ) in raw.decode_shape_entities(decode_path):
                yield Entity(
                    dxftype="SHAPE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "insert": insertion,
                            "scale": scale,
                            "rotation": math.degrees(rotation),
                            "width": width_factor,
                            "oblique": math.degrees(oblique),
                            "thickness": thickness,
                            "shape_no": int(shape_no),
                            "extrusion": extrusion,
                            "shapefile_handle": shapefile_handle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="SHAPE",
                    ),
                )
            return

        if dxftype == "3DSOLID":
            acis_candidate_map = _acis_candidate_handles_map(decode_path)
            acis_record_map = _acis_candidate_record_map(decode_path)
            for row in raw.decode_3dsolid_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                acis_handles = _normalize_int_handles(row[1] if len(row) >= 2 else [])
                dxf = _attach_entity_color(
                    handle,
                    {
                        "acis_handles": acis_handles,
                    },
                    entity_style_map,
                    layer_color_map,
                    layer_color_overrides,
                    dxftype="3DSOLID",
                )
                layer_handle = dxf.get("layer_handle")
                if isinstance(layer_handle, int):
                    dxf["acis_handles"] = [h for h in acis_handles if h != layer_handle]
                candidate_handles = list(acis_candidate_map.get(handle, ()))
                dxf.update(_build_acis_entity_payload(handle, candidate_handles, acis_record_map))
                yield Entity(
                    dxftype="3DSOLID",
                    handle=handle,
                    dxf=dxf,
                )
            return

        if dxftype == "BODY":
            acis_candidate_map = _acis_candidate_handles_map(decode_path)
            acis_record_map = _acis_candidate_record_map(decode_path)
            for row in raw.decode_body_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                acis_handles = _normalize_int_handles(row[1] if len(row) >= 2 else [])
                dxf = _attach_entity_color(
                    handle,
                    {
                        "acis_handles": acis_handles,
                    },
                    entity_style_map,
                    layer_color_map,
                    layer_color_overrides,
                    dxftype="BODY",
                )
                layer_handle = dxf.get("layer_handle")
                if isinstance(layer_handle, int):
                    dxf["acis_handles"] = [h for h in acis_handles if h != layer_handle]
                candidate_handles = list(acis_candidate_map.get(handle, ()))
                dxf.update(_build_acis_entity_payload(handle, candidate_handles, acis_record_map))
                yield Entity(
                    dxftype="BODY",
                    handle=handle,
                    dxf=dxf,
                )
            return

        if dxftype == "VIEWPORT":
            for row in raw.decode_viewport_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                yield Entity(
                    dxftype="VIEWPORT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {},
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="VIEWPORT",
                    ),
                )
            return

        if dxftype == "OLEFRAME":
            for row in raw.decode_oleframe_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                yield Entity(
                    dxftype="OLEFRAME",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {},
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="OLEFRAME",
                    ),
                )
            return

        if dxftype == "OLE2FRAME":
            for row in raw.decode_ole2frame_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                yield Entity(
                    dxftype="OLE2FRAME",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {},
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="OLE2FRAME",
                    ),
                )
            return

        if dxftype == "REGION":
            acis_candidate_map = _acis_candidate_handles_map(decode_path)
            acis_record_map = _acis_candidate_record_map(decode_path)
            for row in raw.decode_region_entities(decode_path):
                if not row:
                    continue
                handle = int(row[0])
                acis_handles = _normalize_int_handles(row[1] if len(row) >= 2 else [])
                dxf = _attach_entity_color(
                    handle,
                    {
                        "acis_handles": acis_handles,
                    },
                    entity_style_map,
                    layer_color_map,
                    layer_color_overrides,
                    dxftype="REGION",
                )
                layer_handle = dxf.get("layer_handle")
                if isinstance(layer_handle, int):
                    dxf["acis_handles"] = [h for h in acis_handles if h != layer_handle]
                candidate_handles = list(acis_candidate_map.get(handle, ()))
                dxf.update(_build_acis_entity_payload(handle, candidate_handles, acis_record_map))
                yield Entity(
                    dxftype="REGION",
                    handle=handle,
                    dxf=dxf,
                )
            return

        if dxftype == "RAY":
            for handle, start, unit_vector in raw.decode_ray_entities(decode_path):
                yield Entity(
                    dxftype="RAY",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "start": start,
                            "unit_vector": unit_vector,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="RAY",
                    ),
                )
            return

        if dxftype == "XLINE":
            for handle, start, unit_vector in raw.decode_xline_entities(decode_path):
                yield Entity(
                    dxftype="XLINE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "start": start,
                            "unit_vector": unit_vector,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="XLINE",
                    ),
                )
            return

        if dxftype == "POINT":
            for handle, x, y, z, angle in raw.decode_point_entities(decode_path):
                yield Entity(
                    dxftype="POINT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "location": (x, y, z),
                            "x_axis_angle": angle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POINT",
                    ),
                )
            return

        if dxftype == "CIRCLE":
            if bulk_rows is not None:
                circle_rows = bulk_rows[2]
            else:
                circle_rows = list(raw.decode_circle_entities(decode_path))
            circle_supplementary_handles = _circle_supplementary_handles(
                circle_rows, entity_style_map, layer_color_overrides
            )
            for handle, cx, cy, cz, radius in circle_rows:
                dxf = _attach_entity_color(
                    handle,
                    {
                        "center": (cx, cy, cz),
                        "radius": radius,
                    },
                    entity_style_map,
                    layer_color_map,
                    layer_color_overrides,
                    dxftype="CIRCLE",
                )
                if handle in circle_supplementary_handles:
                    dxf["resolved_color_index"] = 9
                    dxf["resolved_true_color"] = None
                yield Entity(
                    dxftype="CIRCLE",
                    handle=handle,
                    dxf=dxf,
                )
            return

        if dxftype == "ELLIPSE":
            for (
                handle,
                center,
                major_axis,
                extrusion,
                axis_ratio,
                start_angle,
                end_angle,
            ) in raw.decode_ellipse_entities(decode_path):
                yield Entity(
                    dxftype="ELLIPSE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "center": center,
                            "major_axis": major_axis,
                            "extrusion": extrusion,
                            "axis_ratio": axis_ratio,
                            "start_angle": start_angle,
                            "end_angle": end_angle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="ELLIPSE",
                    ),
                )
            return

        if dxftype == "SPLINE":
            for (
                handle,
                flags_data,
                tolerance_data,
                knots,
                control_points,
                weights,
                fit_points,
            ) in raw.decode_spline_entities(decode_path):
                scenario, degree, rational, closed, periodic = flags_data
                fit_tolerance, knot_tolerance, ctrl_tolerance = tolerance_data
                points = list(fit_points if len(fit_points) >= 2 else control_points)
                if closed and len(points) > 1 and points[0] != points[-1]:
                    points.append(points[0])
                yield Entity(
                    dxftype="SPLINE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "scenario": scenario,
                            "degree": degree,
                            "rational": bool(rational),
                            "closed": bool(closed),
                            "periodic": bool(periodic),
                            "fit_tolerance": fit_tolerance,
                            "knot_tolerance": knot_tolerance,
                            "ctrl_tolerance": ctrl_tolerance,
                            "knots": list(knots),
                            "control_points": list(control_points),
                            "weights": list(weights),
                            "fit_points": list(fit_points),
                            "points": points,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="SPLINE",
                    ),
                )
            return

        if dxftype == "TEXT":
            for (
                handle,
                text,
                insertion,
                alignment,
                extrusion,
                metrics,
                align_flags,
                style_handle,
            ) in raw.decode_text_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
                yield Entity(
                    dxftype="TEXT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "text": text,
                            "insert": insertion,
                            "align_point": alignment,
                            "extrusion": extrusion,
                            "thickness": thickness,
                            "oblique": math.degrees(oblique_angle),
                            "height": height,
                            "rotation": math.degrees(rotation),
                            "width": width_factor,
                            "text_generation_flag": generation,
                            "halign": horizontal_alignment,
                            "valign": vertical_alignment,
                            "style_handle": style_handle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="TEXT",
                    ),
                )
            return

        if dxftype == "ATTRIB":
            for (
                handle,
                text,
                tag,
                prompt,
                insertion,
                alignment,
                extrusion,
                metrics,
                align_flags,
                attrib_flags,
                lock_position,
                style_owner_handles,
            ) in raw.decode_attrib_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
                style_handle = None
                owner_handle = None
                if isinstance(style_owner_handles, tuple):
                    if len(style_owner_handles) >= 1:
                        style_handle = style_owner_handles[0]
                    if len(style_owner_handles) >= 2:
                        owner_handle = style_owner_handles[1]
                owner_handle_value = None
                if owner_handle is not None:
                    try:
                        owner_handle_value = int(owner_handle)
                    except Exception:
                        owner_handle_value = None
                yield Entity(
                    dxftype="ATTRIB",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "text": text,
                            "tag": tag,
                            "prompt": prompt,
                            "insert": insertion,
                            "align_point": alignment,
                            "extrusion": extrusion,
                            "thickness": thickness,
                            "oblique": math.degrees(oblique_angle),
                            "height": height,
                            "rotation": math.degrees(rotation),
                            "width": width_factor,
                            "text_generation_flag": generation,
                            "halign": horizontal_alignment,
                            "valign": vertical_alignment,
                            "style_handle": style_handle,
                            "attribute_flags": int(attrib_flags),
                            "lock_position": bool(lock_position),
                            "owner_handle": owner_handle_value,
                            "owner_type": "INSERT" if owner_handle_value is not None else None,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="ATTRIB",
                    ),
                )
            return

        if dxftype == "ATTDEF":
            for (
                handle,
                text,
                tag,
                prompt,
                insertion,
                alignment,
                extrusion,
                metrics,
                align_flags,
                attrib_flags,
                lock_position,
                style_owner_handles,
            ) in raw.decode_attdef_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
                style_handle = None
                owner_handle = None
                if isinstance(style_owner_handles, tuple):
                    if len(style_owner_handles) >= 1:
                        style_handle = style_owner_handles[0]
                    if len(style_owner_handles) >= 2:
                        owner_handle = style_owner_handles[1]
                owner_handle_value = None
                if owner_handle is not None:
                    try:
                        owner_handle_value = int(owner_handle)
                    except Exception:
                        owner_handle_value = None
                yield Entity(
                    dxftype="ATTDEF",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "text": text,
                            "tag": tag,
                            "prompt": prompt,
                            "insert": insertion,
                            "align_point": alignment,
                            "extrusion": extrusion,
                            "thickness": thickness,
                            "oblique": math.degrees(oblique_angle),
                            "height": height,
                            "rotation": math.degrees(rotation),
                            "width": width_factor,
                            "text_generation_flag": generation,
                            "halign": horizontal_alignment,
                            "valign": vertical_alignment,
                            "style_handle": style_handle,
                            "attribute_flags": int(attrib_flags),
                            "lock_position": bool(lock_position),
                            "owner_handle": owner_handle_value,
                            "owner_type": "BLOCK" if owner_handle_value is not None else None,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="ATTDEF",
                    ),
                )
            return

        if dxftype == "MTEXT":
            for (
                handle,
                text,
                insertion,
                extrusion,
                x_axis_dir,
                rect_width,
                text_height,
                attachment,
                drawing_dir,
                background_data,
            ) in raw.decode_mtext_entities(decode_path):
                (
                    background_flags,
                    background_scale_factor,
                    background_color_index,
                    background_true_color,
                    background_transparency,
                ) = background_data
                rotation = math.degrees(math.atan2(x_axis_dir[1], x_axis_dir[0]))
                plain_text = _decode_mtext_plain_text(text)
                yield Entity(
                    dxftype="MTEXT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "text": plain_text,
                            "raw_text": text,
                            "insert": insertion,
                            "extrusion": extrusion,
                            "text_direction": x_axis_dir,
                            "rotation": rotation,
                            "rect_width": rect_width,
                            "char_height": text_height,
                            "attachment_point": attachment,
                            "drawing_direction": drawing_dir,
                            "background_flags": background_flags,
                            "background_scale_factor": background_scale_factor,
                            "background_color_index": background_color_index,
                            "background_true_color": background_true_color,
                            "background_transparency": background_transparency,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="MTEXT",
                    ),
                )
            return

        if dxftype == "LEADER":
            for handle, annotation_type, path_type, points in raw.decode_leader_entities(
                decode_path
            ):
                points_list = list(points)
                yield Entity(
                    dxftype="LEADER",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "annotation_type": int(annotation_type),
                            "path_type": int(path_type),
                            "points": points_list,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="LEADER",
                    ),
                )
            return

        if dxftype == "HATCH":
            for (
                handle,
                name,
                solid_fill,
                associative,
                elevation,
                extrusion,
                path_rows,
            ) in raw.decode_hatch_entities(decode_path):
                paths = []
                for closed, points in path_rows:
                    path_points = [(x, y, elevation) for x, y in points]
                    if bool(closed) and len(path_points) > 1 and path_points[0] != path_points[-1]:
                        path_points.append(path_points[0])
                    paths.append(
                        {
                            "closed": bool(closed),
                            "points": path_points,
                        }
                    )
                yield Entity(
                    dxftype="HATCH",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "pattern_name": name,
                            "solid_fill": bool(solid_fill),
                            "associative": bool(associative),
                            "elevation": elevation,
                            "extrusion": extrusion,
                            "paths": paths,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="HATCH",
                    ),
                )
            return

        if dxftype == "TOLERANCE":
            for (
                handle,
                text,
                insertion,
                x_direction,
                extrusion,
                height,
                dimgap,
                dimstyle_handle,
            ) in raw.decode_tolerance_entities(decode_path):
                rotation = math.degrees(math.atan2(x_direction[1], x_direction[0]))
                yield Entity(
                    dxftype="TOLERANCE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "text": text,
                            "insert": insertion,
                            "x_direction": x_direction,
                            "extrusion": extrusion,
                            "height": height,
                            "dimgap": dimgap,
                            "rotation": rotation,
                            "dimstyle_handle": dimstyle_handle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="TOLERANCE",
                    ),
                )
            return

        if dxftype == "MLINE":
            for (
                handle,
                scale,
                justification,
                base_point,
                extrusion,
                open_closed,
                lines_in_style,
                vertices,
                mlinestyle_handle,
            ) in raw.decode_mline_entities(decode_path):
                vertices_list = list(vertices)
                points = [vertex[0] for vertex in vertices_list if len(vertex) >= 1]
                vertex_directions = [vertex[1] for vertex in vertices_list if len(vertex) >= 2]
                miter_directions = [vertex[2] for vertex in vertices_list if len(vertex) >= 3]
                flags = int(open_closed)
                closed = flags == 3 or bool(flags & 0x02)
                yield Entity(
                    dxftype="MLINE",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "scale": scale,
                            "justification": int(justification),
                            "base_point": base_point,
                            "extrusion": extrusion,
                            "flags": flags,
                            "closed": closed,
                            "line_count": int(lines_in_style),
                            "points": points,
                            "vertex_directions": vertex_directions,
                            "miter_directions": miter_directions,
                            "mlinestyle_handle": mlinestyle_handle,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="MLINE",
                    ),
                )
            return

        if dxftype == "MINSERT":
            for row in raw.decode_minsert_entities(decode_path):
                if len(row) == 9 and isinstance(row[8], tuple):
                    (
                        handle,
                        px,
                        py,
                        pz,
                        sx,
                        sy,
                        sz,
                        rotation,
                        array_info,
                    ) = row
                    (
                        num_columns,
                        num_rows,
                        column_spacing,
                        row_spacing,
                        name,
                    ) = array_info
                elif len(row) == 12:
                    (
                        handle,
                        px,
                        py,
                        pz,
                        sx,
                        sy,
                        sz,
                        rotation,
                        num_columns,
                        num_rows,
                        column_spacing,
                        row_spacing,
                    ) = row
                    name = None
                else:
                    # Backward-compatible shape from older extension builds.
                    (
                        handle,
                        px,
                        py,
                        pz,
                        sx,
                        sy,
                        sz,
                        rotation,
                        num_columns,
                        num_rows,
                        column_spacing,
                        row_spacing,
                        name,
                    ) = row
                dxf = {
                    "insert": (px, py, pz),
                    "xscale": sx,
                    "yscale": sy,
                    "zscale": sz,
                    "rotation": math.degrees(rotation),
                    "column_count": num_columns,
                    "row_count": num_rows,
                    "column_spacing": column_spacing,
                    "row_spacing": row_spacing,
                }
                if isinstance(name, str) and name:
                    dxf["name"] = name
                yield Entity(
                    dxftype="MINSERT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        dxf,
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="MINSERT",
                    ),
                )
            return

        if dxftype == "BLOCK":
            block_name_map, _ = _block_and_endblk_name_maps(decode_path)
            for handle in _entity_handles_by_type_name(decode_path, "BLOCK"):
                yield Entity(
                    dxftype="BLOCK",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "name": block_name_map.get(handle),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="BLOCK",
                    ),
                )
            return

        if dxftype == "ENDBLK":
            _, endblk_name_map = _block_and_endblk_name_maps(decode_path)
            for handle in _entity_handles_by_type_name(decode_path, "ENDBLK"):
                yield Entity(
                    dxftype="ENDBLK",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "name": endblk_name_map.get(handle),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="ENDBLK",
                    ),
                )
            return

        if dxftype == "INSERT":
            for row in raw.decode_insert_entities(decode_path):
                if len(row) == 8:
                    handle, px, py, pz, sx, sy, sz, rotation = row
                    name = None
                else:
                    handle, px, py, pz, sx, sy, sz, rotation, name = row
                dxf = {
                    "insert": (px, py, pz),
                    "xscale": sx,
                    "yscale": sy,
                    "zscale": sz,
                    "rotation": math.degrees(rotation),
                }
                if isinstance(name, str) and name:
                    dxf["name"] = name
                yield Entity(
                    dxftype="INSERT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        dxf,
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="INSERT",
                    ),
                )
            return

        if dxftype == "DIMENSION":
            dimension_rows: list[tuple[str, tuple]] = []
            used_bulk_decoder = False

            try:
                for dimtype, row in raw.decode_dimension_entities(decode_path):
                    dimension_rows.append((str(dimtype).upper(), row))
                used_bulk_decoder = True
            except Exception:
                used_bulk_decoder = False

            def _append_rows(dimtype: str, decode_fn) -> None:
                try:
                    rows = decode_fn(decode_path)
                except Exception:
                    rows = []
                for row in rows:
                    dimension_rows.append((dimtype, row))

            if not used_bulk_decoder:
                _append_rows("LINEAR", raw.decode_dim_linear_entities)
                _append_rows("ORDINATE", raw.decode_dim_ordinate_entities)
                _append_rows("ALIGNED", raw.decode_dim_aligned_entities)
                _append_rows("ANG3PT", raw.decode_dim_ang3pt_entities)
                _append_rows("ANG2LN", raw.decode_dim_ang2ln_entities)
                _append_rows("RADIUS", raw.decode_dim_radius_entities)
                _append_rows("DIAMETER", raw.decode_dim_diameter_entities)

            dimension_rows.sort(key=lambda item: item[1][0])
            for dimtype, row in dimension_rows:
                (
                    handle,
                    user_text,
                    point10,
                    point13,
                    point14,
                    text_midpoint,
                    insert_point,
                    transforms,
                    angles,
                    common_data,
                    handle_data,
                ) = row
                extrusion, insert_scale = transforms
                text_rotation, horizontal_direction, ext_line_rotation, dim_rotation = angles
                (
                    dim_flags,
                    actual_measurement,
                    attachment_point,
                    line_spacing_style,
                    line_spacing_factor,
                    insert_rotation,
                ) = common_data
                dimstyle_handle, anonymous_block_handle = handle_data
                common_dxf = _build_dimension_common_dxf(
                    user_text=user_text,
                    text_midpoint=text_midpoint,
                    insert_point=insert_point,
                    extrusion=extrusion,
                    insert_scale=insert_scale,
                    text_rotation=text_rotation,
                    horizontal_direction=horizontal_direction,
                    dim_flags=dim_flags,
                    actual_measurement=actual_measurement,
                    attachment_point=attachment_point,
                    line_spacing_style=line_spacing_style,
                    line_spacing_factor=line_spacing_factor,
                    insert_rotation=insert_rotation,
                    dimstyle_handle=dimstyle_handle,
                    anonymous_block_handle=anonymous_block_handle,
                )
                dim_dxf = {
                    "dimtype": dimtype,
                    "defpoint": point10,
                    "defpoint2": point13,
                    "defpoint3": point14,
                    "oblique_angle": math.degrees(ext_line_rotation),
                    "angle": math.degrees(dim_rotation),
                }
                dim_dxf.update(common_dxf)
                dim_dxf["common"] = dict(common_dxf)
                yield Entity(
                    dxftype="DIMENSION",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        dim_dxf,
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="DIMENSION",
                    ),
                )
            return

        raise ValueError(
            f"unsupported entity type: {dxftype}. "
            "Supported types: LINE, LWPOLYLINE, POLYLINE_2D, VERTEX_2D, POLYLINE_3D, VERTEX_3D, POLYLINE_MESH, VERTEX_MESH, POLYLINE_PFACE, VERTEX_PFACE, VERTEX_PFACE_FACE, SEQEND, 3DFACE, SOLID, TRACE, SHAPE, 3DSOLID, BODY, VIEWPORT, OLEFRAME, OLE2FRAME, REGION, RAY, XLINE, ARC, CIRCLE, ELLIPSE, SPLINE, POINT, TEXT, ATTRIB, ATTDEF, MTEXT, LEADER, HATCH, TOLERANCE, MLINE, BLOCK, ENDBLK, INSERT, MINSERT, DIMENSION"
        )


def _decode_mtext_plain_text(value: str) -> str:
    if not value:
        return ""

    out: list[str] = []
    i = 0
    n = len(value)
    while i < n:
        ch = value[i]

        if ch in "{}":
            i += 1
            continue
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        if i + 1 >= n:
            out.append("\\")
            break

        code = value[i + 1]
        if code in "\\{}":
            out.append(code)
            i += 2
            continue
        if code in {"P", "X"}:
            out.append("\n")
            i += 2
            continue
        if code == "~":
            out.append(" ")
            i += 2
            continue
        if code in {"L", "l", "O", "o", "K", "k"}:
            i += 2
            continue
        if code in {"U", "u"} and i + 6 < n and value[i + 2] == "+":
            hex_digits = value[i + 3 : i + 7]
            if all(c in "0123456789abcdefABCDEF" for c in hex_digits):
                out.append(chr(int(hex_digits, 16)))
                i += 7
                continue
        if code == "S":
            i += 2
            stacked: list[str] = []
            while i < n and value[i] != ";":
                token = value[i]
                if token in {"#", "^"}:
                    token = "/"
                stacked.append(token)
                i += 1
            if i < n and value[i] == ";":
                i += 1
            out.append("".join(stacked))
            continue
        if code in {"A", "C", "c", "F", "f", "H", "h", "Q", "q", "T", "t", "W", "w", "p"}:
            i += 2
            while i < n and value[i] != ";":
                i += 1
            if i < n and value[i] == ";":
                i += 1
            continue

        out.append(code)
        i += 2

    return "".join(out)


def _strip_duplicate_closure_point(
    points: list[tuple[float, float, float]],
) -> list[tuple[float, float, float]]:
    if len(points) > 1 and points[0] == points[-1]:
        return points[:-1]
    return points


def _polyline_2d_flags_info(flags: int) -> dict[str, bool]:
    value = int(flags)
    return {
        "closed": bool(value & 0x01),
        "curve_fit": bool(value & 0x02),
        "spline_fit": bool(value & 0x04),
        "is_3d_polyline": bool(value & 0x08),
        "is_3d_mesh": bool(value & 0x10),
        "is_closed_mesh": bool(value & 0x20),
        "is_polyface_mesh": bool(value & 0x40),
        "continuous_linetype": bool(value & 0x80),
    }


def _polyline_2d_should_interpolate(
    curve_fit: bool,
    spline_fit: bool,
    curve_type_label: str | None,
) -> bool:
    if curve_fit or spline_fit:
        return True
    if curve_type_label in _POLYLINE_2D_SPLINE_CURVE_TYPES:
        return True
    return False


def _polyline_2d_interpreted_map(path: str) -> dict[int, dict[str, object]]:
    try:
        rows = raw.decode_polyline_2d_entities_interpreted(path)
    except Exception:
        return {}

    interpreted: dict[int, dict[str, object]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 12:
            continue
        (
            handle,
            _flags,
            curve_type,
            curve_type_label,
            closed,
            curve_fit,
            spline_fit,
            is_3d_polyline,
            is_3d_mesh,
            is_closed_mesh,
            is_polyface_mesh,
            continuous_linetype,
        ) = row
        interpreted[int(handle)] = {
            "curve_type": int(curve_type),
            "curve_type_label": str(curve_type_label),
            "closed": bool(closed),
            "curve_fit": bool(curve_fit),
            "spline_fit": bool(spline_fit),
            "is_3d_polyline": bool(is_3d_polyline),
            "is_3d_mesh": bool(is_3d_mesh),
            "is_closed_mesh": bool(is_closed_mesh),
            "is_polyface_mesh": bool(is_polyface_mesh),
            "continuous_linetype": bool(continuous_linetype),
        }
    return interpreted


def _polyline_2d_interpolated_points_map(
    path: str,
    interpreted_map: dict[int, dict[str, object]],
) -> dict[int, list[tuple[float, float, float]]]:
    interpolation_handles = {
        handle
        for handle, info in interpreted_map.items()
        if _polyline_2d_should_interpolate(
            bool(info.get("curve_fit", False)),
            bool(info.get("spline_fit", False)),
            str(info.get("curve_type_label", "")),
        )
    }
    if not interpolation_handles:
        return {}

    try:
        rows = raw.decode_polyline_2d_with_vertices_interpolated(
            path, _POLYLINE_2D_INTERPOLATION_SEGMENTS
        )
    except Exception:
        return {}

    interpolated: dict[int, list[tuple[float, float, float]]] = {}
    for handle, flags, applied, points in rows:
        handle_key = int(handle)
        if handle_key not in interpolation_handles or not bool(applied):
            continue
        closed = bool(
            interpreted_map.get(handle_key, {}).get("closed", bool(int(flags) & 0x01))
        )
        points3d = list(points)
        if closed:
            points3d = _strip_duplicate_closure_point(points3d)
        interpolated[handle_key] = points3d
    return interpolated


def _build_dimension_common_dxf(
    *,
    user_text: str,
    text_midpoint: tuple[float, float, float],
    insert_point: tuple[float, float, float] | None,
    extrusion: tuple[float, float, float],
    insert_scale: tuple[float, float, float],
    text_rotation: float,
    horizontal_direction: float,
    dim_flags: int,
    actual_measurement: float | None,
    attachment_point: int | None,
    line_spacing_style: int | None,
    line_spacing_factor: float | None,
    insert_rotation: float,
    dimstyle_handle: int | None,
    anonymous_block_handle: int | None,
) -> dict:
    return {
        "text_midpoint": text_midpoint,
        "insert": insert_point,
        "extrusion": extrusion,
        "insert_scale": insert_scale,
        "text": user_text,
        "text_rotation": math.degrees(text_rotation),
        "horizontal_direction": math.degrees(horizontal_direction),
        "dim_flags": dim_flags,
        "actual_measurement": actual_measurement,
        "attachment_point": attachment_point,
        "line_spacing_style": line_spacing_style,
        "line_spacing_factor": line_spacing_factor,
        "insert_rotation": math.degrees(insert_rotation),
        "dimstyle_handle": dimstyle_handle,
        "anonymous_block_handle": anonymous_block_handle,
    }


@lru_cache(maxsize=32)
def _present_supported_types(
    path: str | None, *, include_explicit_only: bool = False
) -> tuple[str, ...]:
    if not path:
        return tuple(SUPPORTED_ENTITY_TYPES)
    try:
        headers = raw.list_object_headers_with_type(path)
    except Exception:
        return tuple(SUPPORTED_ENTITY_TYPES)

    seen: set[str] = set()
    for row in headers:
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        canonical = _canonical_entity_type_name(row[4])
        if canonical is not None:
            seen.add(canonical)

    if not seen:
        present = tuple(SUPPORTED_ENTITY_TYPES)
    else:
        present = tuple(dxftype for dxftype in SUPPORTED_ENTITY_TYPES if dxftype in seen)

    if include_explicit_only:
        return present
    return tuple(dxftype for dxftype in present if dxftype not in _EXPLICIT_ONLY_ENTITY_TYPES)


def _canonical_entity_type_name(raw_name: object) -> str | None:
    name = str(raw_name).strip().upper()
    if not name:
        return None
    if name.startswith("DIM_"):
        return "DIMENSION"
    canonical = TYPE_ALIASES.get(name, name)
    if canonical in SUPPORTED_ENTITY_TYPES:
        return canonical
    return None


def _normalize_types(types: str | Iterable[str] | None, path: str | None = None) -> list[str]:
    default_types = list(_present_supported_types(path))
    candidate_types = (
        list(_present_supported_types(path, include_explicit_only=True))
        if path is not None
        else list(SUPPORTED_ENTITY_TYPES)
    )
    if types is None:
        return default_types
    if isinstance(types, str):
        tokens = re.split(r"[,\s]+", types.strip())
    else:
        tokens = list(types)

    normalized = [token.strip().upper() for token in tokens if token and token.strip()]
    normalized = [TYPE_ALIASES.get(token, token) for token in normalized]
    if not normalized:
        return default_types

    if any(token in {"*", "ALL"} for token in normalized):
        return default_types

    selected: list[str] = []
    seen = set()

    for token in normalized:
        if any(ch in token for ch in "*?[]"):
            matches = [
                name for name in candidate_types if fnmatch.fnmatchcase(name, token)
            ]
            if not matches:
                continue
            for name in matches:
                if name not in seen:
                    seen.add(name)
                    selected.append(name)
            continue

        if token in SUPPORTED_ENTITY_TYPES:
            if token not in seen:
                seen.add(token)
                selected.append(token)

    return selected


@lru_cache(maxsize=16)
def _entity_handles_by_type_name(path: str, type_name: str) -> tuple[int, ...]:
    target = str(type_name).strip().upper()
    if not target:
        return ()
    try:
        rows = raw.list_object_headers_with_type(path)
    except Exception:
        return ()
    handles: list[int] = []
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        name = str(row[4]).strip().upper()
        if name != target:
            continue
        try:
            handles.append(int(row[0]))
        except Exception:
            continue
    return tuple(handles)


@lru_cache(maxsize=16)
def _object_headers_with_type_map(path: str) -> dict[int, tuple[int, int, int, str, str]]:
    try:
        rows = raw.list_object_headers_with_type(path)
    except Exception:
        return {}

    out: dict[int, tuple[int, int, int, str, str]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        try:
            handle = int(row[0])
            offset = int(row[1])
            data_size = int(row[2])
            type_code = int(row[3])
        except Exception:
            continue
        type_name = str(row[4]).strip().upper()
        type_class = str(row[5]).strip().upper()
        out[handle] = (offset, data_size, type_code, type_name, type_class)
    return out


def _extract_ascii_preview(raw_bytes: bytes) -> str | None:
    if not raw_bytes:
        return None
    best: str | None = None
    for match in re.finditer(rb"[ -~]{6,}", raw_bytes):
        text = match.group(0).decode("ascii", errors="ignore").strip()
        if not text:
            continue
        if best is None or len(text) > len(best):
            best = text
    if best is None:
        return None
    if len(best) > 96:
        return f"{best[:93]}..."
    return best


def _extract_likely_handle_refs(
    raw_bytes: bytes,
    known_handles: set[int],
    *,
    max_count: int = 8,
    min_handle: int = 64,
) -> list[int]:
    if not raw_bytes or not known_handles:
        return []
    found: list[int] = []
    seen: set[int] = set()

    def _push(value: int) -> None:
        if value < min_handle or value not in known_handles or value in seen:
            return
        seen.add(value)
        found.append(value)

    scan_len = min(len(raw_bytes), 512)
    for offset in range(0, max(0, scan_len - 4 + 1)):
        value = int.from_bytes(raw_bytes[offset : offset + 4], byteorder="little", signed=False)
        _push(value)
        if len(found) >= max_count:
            return found
    for offset in range(0, max(0, scan_len - 8 + 1)):
        value = int.from_bytes(raw_bytes[offset : offset + 8], byteorder="little", signed=False)
        _push(value)
        if len(found) >= max_count:
            return found
    return found


def _acis_role_hint(
    type_code: int,
    record_size: int | None,
    ascii_preview: str | None,
) -> str:
    hint = _ACIS_ROLE_HINTS.get(type_code)
    if hint is None:
        if 0x214 <= type_code <= 0x225:
            return "acis-aux"
        return "unknown"
    if hint == "acis-header" and ascii_preview:
        return "acis-text-header"
    if hint == "acis-payload-chunk" and isinstance(record_size, int) and record_size >= 128:
        return "acis-payload-main"
    return hint


def _handle_ref_details(
    handles: list[int],
    header_map: dict[int, tuple[int, int, int, str, str]],
) -> list[dict[str, object]]:
    details: list[dict[str, object]] = []
    for handle in handles:
        offset, data_size, type_code, type_name, type_class = header_map.get(
            handle,
            (0, 0, 0, "UNKNOWN", ""),
        )
        details.append(
            {
                "handle": handle,
                "offset": offset,
                "data_size": data_size,
                "type_code": type_code,
                "type_name": type_name,
                "type_class": type_class,
            }
        )
    return details


def _normalize_int_handles(values: object) -> list[int]:
    out: list[int] = []
    seen: set[int] = set()
    for item in list(values or []):
        try:
            value = int(item)
        except Exception:
            continue
        if value <= 0 or value in seen:
            continue
        seen.add(value)
        out.append(value)
    return out


def _merge_handle_lists(primary: object, secondary: object) -> list[int]:
    out: list[int] = []
    seen: set[int] = set()
    for values in (primary, secondary):
        for item in list(values or []):
            try:
                value = int(item)
            except Exception:
                continue
            if value <= 0 or value in seen:
                continue
            seen.add(value)
            out.append(value)
    return out


def _select_acis_parent_ref_handles(
    stream_refs: list[int],
    scanned_refs: list[int],
    *,
    confidence: int,
) -> tuple[list[int], str]:
    if not stream_refs:
        return list(scanned_refs), "scan-only"

    scanned_set = set(scanned_refs)
    overlap_refs = [ref for ref in stream_refs if ref in scanned_set]

    if confidence >= 60:
        return list(stream_refs), "stream"

    if overlap_refs:
        if confidence >= 40:
            return list(overlap_refs), "stream-overlap"
        return list(overlap_refs), "stream-scan-overlap"

    if confidence >= 40:
        return list(stream_refs), "stream-mid"

    # Low-confidence decode with no corroborating overlap: keep only explicit
    # overlap and drop inferred references to avoid noisy edges.
    return [], "lowconf-drop"


def _build_acis_entity_payload(
    entity_handle: int,
    candidate_handles: list[int],
    acis_record_map: dict[int, dict[str, object]],
) -> dict[str, object]:
    candidate_handles = _normalize_int_handles(candidate_handles)
    candidate_index = {handle: index for index, handle in enumerate(candidate_handles)}
    candidate_set = set(candidate_handles)

    records: list[dict[str, object]] = []
    for index, handle in enumerate(candidate_handles):
        record = dict(acis_record_map.get(handle, {"handle": handle}))
        refs_source = record.get("acis_parent_ref_handles")
        refs = _normalize_int_handles(
            refs_source if refs_source is not None else record.get("likely_handle_refs")
        )
        parent_ref_strategy = str(record.get("acis_parent_ref_strategy") or "")
        if not refs and parent_ref_strategy == "lowconf-drop":
            # Entity-level safety net: if the merged candidates still include only
            # the current entity owner, recover that edge while keeping noisy refs dropped.
            likely_refs = _normalize_int_handles(record.get("likely_handle_refs"))
            if entity_handle in likely_refs:
                refs = [entity_handle]
                parent_ref_strategy = "lowconf-entity-fallback"
        record["acis_parent_ref_strategy_effective"] = parent_ref_strategy
        entity_refs = [ref for ref in refs if ref == entity_handle]
        candidate_refs = [ref for ref in refs if ref in candidate_set and ref != handle]
        external_refs = [ref for ref in refs if ref not in candidate_set and ref != entity_handle]

        record["acis_candidate_index"] = index
        record["entity_ref_handles"] = entity_refs
        record["candidate_ref_handles"] = candidate_refs
        record["external_ref_handles"] = external_refs
        records.append(record)

    role_by_handle: dict[int, str] = {}
    for record in records:
        try:
            handle = int(record.get("handle", 0))
        except Exception:
            continue
        role_by_handle[handle] = str(record.get("acis_role_hint") or "unknown")

    def _select_candidate_parent(
        current_index: int,
        candidate_refs: list[int],
        *,
        allowed_roles: set[str] | None = None,
    ) -> int | None:
        filtered = [
            ref
            for ref in candidate_refs
            if allowed_roles is None or role_by_handle.get(ref, "unknown") in allowed_roles
        ]
        if not filtered:
            return None
        prev_filtered = [ref for ref in filtered if candidate_index.get(ref, -1) < current_index]
        if prev_filtered:
            return max(prev_filtered, key=lambda ref: candidate_index.get(ref, -1))
        return filtered[0]

    for record in records:
        role_hint = str(record.get("acis_role_hint") or "unknown")
        current_index = int(record.get("acis_candidate_index", 0))
        entity_refs = _normalize_int_handles(record.get("entity_ref_handles"))
        candidate_refs = _normalize_int_handles(record.get("candidate_ref_handles"))
        external_refs = _normalize_int_handles(record.get("external_ref_handles"))

        parent_handle: int | None = None
        parent_kind = "none"
        parent_rule = "none"

        if role_hint in {"acis-header", "acis-text-header"}:
            if entity_refs:
                parent_handle = entity_refs[0]
                parent_kind = "entity"
                parent_rule = "header-prefers-entity"
            else:
                candidate_parent = _select_candidate_parent(
                    current_index,
                    candidate_refs,
                    allowed_roles={"acis-header", "acis-text-header"},
                )
                if candidate_parent is not None:
                    parent_handle = candidate_parent
                    parent_kind = "candidate"
                    parent_rule = "header-fallback-candidate"
        elif role_hint == "acis-link-table":
            for allowed_roles, rule in (
                ({"acis-header", "acis-text-header"}, "link-prefers-header"),
                (None, "link-fallback-candidate"),
            ):
                candidate_parent = _select_candidate_parent(
                    current_index,
                    candidate_refs,
                    allowed_roles=allowed_roles,
                )
                if candidate_parent is not None:
                    parent_handle = candidate_parent
                    parent_kind = "candidate"
                    parent_rule = rule
                    break
            if parent_handle is None and entity_refs:
                parent_handle = entity_refs[0]
                parent_kind = "entity"
                parent_rule = "link-fallback-entity"
        elif role_hint in {"acis-payload-main", "acis-payload-chunk"}:
            for allowed_roles, rule in (
                ({"acis-link-table"}, "payload-prefers-link"),
                ({"acis-payload-main", "acis-payload-chunk"}, "payload-prefers-payload"),
                ({"acis-header", "acis-text-header"}, "payload-fallback-header"),
                (None, "payload-fallback-candidate"),
            ):
                candidate_parent = _select_candidate_parent(
                    current_index,
                    candidate_refs,
                    allowed_roles=allowed_roles,
                )
                if candidate_parent is not None:
                    parent_handle = candidate_parent
                    parent_kind = "candidate"
                    parent_rule = rule
                    break
            if parent_handle is None and entity_refs:
                parent_handle = entity_refs[0]
                parent_kind = "entity"
                parent_rule = "payload-fallback-entity"
        else:
            if entity_refs:
                parent_handle = entity_refs[0]
                parent_kind = "entity"
                parent_rule = "generic-entity"
            else:
                candidate_parent = _select_candidate_parent(current_index, candidate_refs)
                if candidate_parent is not None:
                    parent_handle = candidate_parent
                    parent_kind = "candidate"
                    parent_rule = "generic-candidate"

        if parent_handle is None and external_refs:
            parent_handle = external_refs[0]
            parent_kind = "external"
            parent_rule = "external-fallback"

        record["acis_parent_handle"] = parent_handle
        record["acis_parent_kind"] = parent_kind
        record["acis_parent_rule"] = parent_rule

    child_map: dict[int, list[int]] = {}
    edges: list[dict[str, object]] = []
    primary_edges: list[dict[str, object]] = []
    for record in records:
        source = int(record.get("handle", 0))
        for target in list(record.get("entity_ref_handles") or []):
            edges.append({"source": source, "target": int(target), "kind": "entity"})
        for target in list(record.get("candidate_ref_handles") or []):
            target = int(target)
            edges.append({"source": source, "target": target, "kind": "candidate"})
        for target in list(record.get("external_ref_handles") or []):
            edges.append({"source": source, "target": int(target), "kind": "external"})
        parent_handle = record.get("acis_parent_handle")
        parent_kind = str(record.get("acis_parent_kind") or "none")
        if isinstance(parent_handle, int) and parent_kind in {"entity", "candidate", "external"}:
            primary_edges.append(
                {
                    "source": source,
                    "target": parent_handle,
                    "kind": parent_kind,
                    "rule": str(record.get("acis_parent_rule") or "none"),
                }
            )
        if record.get("acis_parent_kind") == "candidate":
            parent = record.get("acis_parent_handle")
            if isinstance(parent, int):
                child_map.setdefault(parent, []).append(source)

    for record in records:
        source = int(record.get("handle", 0))
        record["acis_child_candidate_handles"] = child_map.get(source, [])

    return {
        "acis_candidate_handles": candidate_handles,
        "acis_candidate_records": records,
        "acis_candidate_edges": edges,
        "acis_primary_edges": primary_edges,
    }


@lru_cache(maxsize=16)
def _acis_candidate_handles_map(path: str) -> dict[int, tuple[int, ...]]:
    try:
        rows = raw.list_object_headers_with_type(path)
    except Exception:
        return {}

    normalized_rows: list[tuple[int, int, int, str, str]] = []
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        try:
            handle = int(row[0])
            offset = int(row[1])
            type_code = int(row[3])
        except Exception:
            continue
        type_name = str(row[4]).strip().upper()
        type_class = str(row[5]).strip().upper()
        normalized_rows.append((handle, offset, type_code, type_name, type_class))

    if not normalized_rows:
        return {}

    def _scan_candidates(
        sorted_rows: list[tuple[int, int, int, str, str]],
    ) -> dict[int, tuple[int, ...]]:
        target_types = {"3DSOLID", "BODY", "REGION"}
        out: dict[int, tuple[int, ...]] = {}
        for index, (handle, _offset, _type_code, type_name, type_class) in enumerate(sorted_rows):
            if type_class not in {"E", "ENTITY"} or type_name not in target_types:
                continue
            candidates: list[int] = []
            for (
                next_handle,
                _next_offset,
                next_type_code,
                next_type_name,
                next_type_class,
            ) in sorted_rows[index + 1 :]:
                if next_type_class in {"E", "ENTITY"}:
                    break
                if not next_type_name.startswith("UNKNOWN("):
                    continue
                # ACIS companion records for modeler entities are typically in the
                # dynamic UNKNOWN range 0x214-0x225 for our current sample corpus.
                if 0x214 <= next_type_code <= 0x225:
                    candidates.append(next_handle)
            out[handle] = tuple(candidates)
        return out

    by_offset = sorted(normalized_rows, key=lambda item: (item[1], item[0]))
    by_handle = sorted(normalized_rows, key=lambda item: (item[0], item[1]))
    offset_map = _scan_candidates(by_offset)
    handle_map = _scan_candidates(by_handle)

    out: dict[int, tuple[int, ...]] = {}
    for handle in set(offset_map.keys()) | set(handle_map.keys()):
        handle_candidates = handle_map.get(handle, ())
        offset_candidates = offset_map.get(handle, ())
        if handle_candidates:
            out[handle] = handle_candidates
        elif offset_candidates:
            out[handle] = offset_candidates
        else:
            out[handle] = ()

    return out


@lru_cache(maxsize=16)
def _acis_candidate_record_map(path: str) -> dict[int, dict[str, object]]:
    candidate_map = _acis_candidate_handles_map(path)
    candidate_handles = sorted(
        {
            handle
            for handles in candidate_map.values()
            for handle in handles
            if isinstance(handle, int) and handle > 0
        }
    )
    if not candidate_handles:
        return {}

    try:
        rows = raw.read_object_records_by_handle(path, candidate_handles)
    except Exception:
        rows = []
    try:
        acis_info_rows = raw.decode_acis_candidate_infos(path, candidate_handles)
    except Exception:
        acis_info_rows = []
    acis_info_map: dict[int, tuple[int, int, str, list[int], int]] = {}
    for row in acis_info_rows:
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        try:
            handle = int(row[0])
            type_code = int(row[1])
            data_size = int(row[2])
        except Exception:
            continue
        role_hint = str(row[3]).strip()
        refs = _normalize_int_handles(row[4])
        confidence = 0
        if len(row) >= 6:
            try:
                confidence = int(row[5])
            except Exception:
                confidence = 0
        confidence = max(0, min(100, confidence))
        acis_info_map[handle] = (type_code, data_size, role_hint, refs, confidence)

    header_map = _object_headers_with_type_map(path)
    known_handles = set(header_map.keys())
    out: dict[int, dict[str, object]] = {}
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        try:
            handle = int(row[0])
            offset = int(row[1])
            data_size = int(row[2])
            type_code = int(row[3])
        except Exception:
            continue
        record_bytes = bytes(row[4]) if row[4] is not None else b""
        _, _, _, type_name, _type_class = header_map.get(
            handle,
            (offset, data_size, type_code, f"UNKNOWN(0x{type_code:X})", ""),
        )
        info = acis_info_map.get(handle)
        if info is not None:
            info_type_code, info_data_size, info_role_hint, info_refs, info_confidence = info
            if info_type_code > 0:
                type_code = info_type_code
            if info_data_size > 0:
                data_size = info_data_size
            role_hint = info_role_hint
            ref_confidence = info_confidence
            stream_handle_refs = [
                ref for ref in info_refs if ref in known_handles and ref != handle
            ]
        else:
            role_hint = ""
            ref_confidence = 0
            stream_handle_refs = []
        scanned_refs = [
            ref
            for ref in _extract_likely_handle_refs(record_bytes, known_handles)
            if ref != handle
        ]
        if stream_handle_refs:
            if ref_confidence < 40:
                likely_handle_refs = _merge_handle_lists(stream_handle_refs, scanned_refs)
            else:
                likely_handle_refs = list(stream_handle_refs)
            parent_handle_refs, parent_ref_strategy = _select_acis_parent_ref_handles(
                stream_handle_refs,
                scanned_refs,
                confidence=ref_confidence,
            )
        else:
            likely_handle_refs = scanned_refs
            parent_handle_refs, parent_ref_strategy = _select_acis_parent_ref_handles(
                stream_handle_refs,
                scanned_refs,
                confidence=ref_confidence,
            )
        ascii_preview = _extract_ascii_preview(record_bytes)
        out[handle] = {
            "handle": handle,
            "offset": offset,
            "data_size": data_size,
            "type_code": type_code,
            "type_name": type_name,
            "record_size": len(record_bytes),
            "ascii_preview": ascii_preview,
            "likely_handle_refs": likely_handle_refs,
            "likely_handle_ref_details": _handle_ref_details(likely_handle_refs, header_map),
            "acis_stream_handle_refs": stream_handle_refs,
            "acis_scanned_handle_refs": scanned_refs,
            "acis_parent_ref_handles": parent_handle_refs,
            "acis_parent_ref_strategy": parent_ref_strategy,
            "acis_ref_confidence": ref_confidence,
            "acis_role_hint": role_hint
            if role_hint
            else _acis_role_hint(type_code, len(record_bytes), ascii_preview),
        }

    for handle in candidate_handles:
        if handle in out:
            continue
        offset, data_size, type_code, type_name, _type_class = header_map.get(
            handle,
            (0, 0, 0, "UNKNOWN", ""),
        )
        out[handle] = {
            "handle": handle,
            "offset": offset,
            "data_size": data_size,
            "type_code": type_code,
            "type_name": type_name,
            "record_size": None,
            "ascii_preview": None,
            "likely_handle_refs": [],
            "likely_handle_ref_details": [],
            "acis_stream_handle_refs": [],
            "acis_scanned_handle_refs": [],
            "acis_parent_ref_handles": [],
            "acis_parent_ref_strategy": "none",
            "acis_ref_confidence": 0,
            "acis_role_hint": (
                acis_info_map.get(handle, (type_code, data_size, "", [], 0))[2]
                or _acis_role_hint(type_code, None, None)
            ),
        }

    return out


@lru_cache(maxsize=16)
def _polyline_sequence_relationships(
    path: str,
) -> tuple[
    dict[int, dict[str, object]],
    dict[int, tuple[int, str]],
    dict[int, tuple[int, str]],
]:
    try:
        rows = raw.decode_polyline_sequence_members(path)
    except Exception:
        return {}, {}, {}

    polyline_map: dict[int, dict[str, object]] = {}
    vertex_owner_map: dict[int, tuple[int, str]] = {}
    seqend_owner_map: dict[int, tuple[int, str]] = {}

    for row in rows:
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        raw_handle, raw_type_name, raw_vertex_handles, raw_face_handles, raw_seqend_handle = row
        try:
            handle = int(raw_handle)
        except Exception:
            continue
        owner_type = str(raw_type_name).strip().upper()
        if owner_type == "":
            continue

        vertex_handles: list[int] = []
        for item in list(raw_vertex_handles or []):
            try:
                vertex_handles.append(int(item))
            except Exception:
                continue
        face_handles: list[int] = []
        for item in list(raw_face_handles or []):
            try:
                face_handles.append(int(item))
            except Exception:
                continue
        seqend_handle: int | None = None
        if raw_seqend_handle is not None:
            try:
                seqend_handle = int(raw_seqend_handle)
            except Exception:
                seqend_handle = None

        polyline_map[handle] = {
            "vertex_handles": tuple(vertex_handles),
            "face_handles": tuple(face_handles),
            "seqend_handle": seqend_handle,
        }
        for vertex_handle in vertex_handles:
            vertex_owner_map[vertex_handle] = (handle, owner_type)
        for face_handle in face_handles:
            vertex_owner_map[face_handle] = (handle, owner_type)
        if seqend_handle is not None:
            seqend_owner_map[seqend_handle] = (handle, owner_type)

    return polyline_map, vertex_owner_map, seqend_owner_map


@lru_cache(maxsize=16)
def _block_and_endblk_name_maps(path: str) -> tuple[dict[int, str], dict[int, str]]:
    block_handles = list(_entity_handles_by_type_name(path, "BLOCK"))
    endblk_handles = list(_entity_handles_by_type_name(path, "ENDBLK"))
    if not block_handles and not endblk_handles:
        return {}, {}

    # Preferred: Rust-side deterministic mapping from BLOCK_HEADER -> BLOCK/ENDBLK.
    try:
        entity_rows = raw.decode_block_entity_names(path)
    except Exception:
        entity_rows = []
    if entity_rows:
        block_map: dict[int, str] = {}
        endblk_map: dict[int, str] = {}
        for row in entity_rows:
            if not isinstance(row, tuple) or len(row) < 3:
                continue
            handle, type_name, name = row[0], row[1], row[2]
            if not isinstance(name, str) or not name:
                continue
            try:
                handle_key = int(handle)
            except Exception:
                continue
            type_token = str(type_name).strip().upper()
            if type_token == "BLOCK":
                block_map[handle_key] = name
            elif type_token == "ENDBLK":
                endblk_map[handle_key] = name
        if block_map or endblk_map:
            return block_map, endblk_map

    try:
        rows = raw.decode_block_header_names(path)
    except Exception:
        rows = []

    by_header_handle: dict[int, str] = {}
    ordered_names: list[str] = []
    for row in rows:
        if not isinstance(row, tuple) or len(row) < 2:
            continue
        header_handle, name = row[0], row[1]
        if not isinstance(name, str) or not name:
            continue
        ordered_names.append(name)
        try:
            by_header_handle[int(header_handle)] = name
        except Exception:
            pass

    block_map: dict[int, str] = {}
    endblk_map: dict[int, str] = {}

    # Prefer exact handle matches when available.
    for handle in block_handles:
        name = by_header_handle.get(handle)
        if name:
            block_map[handle] = name
    for handle in endblk_handles:
        name = by_header_handle.get(handle)
        if name:
            endblk_map[handle] = name

    # Fallback: assign names in declaration order to preserve deterministic output.
    remaining_names = iter(ordered_names)
    for handle in block_handles:
        if handle not in block_map:
            name = next(remaining_names, None)
            if isinstance(name, str):
                block_map[handle] = name
    for index, handle in enumerate(endblk_handles):
        if handle in endblk_map:
            continue
        if index < len(block_handles):
            paired_name = block_map.get(block_handles[index])
            if paired_name:
                endblk_map[handle] = paired_name
                continue
        name = next(remaining_names, None)
        if isinstance(name, str):
            endblk_map[handle] = name

    return block_map, endblk_map


@lru_cache(maxsize=16)
def _line_arc_circle_rows(
    path: str,
) -> tuple[
    list[tuple[int, float, float, float, float, float, float]],
    list[tuple[int, float, float, float, float, float, float]],
    list[tuple[int, float, float, float, float]],
]:
    try:
        line_rows, arc_rows, circle_rows = raw.decode_line_arc_circle_entities(path)
        return list(line_rows), list(arc_rows), list(circle_rows)
    except Exception:
        return (
            list(raw.decode_line_entities(path)),
            list(raw.decode_arc_entities(path)),
            list(raw.decode_circle_entities(path)),
        )


@lru_cache(maxsize=16)
def _entity_style_map(path: str) -> dict[int, tuple[int | None, int | None, int]]:
    try:
        return {
            handle: (index, true_color, layer_handle)
            for handle, index, true_color, layer_handle in raw.decode_entity_styles(path)
        }
    except Exception:
        return {}


@lru_cache(maxsize=16)
def _layer_color_map(path: str) -> dict[int, tuple[int, int | None]]:
    try:
        return {
            handle: (index, true_color)
            for handle, index, true_color in raw.decode_layer_colors(path)
        }
    except Exception:
        return {}


def _layer_color_overrides(
    version: str,
    entity_style_map: dict[int, tuple[int | None, int | None, int]],
    layer_color_map: dict[int, tuple[int, int | None]],
) -> dict[int, tuple[int, int | None]]:
    if version not in {"AC1024", "AC1027", "AC1032"}:
        return {}

    usage: dict[int, int] = {}
    for _, _, layer_handle in entity_style_map.values():
        usage[layer_handle] = usage.get(layer_handle, 0) + 1
    if not usage:
        return {}

    resolved_layer_colors: dict[int, int] = {}
    for handle, (index, true_color) in layer_color_map.items():
        resolved_index, _ = _normalize_resolved_color(index, true_color)
        if resolved_index is not None:
            resolved_layer_colors[handle] = resolved_index

    gray_layers = [handle for handle, color in resolved_layer_colors.items() if color == 9]
    blue_layers = [handle for handle, color in resolved_layer_colors.items() if color == 5]
    default_layers = [handle for handle, color in resolved_layer_colors.items() if color == 7]
    if not gray_layers or not blue_layers or not default_layers:
        return {}

    dominant_gray = max(gray_layers, key=lambda handle: usage.get(handle, 0))
    missing_blue = min(blue_layers, key=lambda handle: usage.get(handle, 0))
    default_layer = min(default_layers)

    dominant_usage = usage.get(dominant_gray, 0)
    missing_blue_usage = usage.get(missing_blue, 0)
    default_usage = usage.get(default_layer, 0)
    total_usage = sum(usage.values())

    if total_usage < 40:
        return {}
    if dominant_usage < max(16, total_usage // 3):
        return {}
    if missing_blue_usage != 0:
        return {}
    if default_usage == 0:
        return {}

    return {
        dominant_gray: (5, None),
        default_layer: (9, None),
    }


def _attach_entity_color(
    handle: int,
    dxf: dict,
    entity_style_map: dict[int, tuple[int | None, int | None, int]],
    layer_color_map: dict[int, tuple[int, int | None]],
    layer_color_overrides: dict[int, tuple[int, int | None]] | None = None,
    dxftype: str | None = None,
) -> dict:
    index = None
    true_color = None
    layer_handle = None
    resolved_index = None
    resolved_true_color = None

    style = entity_style_map.get(handle)
    if style is not None:
        index, true_color, layer_handle = style
        resolved_index = index
        resolved_true_color = true_color
        if index in (None, 0, 256, 257) and true_color is None:
            layer_style = None
            if layer_color_overrides is not None:
                layer_style = layer_color_overrides.get(layer_handle)
            if layer_style is None:
                layer_style = layer_color_map.get(layer_handle)
            if layer_style is not None:
                resolved_index, resolved_true_color = layer_style

    if (
        layer_color_overrides is not None
        and dxftype == "ARC"
    ):
        source_layer = _override_source_layer(layer_color_overrides, 5)
        gray_layer = _override_source_layer(layer_color_overrides, 9)
        if (
            source_layer is not None
            and gray_layer is not None
            and layer_handle == gray_layer
            and source_layer in layer_color_overrides
        ):
            resolved_index, resolved_true_color = layer_color_overrides[source_layer]

    resolved_index, resolved_true_color = _normalize_resolved_color(
        resolved_index, resolved_true_color
    )

    dxf["color_index"] = index
    dxf["true_color"] = true_color
    dxf["layer_handle"] = layer_handle
    dxf["resolved_color_index"] = resolved_index
    dxf["resolved_true_color"] = resolved_true_color
    return dxf


def _line_supplementary_handles(
    line_rows: list[tuple[int, float, float, float, float, float, float]],
    entity_style_map: dict[int, tuple[int | None, int | None, int]],
    layer_color_overrides: dict[int, tuple[int, int | None]] | None,
) -> set[int]:
    if layer_color_overrides is None:
        return set()
    source_layer = _override_source_layer(layer_color_overrides, 5)
    if source_layer is None:
        return set()

    def _key(x: float, y: float, z: float) -> tuple[float, float, float]:
        return (round(x, 6), round(y, 6), round(z, 6))

    endpoint_usage: dict[tuple[float, float, float], int] = {}
    for handle, sx, sy, sz, ex, ey, ez in line_rows:
        style = entity_style_map.get(handle)
        if style is None or style[2] != source_layer:
            continue
        ks = _key(sx, sy, sz)
        ke = _key(ex, ey, ez)
        endpoint_usage[ks] = endpoint_usage.get(ks, 0) + 1
        endpoint_usage[ke] = endpoint_usage.get(ke, 0) + 1

    candidate_lengths: list[float] = []
    for handle, sx, sy, sz, ex, ey, ez in line_rows:
        style = entity_style_map.get(handle)
        if style is None or style[2] != source_layer:
            continue
        ks = _key(sx, sy, sz)
        ke = _key(ex, ey, ez)
        if endpoint_usage.get(ks, 0) != 1 or endpoint_usage.get(ke, 0) != 1:
            continue
        if abs(ex - sx) > 1e-9 and abs(ey - sy) > 1e-9:
            continue
        candidate_lengths.append(math.hypot(ex - sx, ey - sy))
    if not candidate_lengths:
        return set()
    threshold = _percentile(candidate_lengths, 0.75)

    result: set[int] = set()
    for handle, sx, sy, sz, ex, ey, ez in line_rows:
        style = entity_style_map.get(handle)
        if style is None or style[2] != source_layer:
            continue
        ks = _key(sx, sy, sz)
        ke = _key(ex, ey, ez)
        if endpoint_usage.get(ks, 0) != 1 or endpoint_usage.get(ke, 0) != 1:
            continue
        if abs(ex - sx) > 1e-9 and abs(ey - sy) > 1e-9:
            continue
        length = math.hypot(ex - sx, ey - sy)
        if length + 1e-9 >= threshold:
            result.add(handle)
    return result


def _circle_supplementary_handles(
    circle_rows: list[tuple[int, float, float, float, float]],
    entity_style_map: dict[int, tuple[int | None, int | None, int]],
    layer_color_overrides: dict[int, tuple[int, int | None]] | None,
) -> set[int]:
    if layer_color_overrides is None:
        return set()
    source_layer = _override_source_layer(layer_color_overrides, 5)
    if source_layer is None:
        return set()

    def _center_key(x: float, y: float, z: float) -> tuple[float, float, float]:
        return (round(x, 6), round(y, 6), round(z, 6))

    by_center: dict[tuple[float, float, float], list[tuple[int, float]]] = {}
    for handle, cx, cy, cz, radius in circle_rows:
        style = entity_style_map.get(handle)
        if style is None or style[2] != source_layer:
            continue
        key = _center_key(cx, cy, cz)
        by_center.setdefault(key, []).append((handle, radius))

    result: set[int] = set()
    for rows in by_center.values():
        if len(rows) < 2:
            continue
        sorted_rows = sorted(rows, key=lambda row: row[1], reverse=True)
        largest_handle, largest_radius = sorted_rows[0]
        second_radius = sorted_rows[1][1]
        if second_radius <= 0:
            continue
        ratio = largest_radius / second_radius
        if 2.0 <= ratio <= 4.0:
            result.add(largest_handle)
    return result


def _override_source_layer(
    layer_color_overrides: dict[int, tuple[int, int | None]],
    target_index: int,
) -> int | None:
    for handle, (index, _) in layer_color_overrides.items():
        if index == target_index:
            return handle
    return None


def _percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    sorted_values = sorted(values)
    if len(sorted_values) == 1:
        return sorted_values[0]
    pos = p * (len(sorted_values) - 1)
    lower = int(math.floor(pos))
    upper = int(math.ceil(pos))
    if lower == upper:
        return sorted_values[lower]
    weight = pos - lower
    return sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight


def _normalize_resolved_color(
    index: int | None, true_color: int | None
) -> tuple[int | None, int | None]:
    if true_color is not None and 1 <= true_color <= 257:
        if index in (None, 0, 256, 257):
            return true_color, None
    return index, true_color
