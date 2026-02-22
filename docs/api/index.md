# API Reference

ezdwg provides two levels of API:

## High-Level API

The high-level API is the recommended way to work with DWG files. It provides a clean `Document` / `Layout` / `Entity` abstraction.

| Module | Description |
|--------|-------------|
| [Core Functions](core.md) | `ezdwg.read()`, `ezdwg.plot()`, `ezdwg.to_dxf()` |
| [Document & Layout](document.md) | `Document` and `Layout` classes |
| [Entity](entity.md) | `Entity` dataclass |

## Raw API

The raw API (`ezdwg.raw`) provides direct access to the Rust decode functions. It returns data as tuples and is useful for performance-critical code or when you need access to fields not exposed by the high-level API.

| Module | Description |
|--------|-------------|
| [Raw API](raw.md) | Low-level decode functions |

!!! note "Angle Units"
    The high-level API returns ARC angles in **degrees**. The raw API returns angles in **radians**.
