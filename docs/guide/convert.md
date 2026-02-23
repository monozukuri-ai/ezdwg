# DWG to DXF Conversion

ezdwg can convert DWG files to DXF format using [ezdxf](https://ezdxf.readthedocs.io/) as the writing backend.

```bash
pip install "ezdwg[dxf]"
```

## Basic Usage

### From a File Path

```python
import ezdwg

result = ezdwg.to_dxf("input.dwg", "output.dxf")
print(f"Written: {result.written_entities}/{result.total_entities}")
```

### From a Document

```python
doc = ezdwg.read("input.dwg")
result = doc.export_dxf("output.dxf")
```

### From a Layout

```python
doc = ezdwg.read("input.dwg")
msp = doc.modelspace()
result = msp.export_dxf("output.dxf")
```

## Options

### Filtering by Entity Type

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    types="LINE ARC LWPOLYLINE",
)
```

### DXF Version

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    dxf_version="R2010",  # default
)
```

Supported DXF versions: `R2000`, `R2004`, `R2007`, `R2010`, `R2013`, `R2018`.

### Strict Mode

In strict mode, conversion fails if any entity cannot be written:

```python
result = ezdwg.to_dxf(
    "input.dwg",
    "output.dxf",
    strict=True,
)
```

## ConvertResult

`to_dxf()` returns a `ConvertResult` with:

| Attribute | Type | Description |
|-----------|------|-------------|
| `source_path` | `str` | Input DWG path |
| `output_path` | `str` | Output DXF path |
| `total_entities` | `int` | Total entities processed |
| `written_entities` | `int` | Entities successfully written |
| `skipped_entities` | `int` | Entities that could not be written |
| `skipped_by_type` | `dict[str, int]` | Skip counts grouped by type |

## Supported Entity Types

The following entity types can be written to DXF:

LINE, RAY, XLINE, POINT, ARC, CIRCLE, ELLIPSE, LWPOLYLINE,
POLYLINE_2D, POLYLINE_3D, POLYLINE_MESH, POLYLINE_PFACE,
3DFACE, SOLID, TRACE, SHAPE, SPLINE,
TEXT, ATTRIB, ATTDEF, MTEXT, LEADER, HATCH, TOLERANCE, MLINE,
INSERT, MINSERT, DIMENSION

## Limitations

- Conversion is best-effort — some entities may be skipped if they cannot be represented in DXF.
- Block definitions referenced by INSERT entities are reconstructed from the DWG source.
- DIMENSION entities use native ezdxf dimension builders with a text fallback for unsupported subtypes.
- Style, linetype, and layer properties may not be fully preserved.

---

## Native DWG Writer (AC1015)

ezdwg also provides a native DWG writer for AC1015.

```python
import ezdwg

result = ezdwg.to_dwg(
    "input.dwg",
    "output.dwg",
    version="AC1015",
    types="LINE ARC CIRCLE LWPOLYLINE TEXT MTEXT",
)
print(result.written_entities, result.skipped_by_type)
```

You can also call:

```python
doc = ezdwg.read("input.dwg")
doc.export_dwg("output.dwg", version="AC1015")
```

Current native writer scope:

- Version: `AC1015` only
- Entities: `LINE`, `RAY`, `XLINE`, `POINT`, `ARC`, `CIRCLE`, `LWPOLYLINE`, `TEXT`, `MTEXT`
