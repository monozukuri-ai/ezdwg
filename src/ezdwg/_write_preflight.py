"""AC1015 attribute preparation and loss reporting, independent of DXF export."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any

from . import raw
from ._aci import describe_approximation, nearest_aci
from ._dimension import dimension_error
from ._hatch import hatch_error
from ._image import image_error
from ._viewport import viewport_error
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


#: Diagnostics that record a deliberate approximation (such as a true colour
#: rounded to an ACI) rather than a loss; strict writes keep them informational.
INFORMATIONAL_ACTIONS = frozenset({"approximated"})


def strict_losses(diagnostics):
    """The diagnostics that make a strict write or import fail."""
    return [d for d in diagnostics if d.action not in INFORMATIONAL_ACTIONS]


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
        elif kind == "DIMENSION":
            dimension_error(d, number, point)
        elif kind == "IMAGE":
            image_error(d, number, point, direction)
        elif kind == "VIEWPORT":
            viewport_error(d, number, point, direction)
        elif kind == "HATCH":

            def point2(value):
                if not isinstance(value, (tuple, list)) or len(value) not in (2, 3):
                    raise ValueError("expected a two-coordinate point")
                return tuple(number(v) for v in value[:2])

            hatch_error(d, number, point2)
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
    image_rows = {}
    if any(e.dxftype == "IMAGE" for e in entities):
        try:
            defs = {row[0]: row for row in raw.decode_image_defs(path)}
        except (OSError, ValueError, NotImplementedError):
            defs = {}
        for e in entities:
            if e.dxftype != "IMAGE":
                continue
            row = defs.get(e.dxf.get("image_def_handle"))
            if row is None:
                report(
                    int(e.handle),
                    "IMAGE",
                    "image_def",
                    "image definition could not be decoded",
                    "skipped",
                )
                e.dxf["image_def"] = ""
                continue
            name = e.dxf.get("image_def") or _image_def_name(row[1], row[0])
            e.dxf["image_def"] = name
            image_rows.setdefault(
                name,
                {
                    "name": name,
                    "file_path": row[1],
                    "size": tuple(row[2]),
                    "is_loaded": bool(row[3]),
                    "resolution_units": int(row[4]),
                    "pixel_size": tuple(row[5]),
                },
            )

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
                if layer.true_color is not None:
                    index = nearest_aci(layer.true_color)
                    layer_rows[layer.name]["color_index"] = index
                    report(
                        layer.handle,
                        "LAYER",
                        "true_color",
                        f"layer {layer.name!r}: "
                        + describe_approximation(layer.true_color, index),
                        "approximated",
                    )
                for attr in ("transparency", "description", "standard"):
                    if getattr(layer, attr, None) not in (None, 0, ""):
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
        if d.get("true_color") is not None:
            try:
                index = nearest_aci(d["true_color"])
            except ValueError:
                report(
                    handle,
                    e.dxftype,
                    "true_color",
                    "invalid true color is ignored",
                    "dropped",
                )
            else:
                report(
                    handle,
                    e.dxftype,
                    "true_color",
                    describe_approximation(d["true_color"], index),
                    "approximated",
                )
        a["color_index"] = index
        for attr in (
            "transparency",
            "background_flags",
            "xdata",
            "extension_dict",
            "lock_position",
        ):
            if d.get(attr) not in (None, 0, "", {}, []):
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
        image_rows=image_rows,
    )


def _image_def_name(file_path: str, handle: int) -> str:
    stem = file_path.replace("\\", "/").rsplit("/", 1)[-1].rsplit(".", 1)[0]
    return stem or f"IMAGE_{handle}"


# Legacy code pages the AC1015 writer can store, as (Python codec, DWG $DWGCODEPAGE index).
# Every TV string of a drawing is encoded with the single code page the header names, so one
# choice has to cover the whole drawing. The codec names match the encoding_rs labels the Rust
# reader/writer uses for the same index (bit_reader::dwg_codepage_encoding).
WRITE_CODEPAGES: tuple[tuple[str, int], ...] = (
    ("cp1252", 30),  # ANSI_1252 (Western Europe)
    ("cp932", 38),  # ANSI_932 (Japanese)
    ("gbk", 39),  # ANSI_936 (Simplified Chinese)
    ("cp949", 40),  # ANSI_949 (Korean)
    ("big5", 41),  # ANSI_950 (Traditional Chinese)
    ("cp1251", 29),  # ANSI_1251 (Cyrillic)
    ("cp1250", 28),  # ANSI_1250 (Central Europe)
    ("cp1253", 32),  # ANSI_1253 (Greek)
    ("cp1254", 33),  # ANSI_1254 (Turkish)
    ("cp1255", 34),  # ANSI_1255 (Hebrew)
    ("cp1256", 35),  # ANSI_1256 (Arabic)
    ("cp1257", 36),  # ANSI_1257 (Baltic)
    ("cp874", 37),  # ANSI_874 (Thai)
)
_CODEPAGE_CODEC = {code: codec for codec, code in WRITE_CODEPAGES}
_CODEC_CODEPAGE = {codec: code for codec, code in WRITE_CODEPAGES}
_CJK_CODECS = ("cp932", "gbk", "cp949", "big5")
_SINGLE_BYTE_CODECS = ("cp1251", "cp1250", "cp1253", "cp1254", "cp1255", "cp1256", "cp1257", "cp874")


def _has_char(strings, lo, hi):
    return any(lo <= ord(ch) <= hi for text in strings for ch in text)


def _codec_preference(strings):
    """Preference order for the automatic choice.

    ``cp1252`` first keeps ASCII/Latin drawings unchanged. The CJK code pages overlap a lot
    (GBK stores kana and every unified ideograph, cp932 stores basic Cyrillic and Greek), so
    the order decides the ambiguous cases: ideographs without kana stay ANSI_932 (the main
    market of this writer) unless only GBK stores them all, Hangul puts ANSI_949 first, and
    Cyrillic / Thai / Hebrew / Arabic text without any CJK script takes its own Windows code
    page instead of ANSI_932. Greek letters alone (``φ100`` in Japanese drawings) stay
    ANSI_932.
    """
    hangul = _has_char(strings, 0xAC00, 0xD7AF) or _has_char(strings, 0x3130, 0x318F)
    kana = _has_char(strings, 0x3040, 0x30FF) or _has_char(strings, 0xFF66, 0xFF9F)
    han = _has_char(strings, 0x4E00, 0x9FFF) or _has_char(strings, 0x3400, 0x4DBF)
    other_script = (
        _has_char(strings, 0x0400, 0x04FF)  # Cyrillic
        or _has_char(strings, 0x0E00, 0x0E7F)  # Thai
        or _has_char(strings, 0x0590, 0x05FF)  # Hebrew
        or _has_char(strings, 0x0600, 0x06FF)  # Arabic
    )
    cjk = list(_CJK_CODECS)
    if hangul and not kana:
        cjk.remove("cp949")
        cjk.insert(0, "cp949")
    if other_script and not (hangul or kana or han):
        order = ["cp1252", *_SINGLE_BYTE_CODECS, *cjk]
    else:
        order = ["cp1252", *cjk, *_SINGLE_BYTE_CODECS]
    return [(codec, _CODEC_CODEPAGE[codec]) for codec in order]


def select_codepage(strings, requested=None):
    """Choose the legacy code page for a drawing's strings.

    ``requested`` (``Drawing.header["codepage"]``) forces one of ``WRITE_CODEPAGES``.
    Kana anywhere in the drawing forces ANSI_932 and Hangul (without kana) ANSI_949, so a
    Japanese or Korean drawing keeps its usual code page even when a few symbols (``é``,
    simplified ideographs) have to become ``?``. Otherwise the first code page of
    ``_codec_preference`` that stores every string wins; when none does, the one that
    leaves the fewest strings unrepresentable (ties keep the preference order).
    Returns ``(codec, codepage)``.
    """
    if requested in _CODEPAGE_CODEC:
        return _CODEPAGE_CODEC[requested], requested
    strings = [text for text in strings if isinstance(text, str)]
    if _has_char(strings, 0x3040, 0x30FF) or _has_char(strings, 0xFF66, 0xFF9F):
        return "cp932", 38
    if _has_char(strings, 0xAC00, 0xD7AF) or _has_char(strings, 0x3130, 0x318F):
        return "cp949", 40
    best = None
    for codec, code in _codec_preference(strings):
        failures = 0
        for text in strings:
            try:
                text.encode(codec)
            except UnicodeEncodeError:
                failures += 1
        if failures == 0:
            return codec, code
        if best is None or failures < best[0]:
            best = (failures, codec, code)
    return (best[1], best[2]) if best else ("cp1252", 30)


def prepare_strings(
    entities,
    attrs,
    layer_rows,
    ltype_rows,
    style_rows,
    metadata,
    diagnostics,
    block_rows=None,
    image_rows=None,
    layout_row=None,
):
    """Prepare independent copies for legacy text encoding and report losses.

    ``layout_row`` (paper-space LAYOUT settings) is encoded like the table rows
    and returned as the last element; ``None`` keeps the writer defaults.
    """

    def report(handle, kind, attr, message, action="defaulted"):
        diagnostics.append(WriteDiagnostic(handle, kind, attr, message, action))

    # Select a legacy encoding for all strings, including table and font names.
    block_rows = block_rows or {}
    image_rows = image_rows or {}
    layout_row = dict(layout_row) if layout_row is not None else None
    strings = [
        str(e.dxf.get(k, ""))
        for e in entities
        for k in ("text", "raw_text", "tag", "prompt", "name", "image_def", "style_sheet")
    ]
    strings += [
        v
        for r in (
            *layer_rows.values(),
            *ltype_rows.values(),
            *style_rows.values(),
            *block_rows.values(),
            *image_rows.values(),
            *([layout_row] if layout_row else []),
        )
        for v in r.values()
        if isinstance(v, str)
    ]

    requested = metadata.get("codepage")
    if requested is not None and requested not in _CODEPAGE_CODEC:
        report(None, "HEADER", "codepage", "unsupported code page; selected automatically")
        requested = None
    codec, metadata["codepage"] = select_codepage(strings, requested)

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
        for key in ("text", "raw_text", "tag", "prompt", "name", "style_sheet"):
            if isinstance(d.get(key), str):
                d[key] = encode_text(d[key], e.handle, e.dxftype, key)
        # Frozen layer references must match the encoded LAYER record names.
        if e.dxftype == "VIEWPORT" and isinstance(d.get("frozen_layers"), list):
            d["frozen_layers"] = [
                name.encode(codec, errors="replace").decode(codec)
                for name in d["frozen_layers"]
            ]
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
        (image_rows, "IMAGEDEF"),
        ({"layout": layout_row} if layout_row else {}, "LAYOUT"),
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
        list(image_rows.values()),
        layout_row,
    )
