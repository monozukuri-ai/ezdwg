# Plotting

ezdwg can render DWG entities using matplotlib. Install the plotting extra:

```bash
pip install "ezdwg[plot]"
```

## Basic Usage

### From the Command Line

```bash
ezdwg plot drawing.dwg
ezdwg plot drawing.dwg -o drawing.png --dpi 200
ezdwg plot drawing.dwg -o drawing.svg --types "LINE ARC"
```

Without `-o`, the command opens an interactive matplotlib window, or an SVG
preview in your browser if no interactive backend is available. With `-o`, it
saves the drawing without opening a viewer. See the [CLI reference](cli.md#plot).

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
| `line_width` | `float` | `0.5` | Line width for geometry, in points |
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
On near-white backgrounds, very bright colors are darkened while retaining
their hue so yellow, cyan, and green geometry remains visible. Entity color
values in the document are unchanged.

## Drawing Presentation

Geometry uses thin 0.5-point strokes by default. Dimension strokes are thinner
than geometry; their ticks and extension overshoot follow text height rather
than the measured length. Dimension text uses its saved attachment point.
These are preview styles, not a reproduction of DWG lineweight or plot styles.

Automatic fitting keeps the drawing's rectangular bounds while maintaining
equal X/Y scale. The CLI uses a larger canvas with coordinate axes hidden and
gray dimensions. Python callers can continue to customize the returned Axes.

## Text Size and Dimensions

Drawing text is rendered as vector outlines in drawing coordinates. Capital
height follows the DWG text height, so text scales together with geometry when
zooming, resizing, or saving at a different DPI. Text outlines and backgrounds
are included in automatic view bounds, including for text-only drawings.
These outlines are matplotlib patches rather than entries in `ax.texts`.

Dimension text uses the saved height from its referenced anonymous block when
the block's decoded text has one consistent, positive height. The high-level
entity exposes this as `char_height`, with `char_height_source="anonymous_block"`.
If no height can be resolved, the plot uses a fallback of one drawing unit;
it does not infer text height from the dimension length. This does not regenerate
dimension styles or fully reproduce CAD text layout and fonts.

Block references (`INSERT` / `MINSERT`) are currently shown as small, faint
crosses at their insertion points, rather than filled circles. Their block
geometry is not expanded by the plotting API. Actual `POINT` entities use tiny
dots; actual `CIRCLE` entities retain their decoded radius.
