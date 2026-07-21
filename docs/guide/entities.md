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
| `insert` | `(float, float, float)` | Insertion point |
| `text` | `str` | Text content |
| `height` | `float` | Text height |
| `rotation` | `float` | Rotation angle in degrees |

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

For closed paths, the high-level API repeats the first point at the end of the
path. `to_points()` concatenates every path in source order.

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
| `text` | `str` | Override text |
| `angle` | `float` | Rotation angle in degrees |
| `actual_measurement` | `float` | Computed measurement value |

!!! note "ARC Angles"
    The high-level API returns ARC angles in **degrees**. The raw API (`ezdwg.raw`) returns angles in **radians**.
