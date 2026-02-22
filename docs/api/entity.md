# Entity

## Entity

```python
@dataclass(frozen=True)
class Entity:
    dxftype: str
    handle: int
    dxf: dict[str, Any]
```

A DWG entity. Instances are returned by [`Layout.query()`](document.md#query).

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `dxftype` | `str` | Entity type name (e.g. `"LINE"`, `"ARC"`, `"CIRCLE"`) |
| `handle` | `int` | Unique handle within the DWG file |
| `dxf` | `dict[str, Any]` | Entity attributes (type-specific) |

### Methods

#### to_points

```python
Entity.to_points() -> list[tuple[float, float, float]]
```

Extract key coordinate points from the entity.

**Returns:** List of 3D points.

**Raises:** `NotImplementedError` for unsupported entity types.

**Supported types:**

| Type | Points Returned |
|------|----------------|
| `LINE` | `[start, end]` |
| `LWPOLYLINE` | All vertex points |
| `POINT` | `[location]` |
| `TEXT` | `[insert]` |
| `MTEXT` | `[insert]` |
| `DIMENSION` | `[defpoint2, defpoint3]` or `[text_midpoint]` |
| `RAY` | `[start, start + unit_vector]` |
| `XLINE` | `[start - unit_vector, start + unit_vector]` |

**Example:**

```python
for entity in msp.query("LINE"):
    start, end = entity.to_points()
    print(f"Line from {start} to {end}")
```

## Supported Entity Types

The following entity types are supported by the high-level API:

| Type | Description |
|------|-------------|
| `LINE` | Line segment |
| `ARC` | Circular arc |
| `CIRCLE` | Full circle |
| `ELLIPSE` | Ellipse or elliptical arc |
| `LWPOLYLINE` | Lightweight polyline |
| `POLYLINE_2D` | 2D polyline |
| `POLYLINE_3D` | 3D polyline |
| `POLYLINE_MESH` | Polygon mesh |
| `POLYLINE_PFACE` | Polyface mesh |
| `POINT` | Point |
| `TEXT` | Single-line text |
| `MTEXT` | Multi-line text |
| `ATTRIB` | Block attribute |
| `ATTDEF` | Attribute definition |
| `DIMENSION` | Dimension (linear, radius, diameter, aligned, ordinate, angular) |
| `LEADER` | Leader line |
| `HATCH` | Hatch pattern |
| `TOLERANCE` | Tolerance annotation |
| `MLINE` | Multi-line |
| `SPLINE` | Spline curve |
| `INSERT` | Block reference |
| `MINSERT` | Multiple block reference (array) |
| `3DFACE` | 3D face |
| `SOLID` | Solid fill |
| `TRACE` | Trace |
| `SHAPE` | Shape reference |
| `RAY` | Ray |
| `XLINE` | Construction line |
| `VIEWPORT` | Viewport |
| `OLEFRAME` | OLE frame |
| `OLE2FRAME` | OLE2 frame |
| `LONG_TRANSACTION` | Long transaction |
| `REGION` | Region |
| `3DSOLID` | 3D solid |
| `BODY` | ACIS body |
