# Document & Layout

## Document

```python
@dataclass(frozen=True)
class Document:
    path: str
    version: str
    decode_path: str | None = None
    decode_version: str | None = None
```

A DWG document. Created by [`ezdwg.read()`](core.md#ezdwgread).

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `path` | `str` | Path to the DWG file |
| `version` | `str` | DWG version code (e.g. `"AC1015"`) |
| `decode_path` | `str \| None` | Path used for decoding (defaults to `path`) |
| `decode_version` | `str \| None` | Version used for decoding (defaults to `version`) |

### Methods

#### modelspace

```python
Document.modelspace() -> Layout
```

Return the modelspace layout.

**Example:**

```python
doc = ezdwg.read("drawing.dwg")
msp = doc.modelspace()
```

#### graph

```python
Document.graph(limit: int | None = None) -> DocumentGraph
```

Return the graph-oriented IR for the document. The graph contains object nodes, common entity data, object edges, layer table rows, block header rows, and inferred model/paper space header handles.

#### header_variables

```python
Document.header_variables() -> dict[str, Any]
```

Decode a subset of the DWG header variables. Keys: `insunits`, `lunits`,
`luprec`, `aunits`, `auprec`, `ltscale`, `textsize`, `extmin`, `extmax`,
`limmin`, `limmax`. Values are `None` when the variable is absent for the file
version — R14 (`AC1014`) has no `$INSUNITS` header variable, so every field is
`None` for R14 files. Raises on unreadable header sections.

```python
doc = ezdwg.read("drawing.dwg")
variables = doc.header_variables()
variables["insunits"]  # 4
variables["extmin"]    # (0.0, 0.0, 0.0)
```

#### units

```python
Document.units -> str | None
```

Drawing units name resolved from `$INSUNITS` (e.g. `"millimeters"`,
`"inches"`, `"unitless"`). Returns `None` when the code is unavailable (R14, or
an unreadable header). Unknown codes are reported as `"unknown_<code>"`. The
raw integer code is available as `Document.insunits`.

```python
doc = ezdwg.read("drawing.dwg")
doc.units     # "millimeters"
doc.insunits  # 4
```

#### plot

```python
Document.plot(*args, **kwargs) -> Axes
```

Plot all entities in the modelspace. Accepts the same parameters as [`ezdwg.plot()`](core.md#ezdwgplot).

#### export_dxf

```python
Document.export_dxf(output_path: str, **kwargs) -> ConvertResult
```

Export the modelspace to a DXF file. Accepts the same keyword arguments as [`ezdwg.to_dxf()`](core.md#ezdwgto_dxf).

#### export_dwg

```python
Document.export_dwg(output_path: str, **kwargs) -> WriteResult
```

Export the modelspace to a DWG file using the native writer. Accepts the same keyword arguments as [`ezdwg.to_dwg()`](core.md#ezdwgto_dwg).

---

## Layout

```python
@dataclass(frozen=True)
class Layout:
    doc: Document
    name: str
```

A drawing layout (e.g. modelspace).

### Methods

#### query

```python
Layout.query(types: str | Iterable[str] | None = None) -> Iterator[Entity]
```

Iterate over entities, optionally filtered by type.

**Parameters:**

- `types` — Space-separated type names (e.g. `"LINE ARC"`), an iterable of type names, or `None` for all types.

**Returns:** Iterator of [`Entity`](entity.md) objects.

**Examples:**

```python
# All entities
for entity in msp.query():
    print(entity.dxftype)

# Filter by type string
for line in msp.query("LINE"):
    print(line.dxf["start"])

# Multiple types
for entity in msp.query("LINE ARC CIRCLE"):
    print(entity.dxftype, entity.handle)

# From an iterable
for entity in msp.query(["LINE", "ARC"]):
    print(entity.dxftype)
```

#### iter_entities

```python
Layout.iter_entities(types: str | Iterable[str] | None = None) -> Iterator[Entity]
```

Alias for `query()`.

#### plot

```python
Layout.plot(*args, **kwargs) -> Axes
```

Plot entities in this layout. Accepts the same parameters as [`ezdwg.plot()`](core.md#ezdwgplot).

#### export_dxf

```python
Layout.export_dxf(output_path: str, **kwargs) -> ConvertResult
```

Export this layout to a DXF file. Accepts the same keyword arguments as [`ezdwg.to_dxf()`](core.md#ezdwgto_dxf).

#### export_dwg

```python
Layout.export_dwg(output_path: str, **kwargs) -> WriteResult
```

Export this layout to a DWG file using the native writer. Accepts the same keyword arguments as [`ezdwg.to_dwg()`](core.md#ezdwgto_dwg).
