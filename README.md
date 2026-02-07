# ezdwg

Minimal DWG (R2000-R2010 / AC1015-AC1024) reader with a Python API inspired by ezdxf.
This project is **read-only** today and focuses on a simple, friendly API.

## Status
- High-level API (`ezdwg.read`): **R2000 / AC1015**, **R2004 / AC1018**, **R2010 / AC1024** (compat mode)
- Raw API (`ezdwg.raw`): **R2000 / AC1015**, **R2004 / AC1018**
- High-level entities: **LINE**, **ARC**, **LWPOLYLINE**, **POINT**, **CIRCLE**, **ELLIPSE**, **TEXT**, **MTEXT**, **DIMENSION** (linear)
- Additional raw decode: **INSERT** (+ low-level POLYLINE/VERTEX helpers)
- Output units/angles: high-level API returns ARC angles in **degrees**

## Install
Rust toolchain is required (PyO3 build).

```bash
pip install -e .
```

Plotting (optional):

```bash
pip install "ezdwg[plot]"
```

AC1024 compatibility mode requires:

```bash
ODAFileConverter
xvfb-run
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
doc.plot(types="ARC", arc_segments=96)
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
```

## Low-Level API
If you need raw decode access:

```python
from ezdwg import raw

raw.decode_line_entities("path/to/file.dwg")
```

## Limitations
- Read‑only
- High-level API supports R2000 (AC1015), R2004 (AC1018), and R2010 (AC1024)
- AC1024 uses compatibility conversion to AC1018 via `ODAFileConverter` + `xvfb-run`
- Legacy `POLYLINE/VERTEX/SEQEND` samples are not yet covered in AC1018 test data
- ARC angles in raw API are **radians** (high‑level API converts to degrees)
