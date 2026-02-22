# Core Functions

## ezdwg.read

```python
ezdwg.read(path: str) -> Document
```

Open a DWG file and return a `Document`.

**Parameters:**

- `path` — Path to the DWG file.

**Returns:** A [`Document`](document.md) object.

**Raises:** `ValueError` if the DWG version is unsupported.

**Example:**

```python
import ezdwg

doc = ezdwg.read("drawing.dwg")
print(doc.version)
```

---

## ezdwg.plot

```python
ezdwg.plot(
    target: str | Document | Layout,
    types: str | Iterable[str] | None = None,
    ax: Axes | None = None,
    show: bool = True,
    equal: bool = True,
    title: str | None = None,
    line_width: float = 1.0,
    arc_segments: int = 64,
    auto_fit: bool = True,
    fit_margin: float = 0.04,
    dimension_color: Any | None = "black",
) -> Axes
```

Plot DWG entities using matplotlib.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `target` | `str \| Document \| Layout` | — | File path, Document, or Layout |
| `types` | `str \| Iterable[str] \| None` | `None` | Entity type filter |
| `ax` | `Axes \| None` | `None` | Existing matplotlib axes |
| `show` | `bool` | `True` | Call `plt.show()` |
| `equal` | `bool` | `True` | Equal aspect ratio |
| `title` | `str \| None` | `None` | Plot title |
| `line_width` | `float` | `1.0` | Line width |
| `arc_segments` | `int` | `64` | Segments for arcs |
| `auto_fit` | `bool` | `True` | Auto-fit view bounds |
| `fit_margin` | `float` | `0.04` | Margin fraction |
| `dimension_color` | `Any \| None` | `"black"` | Dimension color |

**Returns:** The matplotlib `Axes` object.

**Example:**

```python
import ezdwg

ezdwg.plot("drawing.dwg", types="LINE ARC", title="My Drawing")
```

---

## ezdwg.to_dxf

```python
ezdwg.to_dxf(
    source: str | Document | Layout,
    output_path: str,
    *,
    types: str | Iterable[str] | None = None,
    dxf_version: str = "R2010",
    strict: bool = False,
    include_unsupported: bool = False,
) -> ConvertResult
```

Convert a DWG file to DXF format.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | `str \| Document \| Layout` | — | File path, Document, or Layout |
| `output_path` | `str` | — | Output DXF file path |
| `types` | `str \| Iterable[str] \| None` | `None` | Entity type filter |
| `dxf_version` | `str` | `"R2010"` | DXF version string |
| `strict` | `bool` | `False` | Fail on skipped entities |
| `include_unsupported` | `bool` | `False` | Query unsupported types |

**Returns:** A `ConvertResult` object.

**Raises:** `ValueError` in strict mode if entities are skipped. `ImportError` if ezdxf is not installed.

---

## ConvertResult

```python
@dataclass(frozen=True)
class ConvertResult:
    source_path: str
    output_path: str
    total_entities: int
    written_entities: int
    skipped_entities: int
    skipped_by_type: dict[str, int]
```

Result of a DWG to DXF conversion.

| Attribute | Type | Description |
|-----------|------|-------------|
| `source_path` | `str` | Input DWG path |
| `output_path` | `str` | Output DXF path |
| `total_entities` | `int` | Total entities processed |
| `written_entities` | `int` | Successfully written |
| `skipped_entities` | `int` | Skipped count |
| `skipped_by_type` | `dict[str, int]` | Skipped by entity type |
