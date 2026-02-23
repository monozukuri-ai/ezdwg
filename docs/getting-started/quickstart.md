# Quick Start

## Reading a DWG File

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
print(f"Version: {doc.version}")

msp = doc.modelspace()
```

## Iterating Over Entities

```python
for entity in msp.query("LINE LWPOLYLINE ARC CIRCLE ELLIPSE POINT TEXT MTEXT DIMENSION"):
    print(entity.dxftype, entity.handle, entity.dxf)
```

You can filter by specific types:

```python
# Only lines
for line in msp.query("LINE"):
    print(line.dxf["start"], "->", line.dxf["end"])

# Multiple types
for entity in msp.query("ARC CIRCLE"):
    print(entity.dxftype, entity.dxf["center"], entity.dxf["radius"])
```

## Plotting

Requires the `plot` extra (`pip install "ezdwg[plot]"`).

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
doc.plot()
```

Save to a file instead of displaying:

```python
import matplotlib.pyplot as plt

doc = ezdwg.read("path/to/file.dwg")
ax = doc.plot(show=False)
plt.savefig("output.png", dpi=150, bbox_inches="tight")
```

## DWG to DXF Conversion

Requires the `dxf` extra (`pip install "ezdwg[dxf]"`).

```python
import ezdwg

result = ezdwg.to_dxf("input.dwg", "output.dxf")
print(f"Written: {result.written_entities}/{result.total_entities} entities")
```

Or from an already opened document:

```python
doc = ezdwg.read("input.dwg")
doc.export_dxf("output.dxf")
```

## DWG to DWG (Native AC1015 Writer)

```python
import ezdwg

result = ezdwg.to_dwg("input.dwg", "output.dwg", version="AC1015")
print(f"Written: {result.written_entities}/{result.total_entities} entities")
```

Or from an already opened document:

```python
doc = ezdwg.read("input.dwg")
doc.export_dwg("output.dwg", version="AC1015")
```

## CLI Usage

```bash
# Show version
ezdwg --version

# Inspect a DWG file
ezdwg inspect path/to/file.dwg

# Convert DWG to DXF
ezdwg convert input.dwg output.dxf

# Write DWG (native AC1015 writer)
ezdwg write input.dwg output.dwg --dwg-version AC1015
```
