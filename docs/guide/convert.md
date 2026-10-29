# DWG to DXF Conversion

ezdwg can convert DWG files to DXF format using [ezdxf](https://ezdxf.readthedocs.io/) as the writing backend.

```bash
pip install "ezdwg[dxf]"
```

## Basic Usage

### From a File Path

```python
import ezdwg

result = ezdwg.to_dxf("input.dwg", "output.dxf")
print(f"Written: {result.written_entities}/{result.total_entities}")
```

### From a Document

```python
doc = ezdwg.read("input.dwg")
result = doc.export_dxf("output.dxf")
```

### From a Layout

```python
doc = ezdwg.read("input.dwg")
msp = doc.modelspace()
result = msp.export_dxf("output.dxf")
```

## Options

### Filtering by Entity Type

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    types="LINE ARC LWPOLYLINE",
)
```

### DXF Version

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    dxf_version="R2010",  # default
)
```

Supported DXF versions: `R2000`, `R2004`, `R2007`, `R2010`, `R2013`, `R2018`.

### Strict Mode

In strict mode, conversion fails if any entity cannot be written:

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    strict=True,
)
```

## ConvertResult

`to_dxf()` returns a `ConvertResult` with:

| Attribute | Type | Description |
|-----------|------|-------------|
| `source_path` | `str` | Input DWG path |
| `output_path` | `str` | Output DXF path |
| `total_entities` | `int` | Total entities processed |
| `written_entities` | `int` | Entities successfully written |
| `skipped_entities` | `int` | Entities that could not be written |
| `skipped_by_type` | `dict[str, int]` | Skip counts grouped by type |

## Supported Entity Types

The following entity types can be written to DXF:

LINE, RAY, XLINE, POINT, ARC, CIRCLE, ELLIPSE, LWPOLYLINE,
POLYLINE_2D, POLYLINE_3D, POLYLINE_MESH, POLYLINE_PFACE,
3DFACE, SOLID, TRACE, SHAPE, SPLINE,
TEXT, ATTRIB, ATTDEF, MTEXT, LEADER, HATCH, TOLERANCE, MLINE,
INSERT, MINSERT, DIMENSION

## Limitations

- Conversion is best-effort — some entities may be skipped if they cannot be represented in DXF.
- Block definitions referenced by INSERT entities are reconstructed from the DWG source.
- DIMENSION entities use native ezdxf dimension builders with a text fallback for unsupported subtypes.
- Style, linetype, and layer properties may not be fully preserved.

---

## Native DWG Writer (AC1015)

ezdwg also provides a native DWG writer for AC1015.

```python
import ezdwg

result = ezdwg.to_dwg(
    "input.dwg",
    "output.dwg",
    version="AC1015",
    types="LINE ARC CIRCLE LWPOLYLINE TEXT MTEXT",
)
print(result.written_entities, result.skipped_by_type)
```

You can also call:

```python
doc = ezdwg.read("input.dwg")
doc.export_dwg("output.dwg", version="AC1015")
```

Current native writer scope:

- Version: `AC1015` only
- Entities: `LINE`, `RAY`, `XLINE`, `POINT`, `ARC`, `CIRCLE`, `ELLIPSE`, `SPLINE`, `SOLID`, `TRACE`, `3DFACE`, `LWPOLYLINE`, `POLYLINE_2D`, `POLYLINE_3D`, `TEXT`, `MTEXT`, `INSERT`, `ATTDEF`, `ATTRIB`, `HATCH`, `DIMENSION` (all seven kinds), `IMAGE`, `VIEWPORT` (paper space only)
- Named/anonymous static block definitions, including nested INSERTs and attached attributes
- One paper-space layout (`Layout1`): `Drawing.set_paper()` / `Drawing.layout` for the sheet (millimetres, margins, paper size name, plot settings) and `Drawing.paperspace()` for its entities and `add_viewport()`; the sheet's own viewport is written in front of the user viewports
- Raster image definitions (`IMAGEDEF`, `IMAGEDEF_REACTOR`, `RASTERVARIABLES`, `ACAD_IMAGE_DICT`) and a `Standard` DIMSTYLE
- True colours are rounded to the nearest ACI (R2000 stores ACI only) and reported as `approximated`

A file path or `Document` exports **model space**. It does not export block
definitions as loose geometry. An explicitly supplied `Layout` selects that
layout's entities, which are written into the new file's model space. Referenced
block definitions and attached attributes are included as dependencies.

The writer builds a new AC1015 database with standard tables, model/paper space
records, layouts, dictionaries, entity links and CRCs. It has no runtime
dependency on an external converter or a template drawing. This is a basic
entity export. It preserves the following decoded attributes:

- Referenced layers: name, ACI color, on/off, frozen, frozen in new viewports,
  locked, plot flag, lineweight and simple linetype.
- Entity ACI (including `0`/BYBLOCK and `256`/BYLAYER), linetype selection and
  scale, lineweight and invisibility. Simple dash/gap/dot patterns are supported.
- Drawing units (`INSUNITS`, `LUNITS`, `LUPREC`, `AUNITS`, `AUPREC`) and `LTSCALE`.
- TEXT/MTEXT content and basic text styles (font/bigfont names, height, width,
  oblique, vertical and generation flags). TEXT alignment and MTEXT line spacing
  are retained. Font files are not embedded; display still depends on installed fonts.
- Thickness/extrusion for LINE, ARC, CIRCLE, POINT and TEXT; MTEXT extrusion;
  LWPOLYLINE closure, elevation, extrusion, thickness, widths and bulges.
- Ordinary legacy 2D POLYLINE closure, continuous linetype, default/per-vertex
  widths, bulges, elevation, extrusion and thickness; ordinary 3D POLYLINE
  WCS vertices and closure. VERTEX/SEQEND ownership and links are rebuilt.
- Full/partial ELLIPSE geometry: WCS center, semi-major axis, normal, minor/major
  ratio and radian parameters. Ellipses have no thickness property.
- SOLID/TRACE stored OCS corners with shared Z, thickness and extrusion;
  3DFACE WCS corners and invisible edge flags.
- SPLINE control points, knots, rational weights, closed/periodic flags and
  tolerances; open cubic fit points, fit tolerance and endpoint tangents.
- HATCH boundary paths (polyline with bulges, or line/arc/ellipse/spline edges),
  solid or pattern fill with its definition lines, style, elevation and extrusion.
- DIMENSION definition points, text override, measurement, attachment and the
  anonymous block that holds the rendered graphics; the `Standard` DIMSTYLE.
- IMAGE placement (insertion, pixel vectors, size, display properties, brightness,
  contrast, fade, clip boundary) with its IMAGEDEF file path and pixel size.

The high-level writer chooses Windows-1252 or Windows-932 for the whole drawing,
including layer/style names. Japanese text is supported. Characters that cannot
be represented in the selected encoding are explicitly reported before replacement
in non-strict mode. Invalid table names or unresolved raw references are rejected
even in non-strict mode. Unicode/MIF escapes emitted by external legacy writers are
recognized when reading.

With the default `types=None`, `WriteResult` counts all selected non-structural
entities, including unsupported types and entities that failed to decode.
An explicit `types` filter selects root entities. An INSERT also brings its
referenced definitions (transitively) and attached attributes. Counts include
those entities once per definition, plus each attached ATTRIB; structural BLOCK,
ENDBLK and SEQEND records are not counted.

`strict=True` rejects skipped entities **and detected attribute loss** before
opening the destination. `strict=False` reports skipped geometry in
`skipped_by_type` and attribute defaults, drops or replacements in `diagnostics`.
The CLI prints these diagnostics as warnings. A written entity may still have
attribute diagnostics even when `skipped_entities == 0`:

```python
result = ezdwg.to_dwg("input.dwg", "output.dwg")
for issue in result.diagnostics:
    print(issue.handle, issue.dxftype, issue.attribute, issue.action, issue.message)
```

Examples of reported losses include TrueColor (including RGB black), complex
linetypes, MTEXT backgrounds, transparency, XDATA, extension dictionaries,
nondefault plot/material/visual-style references, extended font/style data and
unresolved attributes.
NaN/Infinity, nonpositive radii or text heights, zero direction vectors and invalid
polyline widths are rejected; non-strict high-level export skips invalid geometry.
The raw writer also validates before opening its output file and raises on invalid
input. It does not silently truncate extra polyline data or replace text bytes.

This is still a bounded export into a new drawing. Dynamic blocks, external
references, source layouts, custom objects and complete document metadata are
outside its scope. Strict mode
checks the selected entities and the decoded attributes described above; it does
not guarantee preservation of every possible DWG property or data that the reader
cannot discover. The editable model-space workflow below has the same bounded
writer scope.

For example, `types="INSERT"` with an unsupported SPLINE inside the referenced
block fails in strict mode. Non-strict export reports the skipped SPLINE while
retaining the block and INSERT; definition geometry stays in its block.

An optional external compatibility check creates empty, mixed-entity and
1,500-line and Japanese styled drawings, reads them with ODA without audit/repair, saves them back
to AC1015, and compares geometry, attributes and DXF audit results. A modern
external fixture also verifies TrueColor, transparency and XDATA loss diagnostics.
Block tests cover base points, nonuniform/reflected scale, normals, nested attributes,
independent AC1015/AC1027 input, and strict detection of unsupported block contents:

```bash
EZDWG_ODA_FILE_CONVERTER=/usr/bin/ODAFileConverter \
  uv run pytest -q tests/test_write_dwg_oda.py tests/test_write_blocks.py tests/test_write_ellipse.py tests/test_write_polyline.py tests/test_write_faces.py tests/test_write_spline.py
```

This requires a separately installed ODA File Converter, `ezdxf`, and on
headless Linux, `xvfb-run`. Normal CI runs the native regression tests and skips
the external check unless the environment variable is set. ODA acceptance is
separate from AutoCAD UI verification.

The existing raw tuple arguments remain compatible. Advanced callers can pass
keyword-only `entity_attributes` (keyed by nonzero input handle), `layers`,
`linetypes`, `text_styles` and `metadata` to `ezdwg.raw.write_ac1015_dwg`.
`metadata={"codepage": 38}` selects Windows-932; raw output defaults to codepage
30 (Windows-1252). Attributes name table entries rather than reusing source table
handles. Unknown keys, missing references and invalid values raise `ValueError`.

## Native DWG Writer (AC1032)

`AC1032` (AutoCAD 2018 format) preserves entity and layer RGB colors and uses
UTF-16 strings. Text, layer/style/linetype names, block names and descriptions
can mix languages within one drawing, including supplementary characters such
as emoji. Fonts are not embedded; glyph display depends on installed fonts.

```python
import ezdwg

drawing = ezdwg.new("AC1032")
drawing.layers["図面_도면"] = {"true_color": 0x73B9FF}
drawing.modelspace().add_text(
    "日本語 한국어 Русский العربية 😀", (0, 0), height=2,
    layer="図面_도면", true_color=0x123456,
)
drawing.save("unicode.dwg")

# Select the output version when exporting or creating an editable snapshot.
ezdwg.to_dwg("unicode.dwg", "copy.dwg", version="AC1032", strict=True)
edited = ezdwg.read("unicode.dwg").edit(version="AC1032")
edited.save("edited.dwg")
```

Supported entities are `LINE`, `POINT`, `RAY`, `XLINE`, `ARC`, `CIRCLE`,
`ELLIPSE`, `LWPOLYLINE`, `TEXT`, `MTEXT`, static `INSERT`, and single-line
`ATTDEF` / `ATTRIB`. Static blocks may be nested. Referenced layers, simple
linetypes, basic text styles, units and common formatting are preserved.
RGB values are stored as `0xRRGGBB`, without ACI approximation.

Paper-space entities, `VIEWPORT`, `SPLINE`, legacy polylines, faces, `HATCH`,
`DIMENSION`, `IMAGE`, dynamic blocks and extended metadata are outside this
AC1032 writer's scope. Unsupported entity types are reported/skipped by
non-strict exports and rejected by strict exports. A drawing containing
paper-space entities is rejected. R2018 attribute imports retain the existing
conservative diagnostic for unqualified embedded multiline properties.

Strings are limited to 65,535 UTF-16 code units per field; supplementary
characters use two units. NUL characters and overlong strings produce loss
diagnostics and fail strict saves. The legacy `codepage` selection applies to
AC1015; AC1032 uses Unicode regardless of that setting.

The AC1032 writer is native and needs no external converter or template file.
`ezdwg.new()` and exports without a version argument continue to use AC1015.
The raw version-selecting entry point is `ezdwg.raw.write_dwg(...,
version="AC1032")`; `write_ac1015_dwg` keeps its existing contract.

```bash
ezdwg write input.dwg output.dwg --dwg-version AC1032
```

## Creating and Editing a Drawing

`ezdwg.new()` returns an in-memory `Drawing` for AC1015. Its model space holds
live `Entity` objects with unique integer handles:

```python
import ezdwg

drawing = ezdwg.new()
drawing.header["insunits"] = 4  # millimeters
drawing.linetypes["DASHED"] = {"description": "Dashed", "dashes": [1.0, -0.5]}
drawing.layers["作図"] = {"color_index": 2, "linetype": "DASHED", "lineweight": 35}
drawing.text_styles["Notes"] = {"font": "txt.shx", "width": 1.0}

msp = drawing.modelspace()
line = msp.add_line((0, 0), (10, 5), layer="作図")
circle = msp.add_circle((5, 5), radius=2)
msp.add_text("日本語", insert=(1, 2), height=2, style="Notes")
msp.update(line, end=(20, 5))
line.dxf["color_index"] = 1
msp.delete(circle)
result = drawing.save("created.dwg")
```

Convenience methods cover `add_line`, `add_point`, `add_ray`, `add_xline`,
`add_arc`, `add_circle`, `add_ellipse`, `add_lwpolyline`, `add_text`, and `add_mtext`.
Legacy polyline helpers are `add_polyline2d` and `add_polyline3d`.
Face helpers are `add_solid`, `add_trace` and `add_3dface`.
Spline helpers are `add_spline` and `add_spline_fit`. `add_hatch`, the dimension
helpers (`add_linear_dim`, `add_aligned_dim`, `add_radius_dim`,
`add_diameter_dim`, `add_angular_dim`, `add_ordinate_dim`, `add_dimension`) and
`add_image` are described in their own sections below.
`add_entity("LINE", {"start": (0, 0), "end": (1, 1)}, handle=100)` accepts an
explicit handle; duplicates, zero, negatives and values outside unsigned 64-bit
range are rejected. Automatic handles are not reused after deletion.

Positions accept two or three coordinates; normals and direction vectors require
three. LWPOLYLINE vertices are XY pairs, with `elevation` stored separately; use
`closed=True`, `bulges`, `widths` and `const_width` for its other geometry.
ARC start/end angles and TEXT rotation/oblique are degrees. MTEXT uses `text`
for its raw formatting string, `char_height` for height and `text_direction` for
orientation. It has no separate plain-text field in an editable drawing.

`query("LINE CIRCLE")` (also comma-separated names or type globs), `get(handle)`
and iteration return the same live entities. `update(entity_or_handle, **attrs)`
validates geometry before changing the entity. Direct `entity.dxf` changes are
validated when saving. Unknown attribute names raise during add/update; a direct
dictionary mutation is diagnosed on save. Input dictionaries and point arrays
are copied when adding entities.

The three table dictionaries use names as keys and raw writer property dictionaries
as values (omit the `name` property). Unused definitions in a new `Drawing` are
saved too. Undefined references and invalid table definitions reject saving even
with `strict=False`. `header` supports `insunits`, `lunits`, `luprec`, `aunits`,
`auprec` and `ltscale`; text encoding is selected automatically.

For an existing file:

```python
source = ezdwg.read("created.dwg")
edited = source.edit()  # defaults to strict=True
line = next(edited.modelspace().query("LINE"))
edited.modelspace().update(line, end=(30, 10))
edited.save("edited.dwg")
```

`Document.edit()` copies model-space entities, all discovered block definitions
(including unused/empty ones), attached attributes, referenced writer tables and
supported header settings. It is independent of the source file after the
copy; it does not modify the read-only `Document` or its cached layer state.
It does not clone unused layer/linetype/style tables, custom objects or full
document metadata. Known losses, unsupported entities and discovered paper-space
content reject the copy by default. To deliberately work with a lossy subset,
use `edit(strict=False)` and inspect `drawing.diagnostics`. Import losses remain
attached to the snapshot and cause subsequent strict saves to fail. Deleting an
entity removes its entity-specific import diagnostics; document/table diagnostics
remain. Repairing an attribute does not erase an earlier import-loss diagnostic.

`Drawing.save(path, strict=True)` returns `WriteResult`. `save()` without a path
reuses the last successful destination, or the imported source path. A new drawing
requires a path for its first save. `Drawing.export_dwg` is an alias for saving;
`ezdwg.to_dwg(drawing_or_editable_layout, path, types=..., strict=True)` also works.
The existing `to_dwg` function retains its `strict=False` default. For a new
drawing, `WriteResult.source_path` is an empty string.

All native DWG writes serialize and validate before writing the destination.
They write and sync a temporary file in the destination directory, close it, then
rename it over the destination. Validation, write and replacement failures leave
the previous destination intact, and temporary files are cleaned up. Existing
file permission bits are retained; symlink and directory destinations are rejected.
A successful high-level save clears decode caches so subsequent queries read the
replacement. The save path changes only after success. This provides atomic file
replacement on supported local filesystems; it does not promise directory-fsync
durability across power loss or identical behavior on every network filesystem.

Plotting/DXF conversion continue to use read-only documents and layouts; to use
those APIs with edited geometry, save and read the resulting DWG first. Complete
layouts and newer DWG write versions remain outside this editing API's scope.


## Ellipses

`add_ellipse` is available in model space and block definitions. It preserves
ELLIPSE entities, including elliptical arcs, without tessellation.

```python
import math
import ezdwg

drawing = ezdwg.new()
msp = drawing.modelspace()
msp.add_ellipse((10, 20), major_axis=(5, 0, 0), axis_ratio=0.5)
arc = msp.add_ellipse(
    (1, 2, 3), major_axis=(2, 3, -3), axis_ratio=0.25,
    extrusion=(0, 1, 1), start_angle=math.pi / 4, end_angle=math.pi,
)
msp.update(arc, axis_ratio=0.6)
drawing.save("ellipses.dwg")
```

The center and semi-major-axis vector use WCS coordinates. The vector is relative
to the center. `axis_ratio` is the minor/major radius ratio. The existing reader
field names `start_angle` and `end_angle` denote eccentric anomaly parameters in
**radians**; they are not polar angles. A full ellipse defaults to `0` through
`math.tau`. Wrapped arcs such as `5.5` through `0.5` and negative parameters are
retained. See the [Autodesk ELLIPSE reference](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-107CB04F-AD4D-4D2F-8EC9-AC90888063AB.htm).

Geometry must be finite. The writer requires a ratio in `[1e-6, 1]`, axis and
normal lengths greater than `1e-6`, and perpendicular axis/normal directions
(normalized dot-product tolerance `1e-6`). It accepts at most one revolution;
partial arcs must have a counterclockwise parameter span greater than `1e-6`.
Full revolutions use a `1e-12` parameter tolerance. Invalid geometry is rejected
by add/update, diagnosed by strict save, or skipped by non-strict export.
Common layer/color/linetype/lineweight/visibility attributes are supported;
ELLIPSE does not store thickness.

The raw writer accepts the keyword-only `ellipses` argument, with rows matching
`raw.decode_ellipse_entities`: `(handle, center, major_axis, extrusion,
axis_ratio, start_parameter, end_parameter)`. All three vectors have three
coordinates, and both parameters are radians. Existing positional writer
arguments are unchanged.

Tests cover existing R14/R2004/R2007/R2010/R2013 inputs and ODA read/resave of
full, partial, wrapped, tilted, thin and block-contained ellipses. Independent
AC1015/AC1027 input is also imported, edited and exported to AC1015.

## Legacy Polylines

`add_polyline2d` and `add_polyline3d` create ordinary POLYLINE records with owned
VERTEX records and a SEQEND. Both work in model space and block definitions.

```python
import ezdwg

drawing = ezdwg.new()
msp = drawing.modelspace()
poly = msp.add_polyline2d(
    [(0, 0), (4, 0), (4, 3)], closed=True, flags=128,
    bulges=[0, 0.5, -0.25], widths=[(0.1, 0.2), (0.3, 0.4), (0.5, 0.6)],
    width_start=0.7, width_end=0.8, elevation=4, extrusion=(0, 1, 0), thickness=2,
)
msp.add_polyline3d([(0, 0, 1), (2, 3, 4), (5, 6, -2)], closed=False)
msp.update(poly, closed=False, elevation=7)
drawing.save("polylines.dwg")
```

2D `points` are OCS XY pairs; use `elevation` for their shared Z coordinate.
3D `points` are WCS XYZ triples. These coordinate conventions follow the
[Autodesk VERTEX reference](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-0741E831-599E-4CBF-91E1-8ADBCFD6556D.htm).
Both require at least two finite vertices. `closed` overrides the closure bit in
`flags`. 2D supports flags 1 (closed) and 128 (continuous linetype); 3D supports
closure and its implicit 3D type bit 8. Curve/spline fitting and mesh flags are
rejected. 3D polylines do not have bulges, widths, extrusion or thickness.

For 2D polylines, `bulges` and `widths` follow vertex order and may be shorter
than `points`; omitted entries are zero. Widths must be nonnegative, and extra
array entries are rejected. `width_start` / `width_end` retain the parent's
default widths. Use `closed=True` for closure; there is no need to repeat the
first point. Editable imports retain the actual stored vertex sequence, including
an explicit repeated endpoint, rather than the reader's display interpolation.

`Document.edit()` and `to_dwg()` read the owned-vertex arrays or legacy
first/last/next links, retaining order even for sparse, non-monotonic handles.
Missing, inconsistent or unsupported geometry rejects strict export; non-strict
export skips the whole polyline. A polyline counts as one entity in `WriteResult`;
its structural VERTEX/SEQEND records are not counted separately. Parent entity
handles are retained; VERTEX and SEQEND handles are regenerated on saving.

Child records use the parent layer and default BYLAYER display properties.
Individual child display overrides, XDATA/extended metadata and R2010+ vertex
IDs are diagnosed and rejected in strict mode. Fitted polylines, meshes and
polyface meshes remain outside this writer scope. Saving does not evaluate fit
curves or replace them with straight segments.

The raw writer adds keyword-only `polylines2d` and `polylines3d` lists of
dictionaries containing `handle`, `points` and `flags`; 2D rows also accept
`width_start`, `width_end`, `widths`, and `bulges`. Pass parent formatting through
`entity_attributes` and block membership through `entity_owners`. Existing
positional arguments remain compatible.

Native tests cover model/block ownership, vertex order, closure and validation.
Opt-in ODA tests check direct read/resave, tilted 2D geometry, 3D vertices,
independent AC1015/AC1018/AC1027 input, and strict child-property loss handling.

## SOLID, TRACE and 3DFACE

`add_solid`, `add_trace` and `add_3dface` accept three or four finite XYZ
corners, in both model space and blocks. A triangle is stored by repeating its
third corner as the fourth. Input order is retained, including during editing.

```python
import ezdwg

drawing = ezdwg.new()
msp = drawing.modelspace()
# SOLID/TRACE stored order: the quad perimeter visits indices 0, 1, 3, 2.
corners = [(0, 0, 5), (4, 0, 5), (0, 3, 5), (4, 3, 5)]
solid = msp.add_solid(corners, thickness=2, extrusion=(0, 0.6, 0.8))
msp.add_trace(corners[:3], extrusion=(0, 0, -1))
# 3DFACE uses perimeter order and independently varying WCS Z coordinates.
face = msp.add_3dface(
    [(0, 0, 0), (4, 0, 1), (4, 3, 2), (0, 3, -1)],
    invisible_edge_flags=5,
)
msp.update(solid, thickness=-1)
msp.update(face, invisible_edge_flags=0)
drawing.save("faces.dwg")
```

SOLID/TRACE coordinates are OCS, with **equal Z on every corner**. Their shared
elevation comes from the corners; a separate `elevation` attribute is not accepted.
Negative thickness and non-default extrusion are supported; extrusion must be
finite and nonzero. See Autodesk's [OCS conventions](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-D99F1509-E4E4-47A3-8691-92EA07DC88F5.htm).

3DFACE coordinates are WCS; thickness and extrusion are not supported. Its
`invisible_edge_flags` is an integer from 0 to 15, combining bits 1, 2, 4 and 8
for successive edges. See the [Autodesk 3DFACE reference](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-747865D5-51F0-45F2-BEFE-9572DBC5B151.htm).
Unknown flags, non-finite coordinates, wrong corner counts and unequal planar
Z values are rejected. Direct dictionary mutations receive the same checks on
save; non-strict exports skip invalid geometry and report diagnostics.

The raw writer adds keyword-only `solids`, `traces` and `faces3d` lists using the
existing decoded row shapes, always with four corners:

- SOLID/TRACE: `(handle, p1, p2, p3, p4, thickness, extrusion)`.
- 3DFACE: `(handle, p1, p2, p3, p4, invisible_edge_flags)`.

Common attributes and block owners use `entity_attributes` and `entity_owners`.
An explicitly supplied formatting attribute overrides its raw-row value; omitted
attributes retain the raw-row geometry. Existing positional arguments are unchanged.

Tests cover native edit/save and strict destination preservation, ODA direct
read/resave without repair, all 16 edge masks, triangles, tilted planar faces,
block dependencies and independent AC1015/AC1018/AC1027 import/edit/export.

## Splines

The writer retains either the native control-point representation (scenario 1)
or the fit-point representation (scenario 2). It does not tessellate, refit or
convert between them. Both work in model space and block definitions.

```python
import ezdwg

drawing = ezdwg.new()
msp = drawing.modelspace()
control = msp.add_spline(
    [(0, 0, 1), (2, 5, 2), (5, -1, 3), (8, 2, 0)],
    degree=2, knots=[0, 0, 0, 0.4, 1, 1, 1], weights=[1, 0.5, 2, 1],
)
fit = msp.add_spline_fit(
    [(0, 0, 1), (2, 4, 2), (5, -1, 3), (8, 2, 0)],
    start_tangent=(1, 0, 0), end_tangent=(0, 1, 0), fit_tolerance=1e-8,
)
msp.update(control, weights=[1, 0.7, 2, 1])
drawing.save("splines.dwg")
```

Coordinates and tangents are WCS XYZ. Property names follow the existing reader:
`scenario`, `degree`, `rational`, `closed`, `periodic`, `knots`, `control_points`,
`weights`, `fit_points`, `fit_tolerance`, `knot_tolerance`, `ctrl_tolerance`,
`start_tangent`, `end_tangent`. The reader's display `points` field is derived
and is not an editable spline input. See the [Autodesk SPLINE reference](https://help.autodesk.com/cloudhelp/2023/ENU/AutoCAD-DXF/files/GUID-E1F884F8-AA90-4864-A215-3182D47A9C74.htm).

Control splines support degrees 1 through 10, with more control points than the
degree. Knot count must equal `len(control_points) + degree + 1`. Knots must be
finite and nondecreasing, with a nonempty parameter domain and multiplicity no
greater than `degree + 1`. Rational splines require one finite positive weight
per control point. `add_spline` infers `rational` from supplied weights and
generates clamped uniform knots when `knots` is omitted. Direct raw/entity rows
require explicit knots and a consistent `rational` flag.

Closed control splines accept clamped knots with matching endpoint control points,
or periodic wrapped data. Wrapped data repeats the first `degree` control points
and weights at the end, with repeating knot intervals. `periodic=True` requires
`closed=True`. Pass explicit knots for a wrapped periodic curve. Native saves
retain the supplied data. ODA can clamp an equivalent periodic curve and adjust
closure flags when exporting DXF, so external tests compare evaluated curve
positions as well as attributes.

Fit splines currently support open cubic curves with at least two points,
distinct adjacent points, chord parameterization and optional endpoint tangents.
An omitted tangent is stored as a zero vector. Tolerances are finite and
nonnegative; defaults are `1e-7` for knot/control and `1e-10` for fit tolerance.
An entity cannot combine control data with fit data. Closed fit splines, other
fit degrees, non-chord R2013+ fit parameterization, and spline thickness/extrusion
are outside the writer scope. Invalid/unrepresentable geometry is skipped with
diagnostics in non-strict exports, and rejected before output replacement in strict
mode. Unpreserved control-frame display metadata or XDATA also rejects strict saves.

`raw.write_ac1015_dwg(..., splines=[...])` accepts keyword-only dictionaries
with `handle` and the same geometry fields. Common formatting and ownership use
`entity_attributes` and `entity_owners`. `raw.decode_write_splines(path)` exposes
endpoint tangents and R2013+ interpretation fields for import; the existing
`decode_spline_entities` tuple contract is unchanged. `Document.edit()` imports
the extra data automatically and keeps it after the source file is removed.

Tests use existing AC1014 through AC1032 samples (including strict loss rejection),
independent AC1015/AC1018/AC1027 inputs and ODA direct read/resave. These are local
and ODA checks; they do not establish native AutoCAD UI behavior.

## True colours

AC1015 stores colours as ACI only. An entity or layer `true_color`
(`0xRRGGBB`) is accepted by the editable API and by `Document.edit()`; on save
it is rounded to the nearest of the 255 palette entries (pure black and white
map to 7, the foreground colour) and reported as a `WriteDiagnostic` with action
`approximated`. Approximations do not fail strict saves or strict imports; the
drawing keeps the requested `true_color` so a later writer can use it.

```python
drawing.layers["塗"] = {"true_color": 0x73B9FF}
msp.add_line((0, 0), (10, 0), true_color=0xFF0000)  # written as ACI 1
```

`ezdwg._aci.nearest_aci(rgb)` and `aci_palette()` expose the palette.

## Hatches

```python
msp.add_hatch([[(0, 0), (10, 0), (10, 5), (0, 5)]], color_index=1)  # solid fill
msp.add_hatch(
    [{"edges": [
        {"type": "line", "start": (0, 0), "end": (4, 0)},
        {"type": "arc", "center": (4, 2), "radius": 2, "start_angle": -90, "end_angle": 90},
        {"type": "line", "start": (4, 4), "end": (0, 4)},
        {"type": "line", "start": (0, 4), "end": (0, 0)},
    ]}],
    pattern="ANSI31", pattern_angle=30, pattern_scale=25.4,
)
```

A path is a point list (a closed polyline), a dictionary with `points`
(optional `bulges`, `closed`, `flags`), or a dictionary with `edges` (line, arc,
ellipse with `major_axis`/`ratio`, or spline with control points and knots;
angles in degrees). Without `pattern` the hatch is a solid fill. `pattern` is a
name of the built-in definitions (`ANSI31`, `ANSI32`, `ANSI33`, `ANSI37`, `DOTS`,
`LINE`, `NET`, in inches) or `acad.pat` rows `(angle, base_x, base_y, dx, dy,
dashes)`; it is rotated by `pattern_angle`, scaled by `pattern_scale` and stored
as `pattern_lines` (offsets in the OCS orientation, as AutoCAD stores them).
`hatch_style` (0 odd parity, 1 outermost, 2 whole area), `elevation`,
`extrusion` and `pixel_size`/`seed_points` are accepted. The writer stores
non-associative hatches. The reader returns tessellated path points; an edited
import keeps those as polyline paths.

## Dimensions

AutoCAD does not regenerate dimension graphics when it opens a drawing, so each
DIMENSION references an anonymous block (`*D1`, ...) with its lines, arrowheads
(filled `SOLID`s) and measurement `TEXT`. The helpers render that block with the
`Standard` DIMSTYLE the writer stores (ISO-25 like: arrow 2.5, text 2.5, gap
0.625, extension offset 0.625 / extension 1.25, two decimals, `.` separator):

```python
msp.add_linear_dim((0, 10), (0, 0), (50, 0))            # base on the dimension line
msp.add_aligned_dim((0, 20), (30, 40), distance=5)
msp.add_radius_dim((80, 10), radius=12, angle=30)
msp.add_diameter_dim((80, 40), radius=10)
msp.add_angular_dim((120, 0), (140, 0), (120, 20), radius=15)
msp.add_ordinate_dim((150, 10), (150, 25))              # X datum, origin (0, 0)
msp.add_dimension(                                      # two-line angular
    "angular2ln",
    defpoint2=(0, 100), defpoint3=(40, 100),            # first line
    defpoint4=(0, 100), defpoint=(0, 140),              # second line
    defpoint5=(20, 120),                                # point on the arc
)
```

`text` overrides the measurement (`"<>"` inserts it, `" "` suppresses it).
`update()` of a rendered dimension renders its block again; pass
`geometry="*D9"` to use a block you built yourself (nothing is rendered then).
Definition points follow the DXF names `defpoint` (10), `defpoint2` (13),
`defpoint3` (14), `defpoint4` (15), `defpoint5` (16); `angle`, `oblique_angle`,
`text_rotation` and `horizontal_direction` are degrees. Only the `Standard`
DIMSTYLE can be referenced. `Document.edit()` imports the dimensions of a file
with their existing blocks; `to_dwg()` carries them through a re-export.

## Raster images

```python
drawing.image_defs["logo"] = {"file_path": "images/logo.png", "size": (640, 480)}
msp.add_image("logo", (10, 20), width=64)             # keeps the pixel aspect ratio
msp.add_image("logo", (100, 20), width=32, height=32, rotation=90, clipping=True)
```

`image_defs` maps a name to `file_path`, `size` (pixels), optional
`pixel_size` (drawing units per pixel, default 1), `resolution_units` (0 none,
2 centimeters, 5 inches) and `is_loaded`. `add_image` derives the pixel step
vectors (`u_vector`, `v_vector`) from `width`/`height`/`rotation`, or takes them
explicitly. `display_props`, `brightness`, `contrast`, `fade`, `clipping` and a
rectangular (`clip_type=1`, two corners) or polygonal (`clip_type=2`) clip
boundary in pixel coordinates are stored. The writer adds the `IMAGE`,
`IMAGEDEF`, `IMAGEDEF_REACTOR` and `RASTERVARIABLES` classes, the
`ACAD_IMAGE_DICT`/`ACAD_IMAGE_VARS` dictionary entries and one reactor per
image. The image file itself is not embedded; the path is stored as given. The
reader exposes `IMAGE` entities with `file_path` and `image_def` (the file stem)
resolved from their IMAGEDEF, and `Document.edit()` recreates `image_defs`.

## Blocks and Attributes

`drawing.blocks` is a case-insensitive `BlockTable`. `new()` creates a
`BlockLayout` supporting the same query/add/update/delete methods as model space.
Its `base_point` and `description` are editable. Entity handles are unique across
model space, all blocks and all attached attributes.

```python
import ezdwg

drawing = ezdwg.new()
part = drawing.blocks.new("Part", base_point=(1, 2, 0), description="Component")
part.add_line((1, 2), (5, 2), color_index=0)  # BYBLOCK
part.add_attdef("ID", "default", insert=(1, 3), height=1, prompt="Identifier")
part.add_attdef("TYPE", "fixed", insert=(1, 4), height=1, attribute_flags=2)
assembly = drawing.blocks.new("Assembly")
nested = assembly.add_blockref("Part", insert=(3, 4), scale=(-2, 3, 1), rotation=35)
value = assembly.add_attrib(nested, "ID", "P-001", insert=(3, 5), height=1.5)
assembly.update(value, text="P-002")
drawing.modelspace().add_blockref("Assembly", insert=(100, 200), rotation=90)
drawing.save("assembly.dwg")
```

- `add_blockref(name, insert=(0, 0, 0), *, scale=(1, 1, 1), rotation=0, **attrs)`
  stores an INSERT, without exploding the definition. Scale components must be
  finite and nonzero; negative/nonuniform scales and arbitrary nonzero extrusion
  normals are retained. After creation, edit `xscale`, `yscale`, `zscale` or
  `rotation` in the entity's `dxf` dictionary or through `update()`.
- `add_attdef(tag, text="", insert=(0, 0, 0), height=1, *, prompt="", **attrs)`
  stores a single-line attribute definition with TEXT placement/style properties.
- `add_attrib(blockref, tag, text, insert=(0, 0, 0), height=1, **attrs)` attaches a
  single-line attribute to that INSERT. `attribs(blockref)` returns the live
  attribute entities as a tuple. The parent layout's `get`, `update` and `delete`
  can address attached attributes by entity or handle.
- `attribute_flags` retains the standard bits: 1 invisible, 2 constant,
  4 verify, 8 preset. Tags must be nonempty without whitespace or `!`; duplicate
  tags on one INSERT are rejected. Geometry, color, layer, text style, alignment
  and rotation follow the TEXT conventions.

Attribute positions are explicit in the containing layout's coordinate system,
using the same OCS/extrusion convention as TEXT. They are not block-local ATTDEF
positions to be automatically transformed by `add_attrib`. This API does not
instantiate defaults, transform attribute placements, or synchronize existing
ATTRIB values when an ATTDEF changes. Constant definitions remain ATTDEF records.

`delete(insert)` also removes its attached attributes. `drawing.blocks.delete(name)`
refuses a referenced block; delete its references first. Empty definitions are
valid. `Drawing.save()` preserves all in-memory definitions. Exporting a layout
or using a root type filter includes the referenced block closure; exporting a
`BlockLayout` itself writes its selected local geometry into output model space.

Undefined block names, duplicate handles, cyclic block references, invalid
ownership and duplicate attribute tags reject saving even with `strict=False`.
Invalid INSERT geometry can be skipped in non-strict mode, along with its owned
ATTRIBs, with diagnostics for both. Unsupported entities inside referenced blocks
are included in strict validation and skipped counts, even with `types="INSERT"`.

The writer emits BLOCK_CONTROL/BLOCK_HEADER, BLOCK/ENDBLK, block member links,
INSERT references and reverse references, ATTRIB ownership and SEQEND records.
Reading preserves INSERT ACI and extrusion; nested attributes inherit their
INSERT's layout membership instead of appearing as loose model-space geometry.
Editable imports preserve attached-attribute order, including sparse handles,
and diagnose missing or inconsistent owned-attribute references.

Static blocks are supported, including anonymous names such as `*U1`. MINSERT
arrays, dynamic block behavior, xrefs, multiline attributes, lock-position data,
per-block units/scale restrictions, block previews and extended block metadata
are outside this scope. Detected import losses remain diagnostics; R2018
attribute imports are conservatively diagnosed because embedded multiline
properties have not been qualified for AC1015 export. ODA tests establish
read/resave compatibility, not AutoCAD UI or Windows execution coverage.

For raw writers, existing positional arguments are unchanged. Additional keyword
arguments are `blocks` (name/base_point/description dictionaries), `inserts`,
`attdefs`, `attribs`, and `entity_owners`. Owner keys are entity handles; values are
block names for block contents or INSERT handles for attached ATTRIBs. New raw
entity rows use dictionaries; INSERT `scale` is a three-tuple and raw `rotation`
is radians, as in the older raw TEXT rows. Shared formatting still goes in
`entity_attributes`. A zero/omitted handle cannot be referenced as an owner.
