# CLI Reference

ezdwg provides a command-line interface for inspecting and converting DWG files.

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
