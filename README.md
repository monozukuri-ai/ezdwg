# ezdwg
[![PyPI version](https://badge.fury.io/py/ezdwg.svg)](https://badge.fury.io/py/ezdwg)

Minimal DWG (R14-R2018 / AC1014-AC1032) reader with a Python API inspired by ezdxf.
This project is **DWG read-focused** today and focuses on a simple, friendly API.

## Status
- High-level API (`ezdwg.read`): **R14 / AC1014** (experimental), **R2000 / AC1015**, **R2004 / AC1018**, **R2007 / AC1021**, **R2010 / AC1024**, **R2013 / AC1027**, **R2018 / AC1032**
- Raw API (`ezdwg.raw`): **R14 / AC1014** (version detect + object headers), **R2000 / AC1015**, **R2004 / AC1018**, **R2007 / AC1021**, plus native **AC1024/AC1027/AC1032** support for object listing and `LINE`/`ARC`/`LWPOLYLINE` decode
- High-level entities: **LINE**, **ARC**, **LWPOLYLINE**, **POINT**, **CIRCLE**, **ELLIPSE**, **TEXT**, **MTEXT**, **DIMENSION** (linear + radius + diameter)
- Additional raw decode: **INSERT** (+ low-level POLYLINE/VERTEX helpers)
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

## Quick Start
```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
msp = doc.modelspace()

for e in msp.query("LINE LWPOLYLINE ARC CIRCLE ELLIPSE POINT TEXT MTEXT DIMENSION"):
    print(e.dxftype, e.handle, e.dxf)
```

Plot in matplotlib:

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
doc.plot()
```

## CLI
```bash
ezdwg --version
ezdwg inspect examples/data/line_2000.dwg
ezdwg convert examples/data/line_2000.dwg /tmp/line_2000_out.dxf
ezdwg convert examples/data/arc_2000.dwg /tmp/arc_2000_out.dxf --types "ARC" --dxf-version R2010
```

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
- Read‑only
- High-level API supports R14 (AC1014, experimental), R2000 (AC1015), R2004 (AC1018), R2007 (AC1021), R2010 (AC1024), R2013 (AC1027), and R2018 (AC1032)
- AC1014 currently has stable version detection/object-header listing; entity geometry decoding coverage is limited
- AC1021/AC1024/AC1027/AC1032 use native decode for LINE/ARC/LWPOLYLINE/POINT/CIRCLE/ELLIPSE and are regression-tested against paired DXF samples
- TEXT/MTEXT/DIMENSION decoders now use version-aware common header paths internally; R2007+ dedicated sample regression coverage is still pending
- AC1021/AC1024/AC1027/AC1032 entity style/layer color resolution is currently best-effort on some files
- Legacy `POLYLINE/VERTEX/SEQEND` samples are not yet covered in AC1018 test data
- ARC angles in raw API are **radians** (high‑level API converts to degrees)
