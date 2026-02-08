from __future__ import annotations

import math
from pathlib import Path

import ezdwg
import ezdwg.cli as cli_module
import pytest
from ezdwg import raw

from tests._dxf_helpers import (
    dxf_entities_of_type,
    dxf_lwpolyline_points,
    group_float,
    triplet_close,
)


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "dwg_samples"
CASES = (
    {
        "version": "AC1021",
        "line": "line_2007",
        "arc": "arc_2007",
        "polyline": "polyline2d_line_2007",
    },
    {
        "version": "AC1024",
        "line": "line_2010",
        "arc": "arc_2010",
        "polyline": "polyline2d_line_2010",
    },
    {
        "version": "AC1027",
        "line": "line_2013",
        "arc": "arc_2013",
        "polyline": "polyline2d_line_2013",
    },
)


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_read_and_inspect_basics(case: dict[str, str]) -> None:
    path = SAMPLES / f'{case["line"]}.dwg'
    doc = ezdwg.read(str(path))

    assert doc.version == case["version"]
    assert doc.decode_version == case["version"]
    assert doc.decode_path == str(path)
    assert len(list(doc.modelspace().query("LINE"))) == 1


def test_ac1021_cli_inspect_reports_native_decode_version(capsys) -> None:
    code = cli_module._run_inspect(str(SAMPLES / "line_2007.dwg"))
    captured = capsys.readouterr()
    assert code == 0
    assert "version: AC1021" in captured.out
    assert "decode_version: AC1021" in captured.out


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_entity_counts_match_paired_dxf(case: dict[str, str]) -> None:
    pairs = (
        (case["line"], "LINE"),
        (case["arc"], "ARC"),
        (case["polyline"], "LWPOLYLINE"),
    )
    for stem, expected_type in pairs:
        dxf_count = len(dxf_entities_of_type(SAMPLES / f"{stem}.dxf", expected_type))
        rows = raw.list_object_headers_with_type(str(SAMPLES / f"{stem}.dwg"))
        dwg_count = sum(1 for row in rows if row[4] == expected_type)
        assert dxf_count == 1, f"{stem}.dxf: expected one {expected_type}"
        assert dwg_count == dxf_count, f"{stem}.dwg: expected {dxf_count} {expected_type}"


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_line_matches_paired_dxf_geometry(case: dict[str, str]) -> None:
    dwg_path = SAMPLES / f'{case["line"]}.dwg'
    dxf_path = SAMPLES / f'{case["line"]}.dxf'

    lines = list(ezdwg.read(str(dwg_path)).modelspace().query("LINE"))
    dxf_lines = dxf_entities_of_type(dxf_path, "LINE")
    assert len(lines) == 1
    assert len(dxf_lines) == 1
    line = lines[0]
    dxf_line = dxf_lines[0]

    expected_start = (
        group_float(dxf_line, "10"),
        group_float(dxf_line, "20"),
        group_float(dxf_line, "30"),
    )
    expected_end = (
        group_float(dxf_line, "11"),
        group_float(dxf_line, "21"),
        group_float(dxf_line, "31"),
    )

    assert triplet_close(line.dxf["start"], expected_start)
    assert triplet_close(line.dxf["end"], expected_end)


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_arc_matches_paired_dxf_geometry(case: dict[str, str]) -> None:
    dwg_path = SAMPLES / f'{case["arc"]}.dwg'
    dxf_path = SAMPLES / f'{case["arc"]}.dxf'

    arcs = list(ezdwg.read(str(dwg_path)).modelspace().query("ARC"))
    dxf_arcs = dxf_entities_of_type(dxf_path, "ARC")
    assert len(arcs) == 1
    assert len(dxf_arcs) == 1
    arc = arcs[0]
    dxf_arc = dxf_arcs[0]

    expected_center = (
        group_float(dxf_arc, "10"),
        group_float(dxf_arc, "20"),
        group_float(dxf_arc, "30"),
    )
    expected_radius = group_float(dxf_arc, "40")
    expected_start = group_float(dxf_arc, "50")
    expected_end = group_float(dxf_arc, "51")

    assert triplet_close(arc.dxf["center"], expected_center)
    assert abs(arc.dxf["radius"] - expected_radius) < 1e-9
    assert abs(arc.dxf["start_angle"] - expected_start) < 1e-9
    assert abs(arc.dxf["end_angle"] - expected_end) < 1e-9


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_lwpolyline_matches_paired_dxf_geometry(case: dict[str, str]) -> None:
    dwg_path = SAMPLES / f'{case["polyline"]}.dwg'
    dxf_path = SAMPLES / f'{case["polyline"]}.dxf'

    polylines = list(ezdwg.read(str(dwg_path)).modelspace().query("LWPOLYLINE"))
    dxf_polylines = dxf_entities_of_type(dxf_path, "LWPOLYLINE")
    assert len(polylines) == 1
    assert len(dxf_polylines) == 1
    polyline = polylines[0]
    expected_points = dxf_lwpolyline_points(dxf_polylines[0])

    actual_points = polyline.dxf["points"]
    assert len(actual_points) == len(expected_points)
    for actual, expected in zip(actual_points, expected_points):
        assert triplet_close(actual, expected)


@pytest.mark.parametrize("case", CASES, ids=[case["version"] for case in CASES])
def test_r2007plus_decode_payload_shape(case: dict[str, str]) -> None:
    line_rows = raw.decode_line_entities(str(SAMPLES / f'{case["line"]}.dwg'))
    assert len(line_rows) == 1
    line = line_rows[0]
    assert line[0] > 0
    for value in line[1:]:
        assert math.isfinite(value)

    arc_rows = raw.decode_arc_entities(str(SAMPLES / f'{case["arc"]}.dwg'))
    assert len(arc_rows) == 1
    arc = arc_rows[0]
    assert arc[0] > 0
    for value in arc[1:]:
        assert math.isfinite(value)

    lw_rows = raw.decode_lwpolyline_entities(str(SAMPLES / f'{case["polyline"]}.dwg'))
    assert len(lw_rows) == 1
    handle, _flags, points = lw_rows[0]
    assert handle > 0
    for x, y in points:
        assert math.isfinite(x)
        assert math.isfinite(y)
