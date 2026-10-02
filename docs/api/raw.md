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

### decode_document_graph

```python
raw.decode_document_graph(path: str, limit: int | None = None) -> tuple[
    str,
    list[tuple[int, int, int, int, str, str]],
    list[
        tuple[
            int,
            str,
            int | None,
            int | None,
            int | None,
            int,
            int | None,
            int | None,
            int | None,
            int | None,
            list[int],
        ]
    ],
    list[tuple[int, str, int]],
    list[tuple[int, str | None, int | None, int | None]],
    list[tuple[int, str]],
    list[tuple[str, int | None]],
]
```

Decode a compact graph-oriented IR. The returned tuple contains:

- DWG version string.
- Object headers with resolved type names.
- Common entity data: `(handle, type_name, owner_handle, color_index, true_color, layer_handle, linetype_handle, material_handle, plotstyle_handle, extension_dict_handle, reactor_handles)`.
- Object graph edges: `(source_handle, kind, target_handle)`.
- Layer table rows: `(handle, name, color_index, true_color)`.
- Block header table rows: `(handle, name)`.
- Header handle rows such as `("model_space_block_header", handle)`, current table handles such as `("clayer", handle)`, dictionaries, and table control handles such as `("layer_control", handle)`.

When `limit` is provided, object rows are truncated first and graph rows are limited to those object handles.

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

### decode_layer_names

```python
raw.decode_layer_names(path: str, limit: int | None = None) -> list[tuple[int, str]]
```

Decode the layer table. Each tuple: `(handle, name)`.

## Linetype Data

### decode_linetypes

```python
raw.decode_linetypes(path: str, limit: int | None = None) -> list[tuple[int, str, str, float, list[float]]]
```

Decode the linetype table. Each tuple: `(handle, name, description, pattern_length, dashes)`.
`dashes` is the dash pattern in drawing units at linetype scale 1, with the DXF
sign convention: positive = dash, negative = gap, 0 = dot. The table includes
the built-in `ByBlock`, `ByLayer` and `Continuous` entries, whose pattern is empty.

### decode_layer_linetypes

```python
raw.decode_layer_linetypes(path: str, limit: int | None = None) -> list[tuple[int, int]]
```

Linetype of every layer. Each tuple: `(layer_handle, linetype_handle)`.

### decode_entity_linetypes

```python
raw.decode_entity_linetypes(path: str, limit: int | None = None) -> list[tuple[int, int, int, int | None, float]]
```

Linetype of every entity. Each tuple:
`(handle, layer_handle, linetype_flags, linetype_handle, linetype_scale)`.

| `linetype_flags` | Meaning |
|------------------|---------|
| 0 | BYLAYER: the linetype of `layer_handle` |
| 1 | BYBLOCK |
| 2 | CONTINUOUS |
| 3 | The linetype named by `linetype_handle` |

`linetype_scale` is the entity's own linetype scale (DXF group 48). The rows come
from the common entity data, so they cover every entity type, including the ones
without a geometry decoder, and `layer_handle` is the layer stored there.

A dash of an entity is `dash * $LTSCALE * linetype_scale` drawing units long
(`$LTSCALE` is `Document.header_variables()["ltscale"]`).

## Layer State and Lineweights

### decode_layer_states

```python
raw.decode_layer_states(path: str, limit: int | None = None) -> list[tuple[int, bool, bool, bool, bool, bool, int]]
```

State of every layer. Each tuple:
`(layer_handle, frozen, off, frozen_in_new_viewports, locked, plot, lineweight)`.

`lineweight` is in hundredths of a millimetre, as DXF group 370 writes it:
`-3` is the default lineweight, `-1` BYLAYER and `-2` BYBLOCK. R13/R14 files
have neither a plot flag nor lineweights: `plot` is `True` and `lineweight` is
`-3` for every layer.

A layer that is off or frozen shows none of its entities; a layer with
`plot = False` is displayed but not printed.

### decode_dimstyles

```python
raw.decode_dimstyles(path: str, limit: int | None = None) -> list[tuple[int, str, float, float, float]]
```

Sizes of every dimension style. Each tuple:
`(handle, name, dimscale, dimasz, dimtxt)`: the overall scale (0 for a style
scaled by the viewport or annotatively), the arrow size and the text height.

### decode_layout_objects

```python
raw.decode_layout_objects(path: str, limit: int | None = None) -> list[tuple]
```

Layouts of the drawing (the model tab and the paper-space sheets). Each tuple:
`(handle, name, tab_order, flags, block_record_handle, paper, limits, extents,
viewport_handles, last_active_viewport_handle, plot)`.

- `paper`: `(paper_width, paper_height, (left, bottom, right, top) margins,
  paper_size_name, paper_units, plot_rotation)`. Sizes are millimetres before
  the plot rotation (0-3 quarter turns counter-clockwise); `paper_units` is 0
  for inches, 1 for millimetres and 2 for pixels.
- `limits` and `extents`: `(min, max)` in layout units.
- `viewport_handles`: the viewports of the layout, the sheet's own viewport
  first (R2004+; empty before).
- `plot`: `(plot_flags, plot_origin, plot_type, window_min, window_max,
  scale_numerator, scale_denominator, scale_type, scale_factor)`.

Layouts whose record cannot be read are omitted.

### decode_viewport_details

```python
raw.decode_viewport_details(path: str, limit: int | None = None) -> list[tuple]
```

Paper-space viewports. Each tuple: `(handle, center, width, height, view,
frozen_layer_handles, clip_boundary_handle, layer_handle)`.

`center`, `width` and `height` place the viewport on its sheet; `center` is
`None` when the body of the viewport cannot be read. `view` is `(target,
direction, twist_angle, view_height, lens_length, front_clip_z, back_clip_z,
view_center, status_flags, render_mode)` with the twist angle in radians, or
`None` for R13/R14 files, which keep the view in the extended data of the
entity. `decode_viewport_entities` still returns the handles alone.

### decode_entity_lineweights

```python
raw.decode_entity_lineweights(path: str, limit: int | None = None) -> list[tuple[int, int | None, bool]]
```

Lineweight and visibility of every entity. Each tuple:
`(handle, lineweight, invisible)`. `lineweight` uses the values above (`-1`
when the entity takes the lineweight of its layer) and is `None` for R13/R14
files. `invisible` is the invisibility flag of the entity (DXF group 60).

Like `decode_entity_linetypes`, the rows come from the common entity data and
cover every entity type.

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

Decode all DIMENSION entity subtypes. Returns `(dimtype, row)` pairs; `row` is the
12-tuple shared by every `decode_dim_*_entities` function:
`(handle, user_text, defpoint(10), defpoint2(13), defpoint3(14), text_midpoint(11),
insert_point(12) | None, (extrusion, insert_scale), (text_rotation, horizontal_direction,
ext_line_rotation, dim_rotation), (dim_flags, actual_measurement, attachment_point,
line_spacing_style, line_spacing_factor, insert_rotation), (dimstyle_handle,
anonymous_block_handle), (point15 | None, point16 | None))`. The last element carries
the type-specific extra points: DXF code 15 for `ANG3PT`/`ANG2LN`/`RADIUS`/`DIAMETER`
and code 16 (`(x, y)`) for `ANG2LN`; other types yield `(None, None)`.
`anonymous_block_handle` is the `BLOCK_HEADER` that holds the saved graphics of the
dimension (`None` when the dimension has no block); the entities of that block have it
as their owner handle.

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

### decode_hatch_patterns

```python
raw.decode_hatch_patterns(path: str, limit: int | None = None) -> list[tuple[int, float, float, bool, list[tuple[float, tuple[float, float], tuple[float, float], list[float]]]]]
```

Pattern definition of every pattern-filled hatch. Each tuple:
`(handle, pattern_angle, pattern_scale, double, lines)`, with one
`(angle, base, offset, dashes)` per family of parallel pattern lines. Angles are
in radians. The lines are stored already rotated and scaled: line `k` of a
family runs through `base + k * offset` at `angle`, dashed by `dashes`
(empty = continuous). Solid fills and hatches whose definition cannot be read
have no row.
