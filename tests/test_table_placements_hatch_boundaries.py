"""The acadsharp samples are one drawing saved in each DWG version, so what a decoder reads of it must not depend
on the version it was saved in."""

from __future__ import annotations

import math
from pathlib import Path

import pytest

import ezdwg
from ezdwg import _core, raw

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg/acadsharp"


def sample(version: str) -> str:
    return str(SAMPLES / f"sample_{version}.dwg")


def rounded(value):
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, (list, tuple)):
        return type(value)(rounded(part) for part in value)
    return value


@pytest.mark.parametrize("version", ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"])
def test_tables_are_placed_alike_and_draw_a_table_block(version: str) -> None:
    rows = sorted(_core.decode_table_placements(sample(version)), key=lambda row: rounded(row[1]))
    reference = sorted(_core.decode_table_placements(sample("AC1032")), key=lambda row: rounded(row[1]))
    assert len(rows) == len(reference) == 2
    assert [rounded(row[1:5]) for row in rows] == [rounded(row[1:5]) for row in reference]
    names = {handle: name for handle, name in _core.decode_block_header_names(sample(version))}
    assert all(names.get(row[5], "").upper().startswith("*T") for row in rows)


@pytest.mark.parametrize("version", ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027"])
def test_hatch_boundaries_read_as_stored_alike(version: str) -> None:
    def boundaries(name: str) -> list:
        return sorted(str(rounded(paths)) for _, paths in _core.decode_hatch_boundaries(sample(name)))

    assert boundaries(version) == boundaries("AC1032")
    kinds = {path[0] for _, paths in _core.decode_hatch_boundaries(sample("AC1032")) for path in paths}
    assert kinds == {"polyline", "edges"}


def test_spline_boundary_keeps_the_curve_definition(tmp_path) -> None:
    knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]
    controls = [(0.0, 0.0), (1.0, 2.0), (2.0, 0.0)]
    weights = [1.0, 0.5, 1.0]
    doc = ezdwg.new("AC1015")
    doc.modelspace().add_hatch([{"edges": [
        {"type": "spline", "degree": 2, "rational": True,
         "knots": knots, "control_points": controls, "weights": weights},
        {"type": "line", "start": controls[-1], "end": controls[0]},
    ]}])
    path = tmp_path / "spline_boundary.dwg"
    doc.save(path)
    ((handle, [(kind, edges)]),) = raw.decode_hatch_boundaries(str(path))
    assert kind == "edges"
    assert edges[0] == ("spline", 2, True, False, knots, controls, weights, [], None, None)
    assert edges[1] == ("line", controls[-1], controls[0])
    (hatch,) = ezdwg.read(str(path)).entities().query("HATCH")
    assert hatch.handle == handle
    assert len(hatch.dxf["paths"][0]["points"]) > len(controls)


def test_arc_and_ellipse_boundaries_keep_parameters_and_radian_angles(tmp_path) -> None:
    doc = ezdwg.new("AC1015")
    doc.modelspace().add_hatch([{"edges": [
        {"type": "arc", "center": (3, 4), "radius": 2,
         "start_angle": 30, "end_angle": 120, "ccw": False},
        {"type": "ellipse", "center": (0, 0), "major_axis": (5, 0),
         "ratio": 0.25, "start_angle": -60, "end_angle": 90, "ccw": True},
    ]}])
    path = tmp_path / "curved_boundaries.dwg"
    doc.save(path)
    ((_handle, [(kind, [arc, ellipse])]),) = raw.decode_hatch_boundaries(str(path))
    assert kind == "edges"
    assert arc[:3] == ("arc", (3.0, 4.0), 2.0)
    assert arc[3:5] == pytest.approx((math.radians(30), math.radians(120)))
    assert arc[5] is False
    assert ellipse[:4] == ("ellipse", (0.0, 0.0), (5.0, 0.0), 0.25)
    assert ellipse[4:6] == pytest.approx((math.radians(-60), math.radians(90)))
    assert ellipse[6] is True


def test_polyline_boundary_keeps_bulges_instead_of_sampled_vertices(tmp_path) -> None:
    vertices = [(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)]
    bulges = [0.5, 0.0, 0.0]
    doc = ezdwg.new("AC1015")
    doc.modelspace().add_hatch([{"points": vertices, "bulges": bulges, "closed": True}])
    path = tmp_path / "bulged_boundary.dwg"
    doc.save(path)
    ((_handle, [boundary]),) = raw.decode_hatch_boundaries(str(path))
    assert boundary == ("polyline", True, vertices, bulges)
