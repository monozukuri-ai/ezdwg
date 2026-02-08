from __future__ import annotations

from pathlib import Path
from typing import Iterator

import ezdwg
from ezdwg import raw


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


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


def test_r2007plus_point_circle_ellipse_counts_match_paired_dxf() -> None:
    cases = [
        ("point2d_2007.dwg", "POINT"),
        ("point2d_2010.dwg", "POINT"),
        ("point2d_2013.dwg", "POINT"),
        ("point3d_2007.dwg", "POINT"),
        ("point3d_2010.dwg", "POINT"),
        ("point3d_2013.dwg", "POINT"),
        ("circle_2007.dwg", "CIRCLE"),
        ("circle_2010.dwg", "CIRCLE"),
        ("circle_2013.dwg", "CIRCLE"),
        ("ellipse_2007.dwg", "ELLIPSE"),
        ("ellipse_2010.dwg", "ELLIPSE"),
        ("ellipse_2013.dwg", "ELLIPSE"),
    ]

    for dwg_name, expected_type in cases:
        stem = dwg_name.removesuffix(".dwg")
        dxf_count = len(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", expected_type))
        rows = raw.list_object_headers_with_type(str(SAMPLES / dwg_name))
        dwg_count = sum(1 for row in rows if row[4] == expected_type)
        assert dxf_count == 1, f"{stem}.dxf: expected one {expected_type}"
        assert dwg_count == dxf_count, f"{dwg_name}: expected {dxf_count} {expected_type}"


def test_r2007plus_point_geometry_matches_paired_dxf() -> None:
    for stem in ["point2d_2007", "point2d_2010", "point2d_2013", "point3d_2007", "point3d_2010", "point3d_2013"]:
        dwg_path = SAMPLES / f"{stem}.dwg"
        dxf_path = SAMPLES / f"{stem}.dxf"
        points = list(ezdwg.read(str(dwg_path)).modelspace().query("POINT"))
        dxf_points = _dxf_entities_of_type(dxf_path, "POINT")
        assert len(points) == 1
        assert len(dxf_points) == 1
        expected = (
            _group_float(dxf_points[0], "10"),
            _group_float(dxf_points[0], "20"),
            _group_float(dxf_points[0], "30"),
        )
        assert _triplet_close(points[0].dxf["location"], expected)


def test_r2007plus_circle_geometry_matches_paired_dxf() -> None:
    for stem in ["circle_2007", "circle_2010", "circle_2013"]:
        dwg_path = SAMPLES / f"{stem}.dwg"
        dxf_path = SAMPLES / f"{stem}.dxf"
        circles = list(ezdwg.read(str(dwg_path)).modelspace().query("CIRCLE"))
        dxf_circles = _dxf_entities_of_type(dxf_path, "CIRCLE")
        assert len(circles) == 1
        assert len(dxf_circles) == 1
        expected_center = (
            _group_float(dxf_circles[0], "10"),
            _group_float(dxf_circles[0], "20"),
            _group_float(dxf_circles[0], "30"),
        )
        expected_radius = _group_float(dxf_circles[0], "40")
        assert _triplet_close(circles[0].dxf["center"], expected_center)
        assert abs(circles[0].dxf["radius"] - expected_radius) < 1e-9


def test_r2007plus_ellipse_geometry_matches_paired_dxf() -> None:
    for stem in ["ellipse_2007", "ellipse_2010", "ellipse_2013"]:
        dwg_path = SAMPLES / f"{stem}.dwg"
        dxf_path = SAMPLES / f"{stem}.dxf"
        ellipses = list(ezdwg.read(str(dwg_path)).modelspace().query("ELLIPSE"))
        dxf_ellipses = _dxf_entities_of_type(dxf_path, "ELLIPSE")
        assert len(ellipses) == 1
        assert len(dxf_ellipses) == 1
        dxf_ellipse = dxf_ellipses[0]
        expected_center = (
            _group_float(dxf_ellipse, "10"),
            _group_float(dxf_ellipse, "20"),
            _group_float(dxf_ellipse, "30"),
        )
        expected_major_axis = (
            _group_float(dxf_ellipse, "11"),
            _group_float(dxf_ellipse, "21"),
            _group_float(dxf_ellipse, "31"),
        )
        expected_axis_ratio = _group_float(dxf_ellipse, "40")
        expected_start = _group_float(dxf_ellipse, "41")
        expected_end = _group_float(dxf_ellipse, "42")
        ellipse = ellipses[0]
        assert _triplet_close(ellipse.dxf["center"], expected_center)
        assert _triplet_close(ellipse.dxf["major_axis"], expected_major_axis)
        assert abs(ellipse.dxf["axis_ratio"] - expected_axis_ratio) < 1e-9
        assert abs(ellipse.dxf["start_angle"] - expected_start) < 1e-9
        assert abs(ellipse.dxf["end_angle"] - expected_end) < 1e-9
