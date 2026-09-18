# CLI Reference

ezdwg provides a command-line interface for inspecting, plotting, converting, and writing DWG files.

## Version

```bash
ezdwg --version
```

## Inspect

Show information about a DWG file:

```bash
ezdwg inspect path/to/file.dwg
```

Output includes:

- File path
- DWG version (e.g. `AC1015`)
- Decode version
- Total entity count
- Per-type entity counts

### Verbose Mode

Show expanded diagnostics:

```bash
ezdwg inspect path/to/file.dwg --verbose
```

Verbose mode shows additional details for diagnostic entity types and expands the number of unknown handle/type-code entries reported.

### Example Output

```
file: examples/data/line_2000.dwg
version: AC1015
decode_version: AC1015
total_entities: 3
LINE: 3
```

## Plot

Display model-space entities (requires `ezdwg[plot]`):

```bash
ezdwg plot path/to/file.dwg
```

The command opens a matplotlib window when an interactive backend is available.
Otherwise (for example, if Tk/Qt is unavailable or `MPLBACKEND=Agg` is set), it
saves an SVG preview in the OS temporary directory and opens it in your browser.
The printed `preview:` URL can also be opened manually. The preview remains
after the command exits; it can be deleted when no longer needed. If no browser
can be started, the command reports the preview URL and exits with code `2`.

The preview uses a 12×8-inch canvas with coordinate axes hidden, thin geometry
strokes, and thinner gray dimensions. Bright colors are adapted for the white
background. Small crosses indicate block-reference positions whose geometry
is not yet expanded; they are not circular features in the DWG.

Save without opening a viewer, including on machines without a display:

```bash
ezdwg plot path/to/file.dwg -o drawing.png
ezdwg plot path/to/file.dwg -o drawing.svg --types "LINE ARC"
ezdwg plot path/to/file.dwg -o drawing.pdf --title "My drawing"
```

The output extension selects the format using matplotlib's supported formats.
Existing output files are overwritten. Rendering uses the same entity coverage
and behavior as [`ezdwg.plot()`](plotting.md).

### Options

| Option | Description |
|--------|-------------|
| `-o`, `--output` | Save to a file instead of opening a window or browser |
| `--types` | Entity filter (e.g. `"LINE ARC LWPOLYLINE"`) |
| `--dpi` | Saved image resolution, a positive integer (default: `150`) |
| `--title` | Drawing title |

In a local checkout, install the plotting extra and run with uv:

```bash
uv run --extra plot ezdwg plot examples/data/line_2000.dwg
uv run --extra plot ezdwg plot examples/data/line_2000.dwg -o drawing.png --dpi 200
```

## Convert

Convert a DWG file to DXF:

```bash
ezdwg convert input.dwg output.dxf
```

### Options

| Option | Description |
|--------|-------------|
| `--types` | Entity filter (e.g. `"LINE ARC LWPOLYLINE"`) |
| `--dxf-version` | Output DXF version (default: `R2010`) |
| `--strict` | Fail if any entity cannot be converted |
| `--include-unsupported` | Also query unsupported entity types |

### Examples

```bash
# Convert with type filter
ezdwg convert input.dwg output.dxf --types "ARC LINE"

# Specify DXF version
ezdwg convert input.dwg output.dxf --dxf-version R2018

# Strict mode
ezdwg convert input.dwg output.dxf --strict
```

### Example Output

```
input: input.dwg
output: output.dxf
total_entities: 15
written_entities: 12
skipped_entities: 3
skipped[VIEWPORT]: 3
```

## Write

Write a DWG file using the native AC1015 writer:

```bash
ezdwg write input.dwg output.dwg
```

### Options

| Option | Description |
|--------|-------------|
| `--types` | Entity filter (e.g. `"LINE ARC LWPOLYLINE"`) |
| `--dwg-version` | Output DWG version (default: `AC1015`, currently only supported value) |
| `--strict` | Fail if any entity cannot be written |

### Examples

```bash
# Write only selected entity types
ezdwg write input.dwg output.dwg --types "LINE MTEXT"

# Explicit target version
ezdwg write input.dwg output.dwg --dwg-version AC1015

# Strict mode
ezdwg write input.dwg output.dwg --strict
```

### Example Output

```
input: input.dwg
output: output.dwg
target_version: AC1015
total_entities: 8
written_entities: 7
skipped_entities: 1
skipped[POINT]: 1
```
