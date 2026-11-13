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

### IMAGE

| Key | Type | Description |
|-----|------|-------------|
| `insert` | `(float, float, float)` | Insertion point (lower-left corner) |
| `u_vector` / `v_vector` | `(float, float, float)` | Step of one pixel along the image columns / rows |
| `size` | `(float, float)` | Image size in pixels |
| `display_props` | `int` | Display flags (1 show, 2 show when not aligned, 4 use clipping, 8 transparency) |
| `clipping` | `bool` | Whether the clip boundary is enabled |
| `brightness` / `contrast` / `fade` | `int` | 0..100 |
| `clip_type` | `int` | 1 rectangle (two corners), 2 polygon |
| `clip_vertices` | `list[(float, float)]` | Clip boundary in pixel coordinates |
| `image_def_handle` | `int` | Handle of the IMAGEDEF object |
| `file_path` | `str \| None` | Image file path stored in the IMAGEDEF |
| `image_def` | `str \| None` | Definition name used by `Document.edit()` (file stem) |

`Document.image_defs()` is not provided; `ezdwg.raw.decode_image_defs(path)`
returns the IMAGEDEF objects as `(handle, file_path, size, is_loaded,
resolution_units, pixel_size)` tuples.

### WIPEOUT

A mask that hides what lies behind it. The record is the one of IMAGE without
an image definition: the frame `insert`, `u_vector`, `v_vector` and `size`
(usually `(1.0, 1.0)`) places the clip boundary, and the clip boundary is the
masked area. Read only; R2000 and later.

A clip vertex `(x, y)` counts columns from the left of the frame and rows
from its top: `(0, 0)` is the centre of the upper-left cell, so the frame
begins half a cell before it. The world point of a vertex is
`insert + u_vector * (x + 0.5) + v_vector * (size[1] - 0.5 - y)`. This is
checked for frames of size `(1, 1)` whose axes lie along X and Y.

| Key | Type | Description |
|-----|------|-------------|
| `insert` | `(float, float, float)` | Insertion point (lower-left corner of the frame) |
| `u_vector` / `v_vector` | `(float, float, float)` | Step of one unit of the boundary coordinates along the columns / rows |
| `size` | `(float, float)` | Size of the frame in those units |
| `display_props` | `int` | Display flags, as for IMAGE |
| `clipping` | `bool` | Whether the clip boundary is enabled |
| `brightness` / `contrast` / `fade` | `int` | Stored as for IMAGE |
| `clip_type` | `int` | 1 rectangle (two corners), 2 polygon |
| `clip_mode` | `int \| None` | R2010 and later: the clip mode as stored (0 outside, 1 inside); `None` before |
| `clip_vertices` | `list[(float, float)]` | The boundary, in the columns and rows of the frame (see above) |

### MULTILEADER

A leader whose label is a text or a block. Read only; R2000 to R2018.

The row follows the DXF form of the entity: the properties of the object, and
under `context` what is drawn, in drawing units. A key that ends in `_handle`
names another object (a style, a linetype, a text style, a block record, an
attribute definition); 0 is no object. A color is `(color index, value)`: the
value is the 32-bit number a DXF file shows for it, and 0 in R2000, which
stores the color index only. Points and directions are in world coordinates,
as a DXF file of the drawing shows them; that is checked for objects in the
XY plane only. A `rotation` is in radians, as in the DXF MULTILEADER, where
TEXT, INSERT and SHAPE give degrees.

| Key | Type | Description |
|-----|------|-------------|
| `class_version` | `int \| None` | R2010 and later; `None` before |
| `style_handle` | `int` | The MLEADERSTYLE object |
| `content_type` | `int` | 0 none, 1 block, 2 text, 3 tolerance |
| `override_flags` | `int` | Which properties the object sets apart from its style |
| `leader_line_type` | `int` | 0 none, 1 straight, 2 spline |
| `leader_color` | `(int, int)` | Color of the leader lines |
| `leader_linetype_handle` | `int` | Linetype of the leader lines |
| `leader_lineweight` | `int` | As in DXF: -1 BYLAYER, -2 BYBLOCK, -3 default, else hundredths of a millimetre |
| `landing_enabled` / `dogleg_enabled` | `bool` | Whether the landing and the dogleg are on |
| `dogleg_length` | `float` | Length of the dogleg |
| `arrow_handle` | `int` | Block of the arrowhead; 0 is the default arrowhead |
| `arrow_size` | `float` | Size of the arrowhead |
| `text_style_handle` | `int` | Text style of a text label |
| `left_attachment` / `right_attachment` | `int` | Where a leader on that side meets the text |
| `text_frame` | `bool` | Whether the text has a frame |
| `block_handle` | `int` | Block of a block label |
| `block_connection` | `int` | 0 the leader meets the block's extents, 1 its base point |
| `attributes` | `list[dict]` | R2010 and later, block label: `definition_handle` (the ATTDEF of the block), `index`, `width`, `text` |
| `context` | `dict` | See below |

`context`:

| Key | Type | Description |
|-----|------|-------------|
| `scale` | `float` | Overall scale |
| `content_base` | `(float, float, float)` | Base point of the label |
| `text_height` / `arrow_size` / `landing_gap` | `float` | Sizes as drawn |
| `left_attachment` / `right_attachment` / `text_alignment` / `attachment_type` | `int` | As stored |
| `plane_origin` / `plane_x_axis` / `plane_y_axis` | `(float, float, float)` | The plane of the object |
| `plane_normal_reversed` | `bool` | As stored |
| `leaders` | `list[dict]` | See below |
| `text` | `dict \| None` | The text label: `text` (with its MTEXT format codes), `location`, `direction`, `rotation` (radians, as in the DXF MULTILEADER), `width`, `normal`, `style_handle`, `color`, `attachment`, `drawing_direction`, `background_color`, `background_scale`, `background_on` |
| `block` | `dict \| None` | The block label: `block_handle`, `location`, `scale`, `rotation` (radians, as in the DXF MULTILEADER), `normal`, `color`, `matrix` (16 values, in the order of the DXF group 47) |

Each of `leaders`:

| Key | Type | Description |
|-----|------|-------------|
| `lines` | `list[dict]` | The leader lines: `index`, and `points` from the arrowhead on |
| `last_point` | `(float, float, float)` | Where the lines end and the dogleg begins |
| `has_last_point` / `has_dogleg` | `bool` | Whether `last_point` and `dogleg_vector` are set |
| `dogleg_vector` | `(float, float, float)` | Direction of the dogleg |
| `dogleg_length` | `float` | Length of the dogleg |
| `index` | `int` | As stored |
| `attachment_direction` | `int \| None` | R2010 and later: 0 horizontal, 1 vertical |

A leader line runs from its `points` to the `last_point` of its leader. That
is the DXF form of the entity; no test of this library draws a leader line.

Limits:

- More than one leader, line, point or block attribute is read as a repeat
  of the first. No drawing at hand has two, so the repeat is not proven. For
  attributes nothing checks that the handle, the numbers and the text of one
  attribute belong together.
- A block label is proven for R2013 and R2018 only. Before R2013 the layout
  rests on one public sample drawing, which has text labels only.
- A record with a break in a leader or in a leader line, with a text column
  height, or (before R2010) with an entry in the two lists at its end is not
  returned. `ezdwg inspect` counts it under `decode_gap[MULTILEADER]`.

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

## Proxy Graphic of an Entity

An application can store a proxy graphic in the record of an entity, so that
a program without the code of that entity can still draw it. Entities of a
class, such as MULTILEADER, ACAD_TABLE, MESH, IMAGE, WIPEOUT and an underlay,
often carry one. `raw.decode_entity_proxy_graphics` returns those bytes by
handle:

```python
handles = [entity.handle for entity in doc.entities().query("MULTILEADER")]
for handle, graphic in ezdwg.raw.decode_entity_proxy_graphics("drawing.dwg", handles):
    if graphic is None:
        ...  # the entity has no proxy graphic
```

- There is one row for each handle asked whose record is an entity and
  reads, in the order asked. `limit` stops the read after that many rows.
- The bytes are returned as stored: they are the bytes a DXF file of the
  drawing carries as the proxy graphic of the entity. The function does not
  look into them.
- A handle with no row was not read: it is not in the drawing, it is not an
  entity, or its record is damaged (the proxy graphic runs past the end of
  the record, or the record holds another handle than the one asked). The
  handles asked less the rows returned is the count of those.
- DWG versions R14 to R2018. R13 is not read, and gives no rows. R2013 and
  R2018 are checked on production drawings, the older versions on one public
  sample drawing only.
- Each call builds its table of handles over all objects of the drawing, so
  ask the handles of one drawing in one call. One proxy graphic is at most
  as large as its record; the sum of a call has no limit of its own.

!!! note "ARC Angles"
    The high-level API returns ARC angles in **degrees**. The raw API (`ezdwg.raw`) returns angles in **radians**.
