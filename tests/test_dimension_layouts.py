"""Type-specific dimension layouts (ODA spec 20.4.23-20.4.29) against DXF ground truth.

The ACadSharp ``sample_AC10xx.dwg`` fixtures contain every dimension subtype. The
expected values below were read from the paired ``sample_AC10xx_ascii.dxf`` files
(identical across AC1018/AC1021/AC1027/AC1032) and are used to verify that ANG2LN,
ANG3PT, ALIGNED, ORDINATE, RADIUS and DIAMETER are decoded with their own tail
instead of the LINEAR one (which shifted every point for those types).
"""

from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
from ezdwg import raw

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = {
    "AC1018": ROOT / "test_dwg/acadsharp/sample_AC1018.dwg",
    "AC1021": ROOT / "test_dwg/acadsharp/sample_AC1021.dwg",
    "AC1027": ROOT / "test_dwg/acadsharp/sample_AC1027.dwg",
    "AC1032": ROOT / "test_dwg/acadsharp/sample_AC1032.dwg",
}

# handle -> (dimtype, defpoint(10), defpoint2(13), defpoint3(14), defpoint4(15),
#            defpoint5(16), text_midpoint(11), actual_measurement)
EXPECTED = {
    0x514: (
        "LINEAR",
        (360.3581, 44.291307),
        (330.289059, 2.941179),
        (364.487264, 37.139384),
        None,
        None,
        (339.065951, 34.454774),
        46.715616708,
    ),
    0x527: (
        "ALIGNED",
        (410.084871, 48.538786),
        (381.586367, 8.64088),
        (415.784572, 42.839085),
        None,
        None,
        (390.757369, 33.668083),
        48.363565231,
    ),
    0x515: (
        "ANG3PT",
        (472.36045, 22.158063),
        (455.682478, 8.64088),
        (444.283076, 37.139384),
        (438.583375, 14.340581),
        None,
        (470.642041, 31.939054),
        1.647568218,
    ),
    0x516: (
        "ANG2LN",
        (512.679486, 8.64088),
        (495.580383, 14.340581),
        (501.280084, 37.139384),
        (495.580383, 14.340581),
        (528.64747, 21.993741),
        (527.000218, 31.58837),
        1.647568218,
    ),
    0x525: (
        "ORDINATE",
        (313.189957, 111.235495),
        (660.871707, 8.64088),
        (700.769613, 14.340581),
        None,
        None,
        (706.468794, 16.467969),
        102.594614812,
    ),
    0x526: (
        "ORDINATE",
        (313.189957, 111.235495),
        (666.571408, 2.941179),
        (677.97081, 42.839085),
        None,
        None,
        (675.843422, 48.655081),
        353.381451018,
    ),
    0x51F: (
        "RADIUS",
        (569.676494, 25.739983),
        None,
        None,
        (577.737088, 33.800577),
        None,
        (583.348482, 42.420551),
        11.399401646,
    ),
    0x522: (
        "DIAMETER",
        (603.874699, 25.739983),
        None,
        None,
        (649.472306, 25.739983),
        None,
        (626.673502, 25.739983),
        45.597606583,
    ),
}


def _assert_point(actual, expected, label: str) -> None:
    if expected is None:
        return
    assert actual is not None, f"{label}: missing"
    assert actual[0] == pytest.approx(expected[0], abs=1e-4), label
    assert actual[1] == pytest.approx(expected[1], abs=1e-4), label
    if len(actual) > 2:
        assert actual[2] == pytest.approx(0.0, abs=1e-6), label


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_dimension_subtypes_match_dxf_ground_truth(version: str) -> None:
    path = SAMPLES[version]
    assert path.exists(), f"missing sample: {path}"
    doc = ezdwg.read(str(path))
    by_handle = {int(e.handle): e.dxf for e in doc.entities().query("DIMENSION")}
    for handle, (dimtype, p10, p13, p14, p15, p16, mid, measurement) in EXPECTED.items():
        dxf = by_handle.get(handle)
        assert dxf is not None, f"{version}: dimension {handle:#x} not decoded"
        label = f"{version} {handle:#x} {dimtype}"
        assert dxf["dimtype"] == dimtype, label
        _assert_point(dxf.get("defpoint"), p10, f"{label} defpoint")
        _assert_point(dxf.get("defpoint2"), p13, f"{label} defpoint2")
        _assert_point(dxf.get("defpoint3"), p14, f"{label} defpoint3")
        _assert_point(dxf.get("defpoint4"), p15, f"{label} defpoint4")
        _assert_point(dxf.get("defpoint5"), p16, f"{label} defpoint5")
        _assert_point(dxf.get("text_midpoint"), mid, f"{label} text_midpoint")
        assert dxf.get("actual_measurement") == pytest.approx(measurement, abs=1e-6), label
        if dimtype not in ("ANG2LN", "ANG3PT", "RADIUS", "DIAMETER"):
            assert dxf.get("defpoint4") is None, label
        if dimtype != "ANG2LN":
            assert dxf.get("defpoint5") is None, label


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_raw_dimension_rows_carry_extra_points(version: str) -> None:
    path = str(SAMPLES[version])
    rows = {row[0]: row for row in raw.decode_dim_ang2ln_entities(path)}
    row = rows[0x516]
    assert len(row) == 12
    point15, point16 = row[11]
    _assert_point(point15, EXPECTED[0x516][4], "ang2ln point15")
    assert len(point16) == 2
    _assert_point(point16, EXPECTED[0x516][5], "ang2ln point16")

    rows = {row[0]: row for row in raw.decode_dim_ang3pt_entities(path)}
    point15, point16 = rows[0x515][11]
    _assert_point(point15, EXPECTED[0x515][4], "ang3pt point15")
    assert point16 is None

    rows = {row[0]: row for row in raw.decode_dim_linear_entities(path)}
    assert rows[0x514][11] == (None, None)

    rows = {row[0]: row for row in raw.decode_dim_radius_entities(path)}
    point15, point16 = rows[0x51F][11]
    _assert_point(point15, EXPECTED[0x51F][4], "radius point15")
    assert point16 is None
