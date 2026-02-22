# Installation

## From PyPI

The simplest way to install ezdwg:

```bash
pip install ezdwg
```

### Optional Dependencies

**Plotting** (matplotlib):

```bash
pip install "ezdwg[plot]"
```

**DWG to DXF conversion** (ezdxf backend):

```bash
pip install "ezdwg[dxf]"
```

**Both extras**:

```bash
pip install "ezdwg[plot,dxf]"
```

## From Source

Building from source requires a Rust toolchain (stable) and Python >= 3.10.

```bash
git clone https://github.com/monozukuri-ai/ezdwg.git
cd ezdwg
maturin develop
pip install -e .
```

!!! note "Rust Toolchain"
    Install Rust via [rustup](https://rustup.rs/) if you don't have it.
    ezdwg uses [PyO3](https://pyo3.rs/) and [maturin](https://www.maturin.rs/) to build the native extension.

## Requirements

- Python >= 3.10
- Supported platforms: Linux, macOS, Windows
