from __future__ import annotations

import fnmatch
import math
import re
from functools import lru_cache
from dataclasses import dataclass
from typing import Iterable, Iterator

from . import raw
from .entity import Entity

SUPPORTED_VERSIONS = {"AC1015", "AC1018", "AC1021", "AC1024", "AC1027"}
SUPPORTED_ENTITY_TYPES = (
    "LINE",
    "LWPOLYLINE",
    "POLYLINE_3D",
    "POLYLINE_MESH",
    "POLYLINE_PFACE",
    "3DFACE",
    "SOLID",
    "TRACE",
    "SHAPE",
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
        for dxftype in type_set:
            yield from self._iter_type(dxftype)

    def plot(self, *args, **kwargs):
        from .render import plot

        return plot(self, *args, **kwargs)

    def export_dxf(self, output_path: str, **kwargs):
        from .convert import to_dxf

        return to_dxf(self, output_path, **kwargs)

    def _iter_type(self, dxftype: str) -> Iterator[Entity]:
        decode_path = self.doc.decode_path
        entity_style_map = _entity_style_map(decode_path)
        layer_color_map = _layer_color_map(decode_path)
        layer_color_overrides = _layer_color_overrides(
            self.doc.decode_version, entity_style_map, layer_color_map
        )
        if dxftype == "LINE":
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
            for handle, cx, cy, cz, radius, start_angle, end_angle in raw.decode_arc_entities(
                decode_path
            ):
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

        if dxftype == "POLYLINE_3D":
            for handle, flags_70_bits, closed, points in raw.decode_polyline_3d_with_vertices(
                decode_path
            ):
                yield Entity(
                    dxftype="POLYLINE_3D",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "points": list(points),
                            "flags": int(flags_70_bits),
                            "closed": bool(closed),
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_3D",
                    ),
                )
            return

        if dxftype == "POLYLINE_MESH":
            for (
                handle,
                flags,
                m_vertex_count,
                n_vertex_count,
                closed,
                points,
            ) in raw.decode_polyline_mesh_with_vertices(decode_path):
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
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_MESH",
                    ),
                )
            return

        if dxftype == "POLYLINE_PFACE":
            for (
                handle,
                num_vertices,
                num_faces,
                vertices,
                faces,
            ) in raw.decode_polyline_pface_with_faces(decode_path):
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
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="POLYLINE_PFACE",
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
                style_handle,
            ) in raw.decode_attrib_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
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
                style_handle,
            ) in raw.decode_attdef_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
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
            for (
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
            ) in raw.decode_minsert_entities(decode_path):
                yield Entity(
                    dxftype="MINSERT",
                    handle=handle,
                    dxf=_attach_entity_color(
                        handle,
                        {
                            "insert": (px, py, pz),
                            "xscale": sx,
                            "yscale": sy,
                            "zscale": sz,
                            "rotation": math.degrees(rotation),
                            "column_count": num_columns,
                            "row_count": num_rows,
                            "column_spacing": column_spacing,
                            "row_spacing": row_spacing,
                        },
                        entity_style_map,
                        layer_color_map,
                        layer_color_overrides,
                        dxftype="MINSERT",
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
            "Supported types: LINE, LWPOLYLINE, POLYLINE_3D, POLYLINE_MESH, POLYLINE_PFACE, 3DFACE, SOLID, TRACE, SHAPE, ARC, CIRCLE, ELLIPSE, SPLINE, POINT, TEXT, ATTRIB, ATTDEF, MTEXT, LEADER, HATCH, TOLERANCE, MLINE, MINSERT, DIMENSION"
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


@lru_cache(maxsize=16)
def _present_supported_types(path: str | None) -> tuple[str, ...]:
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
        return tuple(SUPPORTED_ENTITY_TYPES)
    return tuple(dxftype for dxftype in SUPPORTED_ENTITY_TYPES if dxftype in seen)


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
    candidate_types = default_types if path is not None else list(SUPPORTED_ENTITY_TYPES)

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
    if version not in {"AC1024", "AC1027"}:
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
