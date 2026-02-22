# Plotting

ezdwg can render DWG entities using matplotlib. Install the plotting extra:

```bash
pip install "ezdwg[plot]"
```

## Basic Usage

### From a Document

```python
import ezdwg

doc = ezdwg.read("drawing.dwg")
doc.plot()
```

### From a Layout

```python
msp = doc.modelspace()
msp.plot()
```

### From a File Path

```python
import ezdwg

ezdwg.plot("drawing.dwg")
```

## Parameters

The `plot()` function accepts the following parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `types` | `str \| None` | `None` | Entity type filter (e.g. `"LINE ARC"`) |
| `ax` | `matplotlib.axes.Axes \| None` | `None` | Existing axes to draw on |
| `show` | `bool` | `True` | Call `plt.show()` after drawing |
| `equal` | `bool` | `True` | Use equal aspect ratio |
| `title` | `str \| None` | `None` | Plot title |
| `line_width` | `float` | `1.0` | Line width for geometry |
| `arc_segments` | `int` | `64` | Segments for arc approximation |
| `auto_fit` | `bool` | `True` | Auto-fit view to content |
| `fit_margin` | `float` | `0.04` | Margin around content (fraction) |
| `dimension_color` | `Any \| None` | `"black"` | Color for dimension entities |

## Saving to a File

Set `show=False` and use matplotlib to save:

```python
import matplotlib.pyplot as plt
import ezdwg

doc = ezdwg.read("drawing.dwg")
ax = doc.plot(show=False)
plt.savefig("output.png", dpi=150, bbox_inches="tight")
```

## Plotting Specific Types

```python
# Only lines and arcs
doc.plot(types="LINE ARC")

# Only text entities
doc.plot(types="TEXT MTEXT")
```

## Custom Axes

```python
import matplotlib.pyplot as plt

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 8))

doc = ezdwg.read("drawing.dwg")
doc.plot(types="LINE", ax=ax1, show=False, title="Lines")
doc.plot(types="ARC CIRCLE", ax=ax2, show=False, title="Arcs & Circles")

plt.tight_layout()
plt.show()
```

## Color Handling

ezdwg resolves entity colors from:

1. **True color** (24-bit RGB) — if present on the entity
2. **ACI color index** — AutoCAD Color Index (1–255)
3. **Layer color** — inherited from the entity's layer

Colors are applied automatically when plotting. ACI index 7 (white/black) is rendered as black for visibility on matplotlib's default light background.
