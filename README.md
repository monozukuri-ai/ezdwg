# ezdwg
[![PyPI version](https://badge.fury.io/py/ezdwg.svg)](https://badge.fury.io/py/ezdwg)

DWG (R13-R2018 / AC1012-AC1032) reader with a Python API inspired by [ezdxf](https://github.com/mozman/ezdxf).
This project is a **DWG read-focused parser** with growing high-level entity coverage while providing a native **AC1015 writer** for basic entities and nested blocks.

Documentation: https://monozukuri-ai.github.io/ezdwg/

## Status
- High-level API (`ezdwg.read`): **R13 / AC1012** and **R14 / AC1014** (2D entities), **R2000 / AC1015**, **R2004 / AC1018**, **R2007 / AC1021**, **R2010 / AC1024**, **R2013 / AC1027**, **R2018 / AC1032**
- Raw API (`ezdwg.raw`): the same versions, with the per-type `decode_*` functions and the table, style and placement functions
- High-level entities include **LINE**, **ARC**, **LWPOLYLINE**, **POINT**, **CIRCLE**, **ELLIPSE**, **TEXT**, **MTEXT**, **DIMENSION**, **INSERT/MINSERT**, **HATCH**, and **SPLINE**
- Raw API exposes the corresponding entity decoders plus low-level POLYLINE/VERTEX helpers
- Output units/angles: high-level API returns ARC angles in **degrees**

## Install
Install from PyPI:

```bash
pip install ezdwg
```

Plotting (optional):

```bash
pip install "ezdwg[plot]"
```

DWG to DXF conversion (optional, ezdxf backend):

```bash
pip install "ezdwg[dxf]"
```

If you want to contribute or develop locally, you can install from source:
Rust toolchain is required (PyO3 build).

```bash
git clone https://github.com/monozukuri-ai/ezdwg.git
cd ezdwg
maturin develop
pip install -e .
```

For release version updates, run `uv run scripts/bump_version.py <version>`.
The root `Cargo.toml` is the single version source for Rust and Python; the
command refreshes the lock files. See [Releasing](docs/releasing.md) for details.

## Quick Start
```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
msp = doc.modelspace()

for e in msp.query(
    "LINE LWPOLYLINE ARC CIRCLE ELLIPSE POINT TEXT MTEXT "
    "DIMENSION INSERT MINSERT HATCH SPLINE"
):
    print(e.dxftype, e.handle, e.dxf)
```

`doc.modelspace()` yields only model-space entities (block-definition contents and
paper-space entities are excluded via each entity's stored placement);
`doc.paperspace()` yields paper-space entities and `doc.entities()` everything in
the file, with `e.dxf["owner_handle"]` / `doc.entity_placement(handle)` to
partition them yourself.

Layouts and viewports:

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
for name, layout in doc.layouts().items():
    print(name, layout["model"], layout["active"], layout["paper_width"], layout["paper_height"])

for viewport in doc.paperspace().query("VIEWPORT"):
    dxf = viewport.dxf
    scale = dxf["height"] / dxf["view_height"]  # R2000+; the first viewport of a sheet is the sheet itself
```

Drawing units and header variables:

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
doc.units                        # "millimeters" ($INSUNITS; None for R14)
doc.header_variables()["extmin"]  # model-space extents, unit formats, ...
```

Plot in matplotlib:

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
doc.plot()
```

## CLI
```bash
uvx ezdwg --version
uvx ezdwg inspect examples/data/line_2000.dwg
uvx ezdwg inspect examples/data/line_2000.dwg --verbose
uvx --from "ezdwg[plot]" ezdwg plot examples/data/line_2000.dwg
uvx --from "ezdwg[plot]" ezdwg plot examples/data/line_2000.dwg -o /tmp/line_2000.png --dpi 200
uvx --from "ezdwg[dxf]" ezdwg convert examples/data/line_2000.dwg /tmp/line_2000_out.dxf
uvx --from "ezdwg[dxf]" ezdwg convert examples/data/arc_2000.dwg /tmp/arc_2000_out.dxf --types "ARC" --dxf-version R2010
uvx ezdwg write examples/data/line_2000.dwg /tmp/line_2000_out.dwg --types "LINE" --dwg-version AC1015
```

`plot` displays model-space entities in a matplotlib window, or opens an SVG
preview in your browser when no interactive backend is available. Use `-o` / `--output`
to save a PNG, SVG, or PDF without opening a viewer; `--types` and `--title`
filter entities and set the title. For local development, run
`uv run --extra plot ezdwg plot path/to/file.dwg`.

## DWG to DXF
`ezdxf` is used as the DXF writing backend.

```python
import ezdwg

result = ezdwg.to_dxf(
    "examples/data/line_2000.dwg",
    "/tmp/line_2000_out.dxf",
    types="LINE ARC LWPOLYLINE",
    dxf_version="R2010",
)
print(result)
```

Or from an already opened document/layout:

```python
doc = ezdwg.read("examples/data/line_2000.dwg")
doc.export_dxf("/tmp/line_2000_out.dxf")
```

## DWG to DWG (Native AC1015 Writer)

```python
import ezdwg

result = ezdwg.to_dwg(
    "examples/data/line_2000.dwg",
    "/tmp/line_2000_rewrite.dwg",
    version="AC1015",
)
print(result)
```

Or from an already opened document/layout:

```python
doc = ezdwg.read("examples/data/line_2000.dwg")
doc.export_dwg("/tmp/line_2000_rewrite.dwg", version="AC1015")
```

Path/Document exports select model space. Use `strict=True` to reject unsupported
or undecoded entities and detected attribute loss before writing. Otherwise inspect
`result.skipped_by_type` and `result.diagnostics`. Referenced layers, simple line
patterns, basic text styles, Japanese text, units and placement attributes are
preserved. Referenced blocks and attached attributes are kept as a graph. Complete
source document metadata remains outside the scope.
See [native writer scope and external checks](docs/guide/convert.md#native-dwg-writer-ac1015).

## Create and Edit DWG

```python
import ezdwg

drawing = ezdwg.new()  # AC1015, no input file needed
msp = drawing.modelspace()
line = msp.add_line((0, 0), (10, 5), color_index=1)
msp.update(line, end=(20, 5))
drawing.save("created.dwg")  # strict validation by default

edited = ezdwg.read("created.dwg").edit()
edited.modelspace().add_circle((5, 5), radius=2)
edited.save("edited.dwg")
```

The editable snapshot supports basic entities, block definitions, nested INSERTs
and single-line ATTDEF/ATTRIB attributes.
Queries return live entities; use `update()`, mutate `entity.dxf`, or call
`delete(entity)`. Saving validates the drawing, writes a temporary sibling file,
and atomically replaces the destination. See the [editing guide](docs/guide/convert.md#creating-and-editing-a-drawing)
for table editing, import limitations and diagnostics.

Blocks use the same geometry methods in local coordinates:

```python
part = drawing.blocks.new("Part", base_point=(0, 0, 0))
part.add_circle((0, 0), radius=2)
part.add_attdef("ID", "default", insert=(0, 3), height=1)
ref = msp.add_blockref("Part", insert=(10, 20), rotation=30)
msp.add_attrib(ref, "ID", "P-001", insert=(10, 23), height=1)
drawing.save("blocks.dwg")
```

Attribute placement is explicit; `add_attrib` does not automatically transform an
ATTDEF or synchronize defaults. See [blocks and attributes](docs/guide/convert.md#blocks-and-attributes).

## Examples
Sample DWG files are available under `examples/data/`.

```bash
python examples/basic_read.py
python examples/query_types.py
python examples/plot.py
python examples/text_mtext.py
python examples/dimensions.py
python examples/raw_insert_2004.py
python examples/convert_to_dxf.py
```

## Low-Level API
If you need raw decode access:

```python
from ezdwg import raw

raw.decode_line_entities("path/to/file.dwg")
```

## Limitations
- Native DWG write currently targets **AC1015** only
- Native DWG write supports a subset of entities: **LINE/RAY/XLINE/POINT/ARC/CIRCLE/ELLIPSE/SPLINE/SOLID/TRACE/3DFACE/LWPOLYLINE/POLYLINE_2D/POLYLINE_3D/TEXT/MTEXT/INSERT/ATTDEF/ATTRIB**, including nested block definitions
- High-level API supports R13 (AC1012, experimental), R14 (AC1014, experimental), R2000 (AC1015), R2004 (AC1018), R2007 (AC1021), R2010 (AC1024), R2013 (AC1027), and R2018 (AC1032)
- AC1012/AC1014 decode the 2D entity types with the layouts of the ODA specification (LINE, ARC, CIRCLE, ELLIPSE, POINT, LWPOLYLINE, POLYLINE_2D, TEXT, MTEXT, ATTRIB/ATTDEF, INSERT/MINSERT, HATCH, SOLID, TRACE, 3DFACE, SPLINE, LEADER, TOLERANCE, MLINE, SHAPE, DIMENSION), with block membership, block names and dimension blocks, and the 3D polylines and meshes. R13/R14 have no lineweights and no layer plot flag, and their viewports state a window but no view
- AC1021/AC1024/AC1027/AC1032 use native decode for LINE/ARC/LWPOLYLINE/POINT/CIRCLE/ELLIPSE and are regression-tested against paired DXF samples
- TEXT/MTEXT/ATTRIB/ATTDEF of R2007 and later are read from the string stream of the object and regression-tested on one drawing saved in every version (R14 to R2018)
- AC1021/AC1024/AC1027/AC1032 entity style/layer color resolution is currently best-effort on some files
- Ordinary legacy 2D/3D polylines have native roundtrip tests and opt-in ODA import/export coverage for AC1015/AC1018/AC1027; fitted polylines and mesh writing remain unsupported
- ARC angles in raw API are **radians** (high‑level API converts to degrees)
