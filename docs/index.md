# ezdwg

[![PyPI version](https://badge.fury.io/py/ezdwg.svg)](https://badge.fury.io/py/ezdwg)
[![GitHub](https://img.shields.io/github/license/monozukuri-ai/ezdwg)](https://github.com/monozukuri-ai/ezdwg)

**Minimal DWG (R14–R2018 / AC1014–AC1032) reader with a Python API inspired by ezdxf.**

ezdwg is a read-only DWG file parser with a Rust core exposed to Python via PyO3. It provides a simple, friendly API for extracting geometry, text, dimensions, and other entities from DWG files.

## Key Features

- **Read-only DWG parsing** — R14 (AC1014) through R2018 (AC1032)
- **High-performance Rust core** — fast binary parsing via PyO3
- **ezdxf-inspired API** — familiar `Document` / `Layout` / `Entity` pattern
- **Plotting support** — render DWG files with matplotlib
- **DXF export** — convert DWG to DXF using ezdxf as backend
- **CLI tools** — inspect and convert DWG files from the command line

## Quick Example

```python
import ezdwg

doc = ezdwg.read("drawing.dwg")
msp = doc.modelspace()

for entity in msp.query("LINE ARC CIRCLE"):
    print(entity.dxftype, entity.handle, entity.dxf)
```

## Next Steps

- [Installation](getting-started/installation.md) — get ezdwg up and running
- [Quick Start](getting-started/quickstart.md) — learn the basics in 5 minutes
- [User Guide](guide/reading-files.md) — detailed usage guides
- [API Reference](api/index.md) — full API documentation
