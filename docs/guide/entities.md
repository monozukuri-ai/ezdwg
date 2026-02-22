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
for entity in msp.query("LINE LWPOLYLINE POINT"):
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
