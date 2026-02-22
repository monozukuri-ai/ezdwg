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
| AC1014 | R14 | Experimental |
| AC1015 | R2000 | Full |
| AC1018 | R2004 | Full |
| AC1021 | R2007 | Full |
| AC1024 | R2010 | Full |
| AC1027 | R2013 | Full |
| AC1032 | R2018 | Full |

!!! note "AC1014 Support"
    R14 (AC1014) has stable version detection and object-header listing, but entity geometry decoding coverage is limited.

## Lazy Loading

ezdwg uses lazy object loading internally. The `ObjectLocator` maps handles to file offsets, and objects are decoded only when accessed. This makes opening large files fast — entities are parsed on demand as you iterate over them.
