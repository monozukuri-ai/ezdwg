# Core Functions

## ezdwg.read

```python
ezdwg.read(path: str) -> Document
```

Open a DWG file and return a `Document`.

**Parameters:**

- `path` — Path to the DWG file.

**Returns:** A [`Document`](document.md) object.

**Raises:** `ValueError` if the DWG version is unsupported.

**Example:**

```python
import ezdwg

doc = ezdwg.read("drawing.dwg")
print(doc.version)
```

---

## ezdwg.clear_decode_caches

```python
ezdwg.clear_decode_caches() -> None
```

Release module-level DWG decode caches. A long-running batch converter can call
this after it has finished using a `Document` so decoded tables and entity
metadata from earlier files are not retained.

```python
for path in drawing_paths:
    try:
        process(ezdwg.read(path))
    finally:
        ezdwg.clear_decode_caches()
```

---

## ezdwg.plot

```python
ezdwg.plot(
    target: str | Document | Layout,
    types: str | Iterable[str] | None = None,
    ax: Axes | None = None,
    show: bool = True,
    equal: bool = True,
    title: str | None = None,
    line_width: float = 0.5,
    arc_segments: int = 64,
    auto_fit: bool = True,
    fit_margin: float = 0.04,
    dimension_color: Any | None = "black",
) -> Axes
```

Plot DWG entities using matplotlib.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `target` | `str \| Document \| Layout` | — | File path, Document, or Layout |
| `types` | `str \| Iterable[str] \| None` | `None` | Entity type filter |
| `ax` | `Axes \| None` | `None` | Existing matplotlib axes |
| `show` | `bool` | `True` | Call `plt.show()` |
| `equal` | `bool` | `True` | Equal aspect ratio |
| `title` | `str \| None` | `None` | Plot title |
| `line_width` | `float` | `0.5` | Geometry stroke width in points; dimensions use thinner strokes |
| `arc_segments` | `int` | `64` | Segments for arcs |
| `auto_fit` | `bool` | `True` | Auto-fit view bounds |
| `fit_margin` | `float` | `0.04` | Margin fraction |
| `dimension_color` | `Any \| None` | `"black"` | Dimension color |

**Returns:** The matplotlib `Axes` object.

**Example:**

```python
import ezdwg

ezdwg.plot("drawing.dwg", types="LINE ARC", title="My Drawing")
```

---

## ezdwg.to_dxf

```python
ezdwg.to_dxf(
    source: str | Document | Layout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
    include_unsupported: bool = False,
) -> ConvertResult
```

Convert a DWG file to DXF format.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | `str \| Document \| Layout` | — | File path, Document, or Layout |
| `output_path` | `str` | — | Output DXF file path |
| `types` | `str \| Iterable[str] \| None` | `None` | Entity type filter |
| `dxf_version` | `str` | `"R2010"` | DXF version string |
| `strict` | `bool` | `False` | Fail on skipped entities |
| `include_unsupported` | `bool` | `False` | Query unsupported types |

**Returns:** A `ConvertResult` object.

**Raises:** `ValueError` in strict mode if entities are skipped. `ImportError` if ezdxf is not installed.

---

## ezdwg.new / Document.edit

```python
ezdwg.new(version: str = "AC1015") -> Drawing
doc.edit(*, strict: bool = True) -> Drawing
```

Create an empty drawing or copy the supported model-space and block subset of a read-only
`Document`. An editable snapshot no longer needs the source file. Import losses
raise by default; `strict=False` retains them in `Drawing.diagnostics`.

## Drawing / EditableLayout

| API | Behavior |
|-----|----------|
| `drawing.modelspace()` | Persistent `EditableLayout` |
| `drawing.blocks.new(name, *, base_point=(0, 0, 0), description="")` | Create a `BlockLayout` |
| `drawing.blocks[name]` / `.delete(name)` | Access case-insensitively or remove an unreferenced block |
| `drawing.layers`, `.linetypes`, `.text_styles` | Mutable dictionaries of name to writer properties |
| `drawing.header` | Mutable units, precision and LTSCALE settings |
| `drawing.diagnostics` | Remaining import losses as a tuple of `WriteDiagnostic` |
| `drawing.save(path=None, *, strict=True)` | Validate and atomically replace a DWG; returns `WriteResult` |
| `drawing.export_dwg(path, *, strict=True)` | Save to a specified destination |
| `drawing.path` | Last successful save destination, initially `None` |
| `drawing.source_path` | Import source, or `None` for a new drawing |
| `layout.query(types=None)` / `iter(layout)` | Live entities in insertion order |
| `layout.get(handle)` | Live entity; raises `KeyError` if absent |
| `layout.add_entity(dxftype, dxf, *, handle=None)` | Copy attributes, validate geometry, allocate or check a handle |
| `layout.update(entity_or_handle, **attributes)` | Validate and change an entity in place |
| `layout.delete(entity_or_handle)` | Remove an entity without reusing its handle |
| `layout.add_line/point/ray/xline/arc/circle/ellipse/lwpolyline/text/mtext(...)` | Geometry-specific creation helpers |
| `layout.add_polyline2d(points, **attrs)` | Ordinary legacy 2D polyline; XY vertices with separate elevation |
| `layout.add_polyline3d(points, **attrs)` | Ordinary legacy 3D polyline; WCS XYZ vertices |
| `layout.add_spline(control_points, degree=3, *, knots=None, weights=None, **attrs)` | WCS control spline; omitted knots generate a clamped uniform vector |
| `layout.add_spline_fit(fit_points, **attrs)` | Open cubic fit spline with optional start/end tangents |
| `layout.add_solid/trace(points, **attrs)` | Three/four OCS XYZ corners in stored order with shared Z |
| `layout.add_3dface(points, **attrs)` | Three/four WCS XYZ corners in perimeter order; `invisible_edge_flags` 0..15 |
| `layout.add_blockref(name, insert=..., *, scale=..., rotation=0, **attrs)` | Add an INSERT |
| `layout.add_attdef(tag, text="", insert=..., height=1, *, prompt="", **attrs)` | Define an attribute |
| `layout.add_attrib(blockref, tag, text, insert=..., height=1, **attrs)` | Attach an attribute |
| `layout.attribs(blockref)` | Tuple of live attached attributes |

`layout.add_ellipse(center, major_axis=(1, 0, 0), axis_ratio=1.0,
start_angle=0.0, end_angle=math.tau, **attrs)` uses WCS center/axis coordinates
and eccentric anomaly parameters in **radians**, matching the existing ELLIPSE
reader fields. See [ellipse geometry and validation](../guide/convert.md#ellipses).

`Entity` identity fields remain immutable; its `dxf` dictionary is mutable.
Only entities obtained from the editable layout participate in its saved state.
Saving without a path uses the last successful destination or imported source;
new drawings require a path. Invalid geometry or known loss raises `ValueError`
in strict mode; filesystem failures raise `OSError` and preserve the destination.

See the [editing guide](../guide/convert.md#creating-and-editing-a-drawing) for
examples, table fields, coordinate conventions, and snapshot/save limitations.

---

## ezdwg.to_dwg

```python
ezdwg.to_dwg(
    source: str | Document | Layout | Drawing | EditableLayout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    version: str = "AC1015",
    strict: bool = False,
) -> WriteResult
```

Write a DWG file using the native writer. A path or `Document` exports model
space; an explicit `Layout` selects its entities for the output model space.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | `str \| Document \| Layout \| Drawing \| EditableLayout` | — | Source file, read-only document/layout, or editable drawing/layout |
| `output_path` | `str` | — | Output DWG file path |
| `types` | `str \| Iterable[str] \| None` | `None` | Entity type filter |
| `version` | `str` | `"AC1015"` | Output DWG version (currently AC1015 only) |
| `strict` | `bool` | `False` | Fail on skipped entities or detected attribute loss |

**Returns:** A `WriteResult` object.

**Raises:** `ValueError` when an unsupported write version is requested, or in strict mode if entities are skipped or attribute loss is detected.

Counts include unsupported and undecoded entities in the selected scope.
Type filters select roots; referenced blocks and attached attributes are included
transitively. `strict` also validates entity coverage and known attribute losses inside those dependencies; see the [writer limitations](../guide/convert.md#native-dwg-writer-ac1015).

**Supported native write entities:**

- `LINE`
- `RAY`
- `XLINE`
- `POINT`
- `ARC`
- `CIRCLE`
- `ELLIPSE` (full ellipses and elliptical arcs; parameters in radians)
- `SPLINE` (control/NURBS and open cubic fit forms; see [scope](../guide/convert.md#splines))
- `SOLID` / `TRACE` (planar OCS corners, thickness and extrusion)
- `3DFACE` (WCS corners and invisible edge flags)
- `LWPOLYLINE`
- Ordinary `POLYLINE_2D` and `POLYLINE_3D`, with owned VERTEX/SEQEND records
- `TEXT`
- `MTEXT`
- `INSERT` with nested block definitions
- Single-line `ATTDEF` and attached `ATTRIB`

---

## ConvertResult

```python
@dataclass(frozen=True)
class ConvertResult:
    source_path: str
    output_path: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]
```

Result of a DWG to DXF conversion.

| Attribute | Type | Description |
|-----------|------|-------------|
| `source_path` | `str` | Input DWG path |
| `output_path` | `str` | Output DXF path |
| `total_entities` | `int` | Total entities processed |
| `written_entities` | `int` | Successfully written |
| `skipped_entities` | `int` | Skipped count |
| `skipped_by_type` | `dict[str, int]` | Skipped by entity type |

---

## WriteResult

```python
@dataclass(frozen=True)
class WriteResult:
    source_path: str
    output_path: str
    target_version: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]
    diagnostics: tuple[WriteDiagnostic, ...] = ()
```

Result of a DWG write operation.

| Attribute | Type | Description |
|-----------|------|-------------|
| `source_path` | `str` | Input DWG path; empty for a new drawing |
| `output_path` | `str` | Output DWG path |
| `target_version` | `str` | Target DWG version |
| `total_entities` | `int` | Total entities processed |
| `written_entities` | `int` | Successfully written |
| `skipped_entities` | `int` | Skipped count |
| `skipped_by_type` | `dict[str, int]` | Skipped by entity type |

### WriteDiagnostic

Each entry in `WriteResult.diagnostics` has `handle` (`int | None`), `dxftype`,
`attribute`, `message` and `action`. Actions are `skipped`, `defaulted`, `dropped`
or `replaced`. A missing handle denotes a document/table-level issue. Diagnostics
are separate from entity counts: an entity written with downgraded attributes is
included in `written_entities`. See the [writer guide](../guide/convert.md#native-dwg-writer-ac1015)
for preserved properties and strict-mode boundaries.
