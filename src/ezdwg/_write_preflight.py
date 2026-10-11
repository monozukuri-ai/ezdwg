"""AC1015 attribute preparation and loss reporting, independent of DXF export."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any

from . import raw
from ._write_spline import FIELDS as SPLINE_FIELDS
from ._write_spline import validate_spline
from .entity import Entity


@dataclass(frozen=True)
class WriteDiagnostic:
    handle: int | None
    dxftype: str
    attribute: str
    message: str
    action: str = "defaulted"


LINEWEIGHTS = {
    0,
    5,
    9,
    13,
    15,
    18,
    20,
    25,
    30,
    35,
    40,
    50,
    53,
    60,
    70,
    80,
    90,
    100,
    106,
    120,
    140,
    158,
    200,
    211,
    -1,
    -2,
    -3,
}
ATTRIBUTES = {
    "extrusion",
    "thickness",
    "elevation",
    "align_point",
    "oblique",
    "width",
    "text_generation_flag",
    "halign",
    "valign",
    "line_spacing_style",
    "line_spacing_factor",
}
GEOMETRY_TYPES = {"LINE", "ARC", "CIRCLE", "POINT", "LWPOLYLINE", "SOLID", "TRACE"}


def geometry_error(e: Entity) -> str | None:
    """Validate before crossing FFI; invalid geometry is skipped, never repaired."""
    d, kind = e.dxf, e.dxftype
    try:

        def number(value):
            v = float(value)
            if not math.isfinite(v):
                raise ValueError("coordinates and numeric attributes must be finite")
            return v

        def point(value):
            if not isinstance(value, (tuple, list)) or len(value) != 3:
                raise ValueError("expected a three-coordinate point")
            return tuple(number(v) for v in value)

        def direction(value):
            if point(value) == (0.0, 0.0, 0.0):
                raise ValueError("direction/extrusion must be nonzero")

        for key in ("thickness", "elevation", "oblique", "rotation", "x_axis_angle"):
            if key in d:
                number(d[key])
        direction(d.get("extrusion", (0.0, 0.0, 1.0)))
        if kind == "LINE":
            point(d["start"])
            point(d["end"])
        elif kind in {"RAY", "XLINE"}:
            point(d["start"])
            direction(d["unit_vector"])
        elif kind == "POINT":
            point(d["location"])
        elif kind in {"ARC", "CIRCLE"}:
            point(d["center"])
            if number(d["radius"]) <= 0:
                raise ValueError("radius must be positive")
            if kind == "ARC":
                number(d["start_angle"])
                number(d["end_angle"])
        elif kind in {"SOLID", "TRACE", "3DFACE"}:
            points = d["points"]
            if len(points) not in (3, 4):
                raise ValueError("SOLID/TRACE/3DFACE require three or four corners")
            corners = [point(p) for p in points]
            if kind in {"SOLID", "TRACE"}:
                if any(p[2] != corners[0][2] for p in corners):
                    raise ValueError("SOLID/TRACE corners must have the same OCS Z")
            else:
                flags = d.get("invisible_edge_flags", 0)
                if (
                    isinstance(flags, bool)
                    or not isinstance(flags, int)
                    or not 0 <= flags <= 15
                ):
                    raise ValueError("3DFACE invisible edge flags must be in 0..15")
                if number(d.get("thickness", 0)) != 0 or point(
                    d.get("extrusion", (0, 0, 1))
                ) != (0, 0, 1):
                    raise ValueError("3DFACE cannot store thickness/extrusion")
        elif kind == "SPLINE":
            validate_spline(d, number, point)
        elif kind == "ELLIPSE":
            point(d["center"])
            axis = point(d["major_axis"])
            normal = point(d.get("extrusion", (0.0, 0.0, 1.0)))
            if not 1e-6 <= number(d["axis_ratio"]) <= 1.0:
                raise ValueError("ellipse axis ratio must be in 1e-6..1")
            alen, nlen = math.hypot(*axis), math.hypot(*normal)
            if not all(math.isfinite(v) and v > 1e-6 for v in (alen, nlen)):
                raise ValueError(
                    "ellipse axis/normal length must be finite and greater than 1e-6"
                )
            if abs(sum((a / alen) * (n / nlen) for a, n in zip(axis, normal))) > 1e-6:
                raise ValueError(
                    "ellipse major axis must be perpendicular to extrusion"
                )
            span = number(d["end_angle"]) - number(d["start_angle"])
            full = abs(abs(span) - math.tau) <= 1e-12
            if not (
                math.isfinite(span)
                and abs(span) <= math.tau + 1e-12
                and (full or span % math.tau > 1e-6)
            ):
                raise ValueError(
                    "ellipse parameter span must be nonzero and at most one revolution"
                )
        elif kind == "INSERT":
            point(d["insert"])
            if not isinstance(d.get("name"), str) or not d["name"]:
                raise ValueError("INSERT requires a block name")
            if any(number(d.get(k, 1.0)) == 0 for k in ("xscale", "yscale", "zscale")):
                raise ValueError("INSERT scale must be nonzero")
        elif kind == "LWPOLYLINE":
            points = d["points"]
            if not points:
                raise ValueError("LWPOLYLINE requires vertices")
            for p in points:
                number(p[0])
                number(p[1])
            for key in ("bulges", "widths"):
                if len(d.get(key) or []) > len(points):
                    raise ValueError(f"{key} exceeds vertex count")
            for v in d.get("bulges") or []:
                number(v)
            widths = [v for pair in (d.get("widths") or []) for v in pair]
            if any(len(pair) != 2 for pair in d.get("widths") or []):
                raise ValueError("widths require start/end pairs")
            if d.get("const_width") is not None:
                widths.append(d["const_width"])
            if any(number(v) < 0 for v in widths):
                raise ValueError("polyline width must be nonnegative")
        elif kind in {"POLYLINE_2D", "POLYLINE_3D"}:
            is_3d = kind == "POLYLINE_3D"
            points = d["points"]
            if len(points) < 2:
                raise ValueError("POLYLINE requires at least two vertices")
            flags = d.get("flags", 8 if is_3d else 0)
            if (
                not isinstance(flags, int)
                or flags < 0
                or flags & ~(9 if is_3d else 129)
                or d.get("curve_type", 0) != 0
            ):
                raise ValueError("unsupported POLYLINE flags/curve fitting")
            for p in points:
                if not isinstance(p, (list, tuple)) or len(p) != (3 if is_3d else 2):
                    raise ValueError(
                        "3D vertices require XYZ; 2D vertices require XY with separate elevation"
                    )
                for v in p:
                    number(v)
            for key in ("bulges", "widths"):
                if len(d.get(key) or []) > len(points):
                    raise ValueError(f"{key} exceeds vertex count")
            for v in d.get("bulges") or []:
                if is_3d and number(v) != 0:
                    raise ValueError("3D POLYLINE cannot store bulges")
                number(v)
            widths = [d.get("width_start", 0), d.get("width_end", 0)]
            for pair in d.get("widths") or []:
                if len(pair) != 2:
                    raise ValueError("widths require start/end pairs")
                widths.extend(pair)
            if any(number(v) < 0 or (is_3d and number(v) != 0) for v in widths):
                raise ValueError("invalid POLYLINE widths")
        elif kind in {"TEXT", "MTEXT", "ATTDEF", "ATTRIB"}:
            point(d["insert"])
            height = d.get("char_height", d.get("height"))
            if number(height) <= 0:
                raise ValueError("text height must be positive")
            text = d.get("raw_text", d.get("text"))
            if not isinstance(text, str) or "\0" in text:
                raise ValueError("text must be a string without NUL")
            if kind != "MTEXT":
                if (
                    number(d.get("width", 1.0)) <= 0
                    or abs(number(d.get("oblique", 0.0))) >= 90
                ):
                    raise ValueError("invalid text width/oblique")
                h, v, g = (
                    d.get(k, 0) for k in ("halign", "valign", "text_generation_flag")
                )
                if (
                    h not in range(6)
                    or v not in range(4)
                    or g not in (0, 2, 4, 6)
                    or (h >= 3 and v != 0)
                ):
                    raise ValueError("invalid text alignment/generation")
                align = d.get("align_point")
                if (h or v) and align is None:
                    raise ValueError("aligned text requires alignment point")
                if align is not None and point(align)[2] != point(d["insert"])[2]:
                    raise ValueError("text alignment elevation differs from insertion")
            else:
                direction(d.get("text_direction", (1.0, 0.0, 0.0)))
                if (
                    number(d.get("rect_width", 0.0)) < 0
                    or d.get("attachment_point", 1) not in range(1, 10)
                    or d.get("drawing_direction", 1) not in (1, 3, 5)
                ):
                    raise ValueError("invalid MTEXT width/attachment/direction")
                if (
                    d.get("line_spacing_style", 1) not in (1, 2)
                    or not 0.25 <= number(d.get("line_spacing_factor", 1.0)) <= 4
                ):
                    raise ValueError("invalid MTEXT spacing")
        if kind in {"ATTDEF", "ATTRIB"}:
            tag = d.get("tag")
            if (
                not isinstance(tag, str)
                or not tag
                or any(c.isspace() or c == "!" for c in tag)
            ):
                raise ValueError("invalid attribute tag")
            if d.get("attribute_flags", 0) not in range(16):
                raise ValueError("unsupported attribute flags")
    except (KeyError, TypeError, ValueError, OverflowError, IndexError) as exc:
        return str(exc)
    return None


def prepare_attributes(doc: Any, entities: list[Entity]):
    diagnostics: list[WriteDiagnostic] = []

    def report(handle, kind, attr, message, action="defaulted"):
        diagnostics.append(WriteDiagnostic(handle, kind, attr, message, action))

    path = doc.decode_path or doc.path
    try:
        layers_by_handle = {l.handle: l for l in doc.layers}
    except (OSError, ValueError, NotImplementedError):
        layers_by_handle = {}
        report(None, "DOCUMENT", "layers", "source layer table could not be decoded")
    try:
        linetypes = doc.linetypes()
        table_rows = raw.decode_write_tables(path)
    except (OSError, ValueError, NotImplementedError):
        linetypes, table_rows = {}, []
        report(
            None,
            "DOCUMENT",
            "tables",
            "source style/linetype tables could not be decoded",
        )
    styles_by_handle = {r["handle"]: r for r in table_rows if r["kind"] == "STYLE"}
    extended_ltypes = {
        r["name"].upper()
        for r in table_rows
        if r["kind"] == "LTYPE" and r.get("extended_data")
    }
    simple_ltypes = {
        r["name"].upper()
        for r in table_rows
        if r["kind"] == "LTYPE" and not r["complex"]
    }
    try:
        header = doc.header_variables()
    except (OSError, ValueError, NotImplementedError):
        header = {}
        report(None, "DOCUMENT", "units", "source units could not be decoded")
    defaults = {
        "insunits": 0,
        "lunits": 2,
        "luprec": 4,
        "aunits": 0,
        "auprec": 0,
        "ltscale": 1.0,
    }
    metadata = {
        k: header.get(k) if header.get(k) is not None else v
        for k, v in defaults.items()
    }
    valid_units = {
        "insunits": range(21),
        "lunits": range(1, 6),
        "luprec": range(9),
        "aunits": range(5),
        "auprec": range(9),
    }
    for key, allowed in valid_units.items():
        if metadata[key] not in allowed:
            report(None, "HEADER", key, "value is not supported by AC1015")
            metadata[key] = defaults[key]
    if (
        not isinstance(metadata["ltscale"], (int, float))
        or not math.isfinite(metadata["ltscale"])
        or metadata["ltscale"] <= 0
    ):
        report(None, "HEADER", "ltscale", "LTSCALE must be finite and positive")
        metadata["ltscale"] = 1.0

    try:
        property_losses = dict(raw.decode_write_property_losses(path))
    except (OSError, ValueError, NotImplementedError):
        property_losses = {}
        report(
            None,
            "DOCUMENT",
            "properties",
            "source common attributes could not be decoded",
        )
    layer_rows, ltype_rows, style_rows, attrs = {}, {}, {}, {}

    def linetype(name, handle, kind):
        if name is None:
            report(handle, kind, "linetype", "unresolved linetype")
            return "CONTINUOUS" if kind == "LAYER" else "BYLAYER"
        if kind == "LAYER" and name.upper() in {"BYLAYER", "BYBLOCK"}:
            report(
                handle,
                kind,
                "linetype",
                "layer linetype cannot inherit; replaced with Continuous",
            )
            return "CONTINUOUS"
        if name.upper() in {"BYLAYER", "BYBLOCK", "CONTINUOUS"}:
            return name.upper()
        row = next((r for n, r in linetypes.items() if n.upper() == name.upper()), None)
        if row is None or name.upper() not in simple_ltypes:
            report(
                handle,
                kind,
                "linetype",
                f"complex or undecoded linetype {name!r} is replaced with Continuous",
            )
            return "CONTINUOUS"
        if name.upper() in extended_ltypes:
            report(
                handle,
                kind,
                "linetype.xdata",
                f"linetype {name!r}: extended data is not preserved",
                "dropped",
            )
        dashes = row["dashes"]
        if (
            len(dashes) > 12
            or not all(math.isfinite(v) for v in dashes)
            or not math.isfinite(sum(abs(v) for v in dashes))
        ):
            report(handle, kind, "linetype", "invalid simple dash pattern")
            return "CONTINUOUS"
        ltype_rows[name] = {
            "name": name,
            "description": row["description"],
            "dashes": dashes,
        }
        return name

    spline_rows = (
        {r["handle"]: r for r in raw.decode_write_splines(path)}
        if any(e.dxftype == "SPLINE" for e in entities)
        else {}
    )
    polyline_rows = (
        {r["handle"]: r for r in raw.decode_write_polylines(path)}
        if any(e.dxftype in {"POLYLINE_2D", "POLYLINE_3D"} for e in entities)
        else {}
    )
    for e in entities:
        d, handle = e.dxf, int(e.handle)
        if e.dxftype == "SPLINE":
            row = spline_rows.get(handle)
            if row is None:
                d["scenario"] = 0
                report(
                    handle,
                    e.dxftype,
                    "geometry",
                    "spline metadata/tangents could not be decoded",
                    "skipped",
                )
            else:
                d.update({k: v for k, v in row.items() if k in SPLINE_FIELDS})
                flags = row.get("spline_flags1") or 0
                parameter = row.get("knot_parameter")
                if flags & ~13:
                    report(
                        handle,
                        e.dxftype,
                        "spline_flags1",
                        "control frame display/unknown spline flags cannot be preserved",
                        "dropped",
                    )
                if d["scenario"] == 2 and (parameter not in (None, 0) or flags & 4):
                    d["scenario"] = 0
                    report(
                        handle,
                        e.dxftype,
                        "knot_parameter",
                        "non-chord or closed fit spline cannot be preserved in AC1015",
                        "skipped",
                    )
                elif d["scenario"] == 1 and flags & 1:
                    report(
                        handle,
                        e.dxftype,
                        "spline_flags1",
                        "fit method metadata on a control spline cannot be preserved",
                        "dropped",
                    )
        if e.dxftype in {"POLYLINE_2D", "POLYLINE_3D"}:
            row = polyline_rows.get(handle)
            if row is None:
                d["points"] = []
                report(
                    handle,
                    e.dxftype,
                    "vertices",
                    "polyline metadata/ownership could not be decoded",
                    "skipped",
                )
            else:
                vertices = row["vertices"]
                d.update(
                    flags=row["flags"],
                    closed=bool(row["flags"] & 1),
                    curve_type=row["curve_type"],
                )
                d["points"] = [
                    p if e.dxftype == "POLYLINE_3D" else p[:2] for p, *_ in vertices
                ]
                if e.dxftype == "POLYLINE_2D":
                    d.update(
                        {
                            k: row[k]
                            for k in (
                                "width_start",
                                "width_end",
                                "thickness",
                                "elevation",
                                "extrusion",
                            )
                        }
                    )
                    d["widths"] = [(v[1], v[2]) for v in vertices]
                    d["bulges"] = [v[3] for v in vertices]
                for loss in row["losses"]:
                    report(handle, e.dxftype, "vertices", loss, "dropped")
                for child in [*row["vertex_handles"], row["seqend_handle"]]:
                    for loss in property_losses.get(
                        child, ("common_properties_undecoded",)
                    ):
                        report(
                            handle,
                            e.dxftype,
                            "vertices",
                            f"child {child}: {loss} cannot be preserved",
                            "dropped",
                        )
                if not row["complete"]:
                    d["points"] = []
                    report(
                        handle,
                        e.dxftype,
                        "vertices",
                        "missing, inconsistent or unsupported vertex/SEQEND references",
                        "skipped",
                    )
        for key in property_losses.get(handle, ()):
            report(
                handle, e.dxftype, key, f"source {key} cannot be preserved", "dropped"
            )
        a = {k: d[k] for k in ATTRIBUTES if k in d and d[k] is not None}
        layer = layers_by_handle.get(d.get("layer_handle"))
        if layer is None:
            a["layer"] = "0"
            if "layer_handle" not in d or d.get("layer_handle") not in (0,):
                report(
                    handle,
                    e.dxftype,
                    "layer",
                    "unresolved layer reference is replaced with layer 0",
                )
        else:
            a["layer"] = layer.name
            if layer.name not in layer_rows:
                layer_rows[layer.name] = {
                    "name": layer.name,
                    "color_index": layer.color if layer.color in range(1, 256) else 7,
                    "linetype": linetype(layer.linetype, layer.handle, "LAYER"),
                    "lineweight": layer.lineweight
                    if layer.lineweight in LINEWEIGHTS
                    else -3,
                    "frozen": layer.frozen,
                    "off": not layer.on,
                    "frozen_in_new_viewports": layer.frozen_in_new_viewports,
                    "locked": layer.locked,
                    "plot": layer.plot,
                }
                for attr in ("true_color", "transparency", "description", "standard"):
                    if (attr == "true_color" and layer.true_color is not None) or (
                        attr != "true_color"
                        and getattr(layer, attr, None) not in (None, 0, "")
                    ):
                        report(
                            layer.handle,
                            "LAYER",
                            attr,
                            f"layer {layer.name!r}: {attr} is not preserved",
                            "dropped",
                        )
        index = d.get("color_index")
        if index is None:
            report(
                handle,
                e.dxftype,
                "color_index",
                "undecoded ACI is replaced with BYLAYER",
            )
            index = 256
        if index not in range(257):
            report(
                handle, e.dxftype, "color_index", "invalid ACI is replaced with BYLAYER"
            )
            index = 256
        a["color_index"] = index
        for attr in (
            "true_color",
            "transparency",
            "background_flags",
            "xdata",
            "extension_dict",
            "lock_position",
        ):
            if (attr == "true_color" and d.get(attr) is not None) or (
                attr != "true_color" and d.get(attr) not in (None, 0, "", {}, [])
            ):
                report(
                    handle,
                    e.dxftype,
                    attr,
                    f"{attr} is not preserved in this AC1015 export",
                    "dropped",
                )
        a["linetype"] = linetype(d.get("linetype"), handle, e.dxftype)
        scale = d.get("linetype_scale", 1.0)
        if (
            not isinstance(scale, (int, float))
            or not math.isfinite(scale)
            or scale <= 0
        ):
            report(
                handle, e.dxftype, "linetype_scale", "invalid scale is replaced with 1"
            )
            scale = 1.0
        a["linetype_scale"] = scale
        weight = d.get("lineweight", -1)
        if weight is None:
            weight = -1
        if weight not in LINEWEIGHTS:
            report(
                handle,
                e.dxftype,
                "lineweight",
                "invalid weight is replaced with BYLAYER",
            )
            weight = -1
        a["lineweight"] = weight
        a["invisible"] = bool(d.get("invisible", False))
        if e.dxftype in GEOMETRY_TYPES and (
            "extrusion" not in d or "thickness" not in d
        ):
            report(
                handle,
                e.dxftype,
                "geometry_attributes",
                "extrusion/thickness could not be fully decoded",
            )
        if e.dxftype in {"TEXT", "MTEXT", "ATTDEF", "ATTRIB"}:
            st = styles_by_handle.get(d.get("style_handle"))
            if st is None or st.get("shape"):
                report(
                    handle,
                    e.dxftype,
                    "style",
                    "undecoded/shape text style is replaced with Standard",
                )
            else:
                if st.get("extended_data"):
                    report(
                        handle,
                        e.dxftype,
                        "style.xdata",
                        "extended font/style data is not preserved",
                        "dropped",
                    )
                a["style"] = st["name"]
                style_rows[st["name"]] = {
                    k: v
                    for k, v in st.items()
                    if k not in {"kind", "handle", "shape", "extended_data"}
                }
        attrs[handle] = a

    return prepare_strings(
        entities,
        attrs,
        layer_rows,
        ltype_rows,
        style_rows,
        metadata,
        diagnostics,
        block_rows=None,
    )


def prepare_strings(
    entities,
    attrs,
    layer_rows,
    ltype_rows,
    style_rows,
    metadata,
    diagnostics,
    block_rows=None,
):
    """Prepare independent copies for legacy text encoding and report losses."""

    def report(handle, kind, attr, message, action="defaulted"):
        diagnostics.append(WriteDiagnostic(handle, kind, attr, message, action))

    # Select a legacy encoding for all strings, including table and font names.
    block_rows = block_rows or {}
    strings = [
        str(e.dxf.get(k, ""))
        for e in entities
        for k in ("text", "raw_text", "tag", "prompt", "name")
    ]
    strings += [
        v
        for r in (
            *layer_rows.values(),
            *ltype_rows.values(),
            *style_rows.values(),
            *block_rows.values(),
        )
        for v in r.values()
        if isinstance(v, str)
    ]

    def encodable(codec):
        try:
            for text in strings:
                text.encode(codec)
            return True
        except UnicodeEncodeError:
            return False

    codec, metadata["codepage"] = (
        ("cp1252", 30) if encodable("cp1252") else ("cp932", 38)
    )

    # Replace only with explicit diagnostics; strict will reject before writing.
    def encode_text(value, handle, kind, attr):
        try:
            data = value.encode(codec)
            if data.decode(codec) != value or len(data) > 65535 or "\0" in value:
                raise ValueError("string is too long or contains NUL")
            return value
        except (UnicodeEncodeError, ValueError):
            report(
                handle, kind, attr, f"string cannot be stored in {codec}", "replaced"
            )
            return (
                value.replace("\0", " ")
                .encode(codec, errors="replace")[:65535]
                .decode(codec, errors="ignore")
            )

    prepared = []
    for e in entities:
        d = dict(e.dxf)
        for key in ("text", "raw_text", "tag", "prompt", "name"):
            if isinstance(d.get(key), str):
                d[key] = encode_text(d[key], e.handle, e.dxftype, key)
        # Only the legacy raw tuple accepts DXF bit 1 as closure. Decoded DWG
        # flag 1 is an explicit normal; carry closure independently here.
        if e.dxftype == "LWPOLYLINE":
            d["flags"] = (int(d.get("flags", 0)) & 0x80) | (
                0x200 if d.get("closed", int(d.get("flags", 0)) & 0x200) else 0
            )
        prepared.append(Entity(e.dxftype, e.handle, d))
    for rows, kind in (
        (layer_rows, "LAYER"),
        (ltype_rows, "LTYPE"),
        (style_rows, "STYLE"),
        (block_rows, "BLOCK"),
    ):
        for row in rows.values():
            for key, value in row.items():
                if isinstance(value, str):
                    row[key] = encode_text(value, None, kind, key)
    # References must use exactly the same encoded names as the table records.
    for a in attrs.values():
        for key in ("layer", "linetype", "style"):
            if key in a:
                a[key] = a[key].encode(codec, errors="replace").decode(codec)
    return (
        prepared,
        attrs,
        list(layer_rows.values()),
        list(ltype_rows.values()),
        list(style_rows.values()),
        metadata,
        diagnostics,
    )
