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
