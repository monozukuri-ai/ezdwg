# Working with Entities

## Entity Structure

Each entity is represented as a frozen dataclass with three fields:

```python
from ezdwg import Entity

# Entity fields:
entity.dxftype  # str — entity type name (e.g. "LINE", "ARC")
entity.handle   # int — unique handle within the file
entity.dxf      # dict[str, Any] — entity-specific attributes
```

## Querying Entities

Use `query()` on a `Layout` to iterate over entities:

```python
msp = doc.modelspace()

# All supported entity types
for entity in msp.query():
    print(entity.dxftype, entity.handle)

# Filter by type name(s)
for entity in msp.query("LINE"):
    print(entity.dxf)

# Multiple types (space-separated)
for entity in msp.query("LINE ARC CIRCLE"):
    print(entity.dxftype, entity.dxf)
```

`iter_entities()` is an alias for `query()`:

```python
for entity in msp.iter_entities("LINE"):
    print(entity.dxf)
```

## Converting to Points

The `to_points()` method extracts key coordinates from an entity:

```python
for entity in msp.query("LINE LWPOLYLINE POINT INSERT HATCH SPLINE"):
    points = entity.to_points()
    print(entity.dxftype, points)
```

Supported types for `to_points()`:

| Type | Returns |
|------|---------|
| LINE | `[start, end]` |
| LWPOLYLINE | List of vertex points |
| POINT | `[location]` |
| TEXT / MTEXT | `[insert]` |
| INSERT / MINSERT | `[insert]` |
| HATCH | All boundary points, flattened in path order |
| SPLINE | Fit points when available, otherwise control points |
| DIMENSION | `[defpoint2, defpoint3]` or `[text_midpoint]` |
| RAY | `[start, start + unit_vector]` |
| XLINE | `[start - unit_vector, start + unit_vector]` |

## Linetype

Every entity carries its linetype next to its color and layer:

| Key | Type | Description |
|-----|------|-------------|
| `linetype` | `str \| None` | `"BYLAYER"`, `"BYBLOCK"`, `"CONTINUOUS"` or the name of an entry of `Document.linetypes()`. `None` when the entity names a linetype that cannot be read |
| `linetype_handle` | `int \| None` | Handle of the linetype record when the entity names one |
| `linetype_scale` | `float` | The entity's own linetype scale (DXF group 48) |
| `layer_handle` | `int` | Handle of the entity's layer; `Document.layers()` gives the layer's linetype for `BYLAYER` |

## Lineweight and Visibility

| Key | Type | Description |
|-----|------|-------------|
| `lineweight` | `int \| None` | Hundredths of a millimetre, as DXF group 370: `-1` BYLAYER (see `Document.layers()`), `-2` BYBLOCK, `-3` the default lineweight. `None` for R13/R14 files, which have no lineweights |
| `invisible` | `bool` | The invisibility flag of the entity (DXF group 60). A layer that is off or frozen hides its entities without setting it |

## Entity Type Reference

### LINE

| Key | Type | Description |
|-----|------|-------------|
| `start` | `(float, float, float)` | Start point |
| `end` | `(float, float, float)` | End point |

### ARC

| Key | Type | Description |
|-----|------|-------------|
| `center` | `(float, float, float)` | Center point |
| `radius` | `float` | Radius |
| `start_angle` | `float` | Start angle in degrees |
| `end_angle` | `float` | End angle in degrees |

### CIRCLE

| Key | Type | Description |
|-----|------|-------------|
| `center` | `(float, float, float)` | Center point |
| `radius` | `float` | Radius |

### LWPOLYLINE

| Key | Type | Description |
|-----|------|-------------|
| `points` | `list[(float, float, float)]` | Vertex points |
| `closed` | `bool` | Whether the polyline is closed |
| `const_width` | `float \| None` | Constant width |
| `bulges` | `list[float] \| None` | Bulge values per vertex |
| `widths` | `list[(float, float)] \| None` | Start/end widths per vertex |

### POINT

| Key | Type | Description |
|-----|------|-------------|
| `location` | `(float, float, float)` | Point location |

### ELLIPSE

| Key | Type | Description |
|-----|------|-------------|
| `center` | `(float, float, float)` | Center point |
| `major_axis` | `(float, float, float)` | Major axis endpoint relative to center |
| `axis_ratio` | `float` | Ratio of minor to major axis |
| `start_angle` | `float` | Start parameter (radians) |
| `end_angle` | `float` | End parameter (radians) |

### TEXT

| Key | Type | Description |
|-----|------|-------------|
| `insert` | `(float, float, float)` | Insertion point: the left end of the baseline |
| `align_point` | `(float, float, float) \| None` | Alignment point; `None` when the file stores none |
| `halign` | `int` | Horizontal justification (DXF group 72): 0 left, 1 center, 2 right, 3 aligned, 4 middle, 5 fit |
| `valign` | `int` | Vertical justification (DXF group 73): 0 baseline, 1 bottom, 2 middle, 3 top |
| `text` | `str` | Text content |
| `height` | `float` | Text height |
| `rotation` | `float` | Rotation angle in degrees |
| `width` | `float` | Width factor |
| `oblique` | `float` | Oblique angle in degrees |

A text with a justification other than left / baseline is anchored at
`align_point`; `insert` is then the left end of the baseline that follows from
it. "Aligned" and "fit" texts run from `insert` to `align_point`.

### ATTRIB / ATTDEF

An `ATTRIB` is the text of one attribute of a block reference, and an `ATTDEF`
the definition inside the block that it was created from. Both carry the keys
of `TEXT` and:

| Key | Type | Description |
|-----|------|-------------|
| `tag` | `str` | Attribute tag |
| `text` | `str` | Value (`ATTRIB`) or default value (`ATTDEF`) |
| `prompt` | `str \| None` | Prompt of an `ATTDEF`; `None` for `ATTRIB` |
| `attribute_flags` | `int` | DXF group 70: 1 invisible, 2 constant, 4 verify, 8 preset |
| `lock_position` | `bool` | R2007 and later |

`Document.entity_placement(handle)` returns `(0, insert_handle)` for an
`ATTRIB`: the owner is its `INSERT`. An `ATTRIB` is drawn where it is stored,
in the coordinate system of that `INSERT` itself, not in the one of the block.

A multi-line attribute is one `ATTRIB` in R2018 files: `text` holds its lines
separated by line feeds, without formatting codes, and the alignment point is
the top-left corner of the text. Earlier versions store one single-line
`ATTRIB` per line, with tags such as `TAG_001`.

### MTEXT

| Key | Type | Description |
|-----|------|-------------|
| `insert` | `(float, float, float)` | Insertion point |
| `text` | `str` | Text content |
| `char_height` | `float` | Character height |
| `width` | `float` | Reference rectangle width |
| `attachment_point` | `int` | Attachment point code |

### INSERT / MINSERT

`INSERT` exposes a block reference without expanding the referenced block geometry.

| Key | Type | Description |
|-----|------|-------------|
| `name` | `str` | Referenced block name (present when resolved) |
| `insert` | `(float, float, float)` | Insertion point |
| `xscale` | `float` | X-axis scale factor |
| `yscale` | `float` | Y-axis scale factor |
| `zscale` | `float` | Z-axis scale factor |
| `rotation` | `float` | Rotation angle in degrees |
| `owner_handle` | `int` | Owning block or layout handle (present when resolved) |

`MINSERT` additionally exposes `column_count`, `row_count`,
`column_spacing`, and `row_spacing`.

### HATCH

| Key | Type | Description |
|-----|------|-------------|
| `pattern_name` | `str` | Hatch pattern name |
| `solid_fill` | `bool` | Whether the hatch is a solid fill |
| `associative` | `bool` | Whether the boundary is associative |
| `elevation` | `float` | Boundary elevation |
| `extrusion` | `(float, float, float)` | Extrusion vector |
| `paths` | `list[dict]` | Boundary paths with `closed` and 3D `points` fields |
| `pattern_angle` | `float` | Pattern angle in degrees (pattern fills only) |
| `pattern_scale` | `float` | Pattern scale or spacing (pattern fills only) |
| `pattern_double` | `bool` | Double hatch flag (pattern fills only) |
| `pattern_lines` | `list[dict]` | Pattern definition (pattern fills only), see below |

For closed paths, the high-level API repeats the first point at the end of the
path. `to_points()` concatenates every path in source order.

Each item of `pattern_lines` is one family of parallel lines with `angle`
(degrees), `base`, `offset` and `dashes`. The values are stored already rotated
and scaled: line `k` of the family runs through `base + k * offset` at `angle`,
and `dashes` is its dash pattern (positive = dash, negative = gap, empty =
continuous). The pattern keys are absent for solid fills and for hatches whose
definition cannot be read.

### SPLINE

| Key | Type | Description |
|-----|------|-------------|
| `degree` | `int` | Spline degree |
| `rational` | `bool` | Whether rational weights are used |
| `closed` | `bool` | Whether the spline is closed |
| `periodic` | `bool` | Whether the spline is periodic |
| `knots` | `list[float]` | Knot values |
| `control_points` | `list[(float, float, float)]` | Control points |
| `weights` | `list[float]` | Control-point weights |
| `fit_points` | `list[(float, float, float)]` | Fit points |
| `points` | `list[(float, float, float)]` | Fit points when available, otherwise control points |

For a closed spline, `points` repeats the first point at the end.

### DIMENSION

The `dxf` dictionary for DIMENSION entities includes:

| Key | Type | Description |
|-----|------|-------------|
| `dimtype` | `str` | Subtype: `LINEAR`, `RADIUS`, `DIAMETER`, `ALIGNED`, `ORDINATE`, `ANG3PT`, `ANG2LN` |
| `text_midpoint` | `(float, float, float)` | Dimension text midpoint |
| `defpoint` | `(float, float, float)` | Definition point (dimension line) |
| `defpoint2` | `(float, float, float)` | First extension line origin |
| `defpoint3` | `(float, float, float)` | Second extension line origin |
| `defpoint4` | `(float, float, float)` | Only `ANG3PT` / `ANG2LN` (DXF code 15: angle vertex / second line start) and `RADIUS` / `DIAMETER` (point on the arc / opposite diameter point) |
| `defpoint5` | `(float, float, float)` | Only `ANG2LN` (DXF code 16: point on the dimension arc) |
| `text` | `str` | Override text |
| `angle` | `float` | Rotation angle in degrees |
| `actual_measurement` | `float` | Computed measurement value |
| `dimstyle_handle` | `int \| None` | Handle of the dimension style (`DIMSTYLE`) |
| `anonymous_block_handle` | `int \| None` | Handle of the anonymous block (`BLOCK_HEADER`) that holds the saved graphics of the dimension; `None` when the dimension has no block |
| `anonymous_block_name` | `str` | Name of that block (`*D...`); omitted when there is no block or its name is unknown |

R13/R14 files store no measurement: `actual_measurement` is `None` there.

### TOLERANCE

| Key | Type | Description |
|-----|------|-------------|
| `insert` | `(float, float, float)` | Insertion point |
| `text` | `str` | Content of the feature control frame, with its formatting codes |
| `x_direction` | `(float, float, float)` | Direction of the frame |
| `rotation` | `float` | Angle of `x_direction` in degrees |
| `height` | `float` | Text height: the one stored with the entity (R13/R14 only), otherwise the text height of the dimension style times its overall scale (`Document.dimstyles()`). `0.0` when neither is known |
| `dimgap` | `float` | Gap as R13/R14 store it; `0.0` for later versions |
| `dimstyle_handle` | `int \| None` | Handle of the dimension style |
| `char_height` | `float` | Optional saved text height from the referenced anonymous block; omitted when unavailable or ambiguous |
| `char_height_source` | `str` | `"anonymous_block"` when `char_height` was resolved from saved block text |

### VIEWPORT

A paper-space viewport: a window on a sheet that shows model space.

| Key | Type | Description |
|-----|------|-------------|
| `center` | `(float, float, float)` | Center of the window on the sheet |
| `width`, `height` | `float` | Size of the window |
| `view_center` | `(float, float)` | Center of the view in display coordinates (DXF groups 12, 22) |
| `view_height` | `float` | Height of the view in model units; `height / view_height` is the scale of the view |
| `view_target` | `(float, float, float)` | View target point |
| `view_direction` | `(float, float, float)` | View direction; `(0, 0, 1)` for a plan view |
| `view_twist_angle` | `float` | View twist in degrees |
| `status_flags` | `int` | DXF group 90: `0x20000` the viewport is off, `0x10000` it has a clip boundary, `0x1` perspective |
| `frozen_layers` | `list[str]` | Names of the layers that are frozen in this viewport |
| `frozen_layer_handles` | `list[int]` | Their handles |
| `clip_boundary_handle` | `int` | Entity that clips the viewport; omitted when there is none |
| `lens_length`, `front_clip_z`, `back_clip_z`, `render_mode` | | The remaining view settings |

The view keys are missing for R13/R14 files, which keep the view in the
extended data of the entity. `center`, `width` and `height` are missing when
the body of the viewport cannot be read.

Every sheet has one viewport that stands for the sheet itself: the first of
`Document.layouts()[name]["viewport_handles"]`. Its view shows the sheet at
scale 1.

!!! note "ARC Angles"
    The high-level API returns ARC angles in **degrees**. The raw API (`ezdwg.raw`) returns angles in **radians**.
