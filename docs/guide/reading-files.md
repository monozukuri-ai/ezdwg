# Reading DWG Files

## Opening a File

Use `ezdwg.read()` to open a DWG file:

```python
import ezdwg

doc = ezdwg.read("path/to/file.dwg")
```

This returns a `Document` object. The function detects the DWG version automatically and raises `ValueError` if the version is unsupported.

## Document Properties

```python
doc = ezdwg.read("path/to/file.dwg")

print(doc.version)         # e.g. "AC1015"
print(doc.decode_version)  # e.g. "AC1015"
print(doc.path)            # file path
```

## Accessing the Modelspace

The modelspace contains the main drawing entities:

```python
msp = doc.modelspace()
```

This returns a `Layout` object, which provides `query()` and `iter_entities()` to access entities.

## Supported Versions

| Version Code | AutoCAD Version | Support Level |
|-------------|-----------------|---------------|
| AC1012 | R13 | 2D entities |
| AC1014 | R14 | 2D entities |
| AC1015 | R2000 | Full |
| AC1018 | R2004 | Full |
| AC1021 | R2007 | Full |
| AC1024 | R2010 | Full |
| AC1027 | R2013 | Full |
| AC1032 | R2018 | Full |

!!! note "R13 / R14 Support"
    R13 (AC1012) and R14 (AC1014) files decode the 2D entity types (lines, arcs, circles, ellipses, points, polylines, text, attributes, block references, hatches, solids, splines, leaders, dimensions) with block membership and block names. 3D polylines and meshes are not decoded. These versions have no lineweights, no layer plot flag and no `$INSUNITS`, and their symbol names (layers, blocks) are upper case.

## Lazy Loading

ezdwg uses lazy object loading internally. The `ObjectLocator` maps handles to file offsets, and objects are decoded only when accessed. This makes opening large files fast — entities are parsed on demand as you iterate over them.
