# Raw API

The `ezdwg.raw` module provides low-level access to the Rust decode functions. These functions return data as tuples for maximum performance.

!!! warning "Angle Units"
    The raw API returns ARC angles in **radians**, unlike the high-level API which uses degrees.

## File Inspection

### detect_version

```python
raw.detect_version(path: str) -> str
```

Detect the DWG version string (e.g. `"AC1015"`).

### list_section_locators

```python
raw.list_section_locators(path: str) -> list[tuple[str, int, int]]
```

List section locators. Each tuple: `(name, offset, size)`.

### list_object_map_entries

```python
raw.list_object_map_entries(path: str, limit: int | None = None) -> list[tuple[int, int]]
```

List object map entries. Each tuple: `(handle, offset)`.

### list_object_headers

```python
raw.list_object_headers(path: str, limit: int | None = None) -> list[tuple[int, int, int, int]]
```

List object headers. Each tuple: `(handle, offset, size, type_code)`.

### list_object_headers_with_type

```python
raw.list_object_headers_with_type(path: str, limit: int | None = None) -> list[tuple[int, int, int, int, str, str]]
```

List object headers with resolved type names. Each tuple: `(handle, offset, size, type_code, type_name, type_class)`.

`type_class` is `"E"` for entities and `"O"` for objects.

### list_object_headers_by_type

```python
raw.list_object_headers_by_type(path: str, type_codes: list[int], limit: int | None = None) -> list[tuple[int, int, int, int, str, str]]
```

List object headers filtered by type codes.

## Object Record Access

### read_object_records_by_type

```python
raw.read_object_records_by_type(path: str, type_codes: list[int], limit: int | None = None) -> list[tuple[int, int, int, int, bytes]]
```

Read raw object records by type code. Each tuple: `(handle, offset, size, type_code, data)`.

### read_object_records_by_handle

```python
raw.read_object_records_by_handle(path: str, handles: list[int], limit: int | None = None) -> list[tuple[int, int, int, int, bytes]]
```

Read raw object records by handle.

### decode_object_handle_stream_refs

```python
raw.decode_object_handle_stream_refs(path: str, handles: list[int], limit: int | None = None) -> list[tuple[int, list[int]]]
```

Decode handle-stream references for objects. Each tuple: `(handle, ref_handles)`.

## Style and Layer Data

### decode_entity_styles

```python
raw.decode_entity_styles(path: str, limit: int | None = None) -> list[tuple[int, int | None, int | None, int]]
```

Decode entity style information. Each tuple: `(handle, color_index, true_color, layer_handle)`.

### decode_layer_colors

```python
raw.decode_layer_colors(path: str, limit: int | None = None) -> list[tuple[int, int, int | None]]
```

Decode layer color information. Each tuple: `(handle, color_index, true_color)`.

## Geometry Decode Functions

All geometry decode functions take a `path` and optional `limit` parameter.

### decode_line_entities

```python
raw.decode_line_entities(path: str, limit: int | None = None) -> list[tuple[int, float, float, float, float, float, float]]
```

Each tuple: `(handle, start_x, start_y, start_z, end_x, end_y, end_z)`.

### decode_arc_entities

```python
raw.decode_arc_entities(path: str, limit: int | None = None) -> list[tuple[int, float, float, float, float, float, float]]
```

Each tuple: `(handle, center_x, center_y, center_z, radius, start_angle, end_angle)`.

!!! warning
    Angles are in **radians**.

### decode_circle_entities

```python
raw.decode_circle_entities(path: str, limit: int | None = None) -> list[tuple[int, float, float, float, float]]
```

Each tuple: `(handle, center_x, center_y, center_z, radius)`.

### decode_point_entities

```python
raw.decode_point_entities(path: str, limit: int | None = None) -> list[tuple[int, float, float, float, float]]
```

Each tuple: `(handle, x, y, z, thickness)`.

### decode_ellipse_entities

```python
raw.decode_ellipse_entities(path: str, limit: int | None = None) -> list[tuple[int, ...]]
```

Each tuple: `(handle, center, extrusion, major_axis, ratio, start_angle, end_angle)`.

### decode_lwpolyline_entities

```python
raw.decode_lwpolyline_entities(path: str, limit: int | None = None) -> list[tuple[int, int, list[tuple[float, float]], list[float], list[tuple[float, float]], float | None]]
```

Each tuple: `(handle, flags, points, bulges, widths, const_width)`.

### decode_text_entities

```python
raw.decode_text_entities(path: str, limit: int | None = None) -> list[tuple[int, str, ...]]
```

Decode TEXT entities with text content, insertion point, alignment, and style information.

### decode_mtext_entities

```python
raw.decode_mtext_entities(path: str, limit: int | None = None) -> list[tuple[int, str, ...]]
```

Decode MTEXT entities with text content, insertion point, size, and attachment information.

### decode_dimension_entities

```python
raw.decode_dimension_entities(path: str, limit: int | None = None) -> list[tuple]
```

Decode all DIMENSION entity subtypes. Returns complex tuples containing dimension type, definition points, text, and measurement data.

### decode_insert_entities

```python
raw.decode_insert_entities(path: str, limit: int | None = None) -> list[tuple[int, float, float, float, float, float, float, float, str | None]]
```

Each tuple: `(handle, x, y, z, xscale, yscale, zscale, rotation, block_name)`.

## Bulk Decode

### decode_line_arc_circle_entities

```python
raw.decode_line_arc_circle_entities(path: str, limit: int | None = None) -> tuple[list, list, list]
```

Decode LINE, ARC, and CIRCLE entities in a single pass for better performance. Returns a 3-tuple of `(lines, arcs, circles)`.

## Usage Example

```python
from ezdwg import raw

# Detect version
version = raw.detect_version("drawing.dwg")
print(f"Version: {version}")

# Decode lines
for handle, sx, sy, sz, ex, ey, ez in raw.decode_line_entities("drawing.dwg"):
    print(f"Line {handle}: ({sx},{sy},{sz}) -> ({ex},{ey},{ez})")

# Decode arcs (angles in radians!)
import math
for handle, cx, cy, cz, r, sa, ea in raw.decode_arc_entities("drawing.dwg"):
    print(f"Arc {handle}: center=({cx},{cy},{cz}) r={r} "
          f"angles={math.degrees(sa):.1f}°-{math.degrees(ea):.1f}°")
```
