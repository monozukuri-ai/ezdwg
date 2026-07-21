from __future__ import annotations

import math
from pathlib import Path

import pytest

import ezdwg
from ezdwg import raw

ROOT = Path(__file__).resolve().parents[1]

# Expected values were cross-checked against the paired DXF headers
# ($INSUNITS / $LUNITS / $AUNITS / $EXTMIN) for every supported version family.
SAMPLES = [
    ("examples/data/line_2000.dwg", "AC1015", 4, 2),
    ("examples/data/arc_2000.dwg", "AC1015", 4, 2),
    ("examples/data/mtext_2000.dwg", "AC1015", 4, 2),
    ("examples/data/insert_2004.dwg", "AC1018", 0, 2),
    ("examples/data/mechanical_example-imperial.dwg", "AC1024", 1, 4),
    ("test_dwg/acadsharp/sample_AC1027.dwg", "AC1027", 1, 2),
    ("test_dwg/acadsharp/sample_AC1032.dwg", "AC1032", 1, 2),
    ("test_dwg/acadsharp/BLOCKPOINTPARAMETER.dwg", "AC1032", 1, 2),
]


@pytest.mark.parametrize(("relative", "version", "insunits", "lunits"), SAMPLES)
def test_header_variables_across_versions(
    relative: str, version: str, insunits: int, lunits: int
) -> None:
    path = ROOT / relative
    doc = ezdwg.read(str(path))
    assert doc.version == version

    variables = doc.header_variables()
    assert variables["insunits"] == insunits
    assert variables["lunits"] == lunits
    assert variables["aunits"] == 0
    assert variables["ltscale"] == pytest.approx(1.0)
    assert variables["textsize"] is not None and variables["textsize"] > 0.0
    for key in ("extmin", "extmax"):
        point = variables[key]
        assert point is not None and len(point) == 3
        assert all(math.isfinite(value) for value in point)
    for key in ("limmin", "limmax"):
        point = variables[key]
        assert point is not None and len(point) == 2
        assert all(math.isfinite(value) for value in point)


def test_units_property_maps_insunits() -> None:
    metric = ezdwg.read(str(ROOT / "examples/data/line_2000.dwg"))
    assert metric.insunits == 4
    assert metric.units == "millimeters"

    imperial = ezdwg.read(str(ROOT / "examples/data/mechanical_example-imperial.dwg"))
    assert imperial.insunits == 1
    assert imperial.units == "inches"

    unitless = ezdwg.read(str(ROOT / "examples/data/insert_2004.dwg"))
    assert unitless.insunits == 0
    assert unitless.units == "unitless"


def test_raw_row_shape() -> None:
    row = raw.decode_header_variables(str(ROOT / "examples/data/line_2000.dwg"))
    assert len(row) == 11
    assert row[0] == 4


def test_missing_file_raises() -> None:
    with pytest.raises(Exception):
        raw.decode_header_variables(str(ROOT / "examples/data/no_such_file.dwg"))
