from __future__ import annotations

import math
from pathlib import Path
from typing import Iterator

import ezdwg
from ezdwg import raw


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "dwg_samples"


def _iter_dxf_entities(path: Path) -> Iterator[dict[str, object]]:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    section_name: str | None = None
    expect_section_name = False
    current_entity: dict[str, object] | None = None

    for i in range(0, len(lines) - 1, 2):
        code = lines[i].strip()
        value = lines[i + 1].strip()

        if code == "0":
            if current_entity is not None and section_name == "ENTITIES":
                yield current_entity
                current_entity = None

            if value == "SECTION":
                expect_section_name = True
                continue

            if value == "ENDSEC":
                section_name = None
                continue

            if section_name == "ENTITIES":
                current_entity = {"type": value, "groups": []}
            continue

        if expect_section_name and code == "2":
            section_name = value
            expect_section_name = False
            continue

        if section_name == "ENTITIES" and current_entity is not None:
            groups = current_entity["groups"]
            assert isinstance(groups, list)
            groups.append((code, value))

    if current_entity is not None and section_name == "ENTITIES":
        yield current_entity


def _dxf_entities_of_type(path: Path, entity_type: str) -> list[dict[str, object]]:
    return [entity for entity in _iter_dxf_entities(path) if entity["type"] == entity_type]


def _group_float(entity: dict[str, object], code: str, default: float = 0.0) -> float:
    groups = entity["groups"]
    assert isinstance(groups, list)
    for group_code, raw_value in groups:
        if group_code == code:
            return float(raw_value)
    return default


def _dxf_lwpolyline_points(entity: dict[str, object]) -> list[tuple[float, float, float]]:
    groups = entity["groups"]
    assert isinstance(groups, list)

    points: list[tuple[float, float, float]] = []
    pending_x: float | None = None
    for group_code, raw_value in groups:
        if group_code == "10":
            pending_x = float(raw_value)
            continue
        if group_code == "20" and pending_x is not None:
            points.append((pending_x, float(raw_value), 0.0))
            pending_x = None
    return points


def _triplet_close(
    actual: tuple[float, float, float],
    expected: tuple[float, float, float],
    eps: float = 1e-9,
) -> bool:
    return (
        abs(actual[0] - expected[0]) < eps
        and abs(actual[1] - expected[1]) < eps
        and abs(actual[2] - expected[2]) < eps
    )


def test_ac1024_read_and_inspect_basics() -> None:
    path = SAMPLES / "line_2010.dwg"
    doc = ezdwg.read(str(path))

    assert doc.version == "AC1024"
    assert doc.decode_version == "AC1024"
    assert doc.decode_path == str(path)
    assert len(list(doc.modelspace().query("LINE"))) == 1


def test_ac1024_entity_counts_match_paired_dxf() -> None:
    cases = [
        ("line_2010.dwg", "LINE"),
        ("arc_2010.dwg", "ARC"),
        ("polyline2d_line_2010.dwg", "LWPOLYLINE"),
    ]

    for dwg_name, expected_type in cases:
        stem = dwg_name.removesuffix(".dwg")
        dxf_count = len(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", expected_type))
        rows = raw.list_object_headers_with_type(str(SAMPLES / dwg_name))
        dwg_count = sum(1 for row in rows if row[4] == expected_type)
        assert dxf_count == 1, f"{stem}.dxf: expected one {expected_type}"
        assert dwg_count == dxf_count, f"{dwg_name}: expected {dxf_count} {expected_type}"


def test_ac1024_line_matches_paired_dxf_geometry() -> None:
    dwg_path = SAMPLES / "line_2010.dwg"
    dxf_path = SAMPLES / "line_2010.dxf"

    lines = list(ezdwg.read(str(dwg_path)).modelspace().query("LINE"))
    dxf_lines = _dxf_entities_of_type(dxf_path, "LINE")
    assert len(lines) == 1
    assert len(dxf_lines) == 1
    line = lines[0]
    dxf_line = dxf_lines[0]

    expected_start = (
        _group_float(dxf_line, "10"),
        _group_float(dxf_line, "20"),
        _group_float(dxf_line, "30"),
    )
    expected_end = (
        _group_float(dxf_line, "11"),
        _group_float(dxf_line, "21"),
        _group_float(dxf_line, "31"),
    )

    assert _triplet_close(line.dxf["start"], expected_start)
    assert _triplet_close(line.dxf["end"], expected_end)


def test_ac1024_arc_matches_paired_dxf_geometry() -> None:
    dwg_path = SAMPLES / "arc_2010.dwg"
    dxf_path = SAMPLES / "arc_2010.dxf"

    arcs = list(ezdwg.read(str(dwg_path)).modelspace().query("ARC"))
    dxf_arcs = _dxf_entities_of_type(dxf_path, "ARC")
    assert len(arcs) == 1
    assert len(dxf_arcs) == 1
    arc = arcs[0]
    dxf_arc = dxf_arcs[0]

    expected_center = (
        _group_float(dxf_arc, "10"),
        _group_float(dxf_arc, "20"),
        _group_float(dxf_arc, "30"),
    )
    expected_radius = _group_float(dxf_arc, "40")
    expected_start = _group_float(dxf_arc, "50")
    expected_end = _group_float(dxf_arc, "51")

    assert _triplet_close(arc.dxf["center"], expected_center)
    assert abs(arc.dxf["radius"] - expected_radius) < 1e-9
    assert abs(arc.dxf["start_angle"] - expected_start) < 1e-9
    assert abs(arc.dxf["end_angle"] - expected_end) < 1e-9


def test_ac1024_lwpolyline_matches_paired_dxf_geometry() -> None:
    dwg_path = SAMPLES / "polyline2d_line_2010.dwg"
    dxf_path = SAMPLES / "polyline2d_line_2010.dxf"

    polylines = list(ezdwg.read(str(dwg_path)).modelspace().query("LWPOLYLINE"))
    dxf_polylines = _dxf_entities_of_type(dxf_path, "LWPOLYLINE")
    assert len(polylines) == 1
    assert len(dxf_polylines) == 1
    polyline = polylines[0]
    expected_points = _dxf_lwpolyline_points(dxf_polylines[0])

    actual_points = polyline.dxf["points"]
    assert len(actual_points) == len(expected_points)
    for actual, expected in zip(actual_points, expected_points):
        assert _triplet_close(actual, expected)


def test_ac1024_decode_payload_shape() -> None:
    line_rows = raw.decode_line_entities(str(SAMPLES / "line_2010.dwg"))
    assert len(line_rows) == 1
    line = line_rows[0]
    assert line[0] > 0
    for value in line[1:]:
        assert math.isfinite(value)

    arc_rows = raw.decode_arc_entities(str(SAMPLES / "arc_2010.dwg"))
    assert len(arc_rows) == 1
    arc = arc_rows[0]
    assert arc[0] > 0
    for value in arc[1:]:
        assert math.isfinite(value)

    lw_rows = raw.decode_lwpolyline_entities(str(SAMPLES / "polyline2d_line_2010.dwg"))
    assert len(lw_rows) == 1
    handle, _flags, points = lw_rows[0]
    assert handle > 0
    for x, y in points:
        assert math.isfinite(x)
        assert math.isfinite(y)
