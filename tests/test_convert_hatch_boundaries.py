"""to_dxf writes HATCH boundaries as the DWG stores them: edges and bulged polylines, not flattened points."""

from __future__ import annotations

import math
from pathlib import Path

import pytest

import ezdwg
import ezdwg.convert as convert_module
import ezdwg.document as document_module
from tests._dxf_helpers import dxf_entities_of_type

ezdxf = pytest.importorskip("ezdxf")
recover = pytest.importorskip("ezdxf.recover")

SAMPLES = Path(__file__).resolve().parents[1] / "test_dwg"


def _hatch_groups(path: Path) -> list[tuple[str, str]]:
    (hatch,) = dxf_entities_of_type(path, "HATCH")
    return hatch["groups"]


def _values(groups: list[tuple[str, str]], code: str) -> list[float]:
    return [float(value) for group, value in groups if group == code]


def _convert(tmp_path: Path, paths: list[dict]) -> Path:
    doc = ezdwg.new("AC1015")
    doc.modelspace().add_hatch(paths)
    dwg = tmp_path / "hatch.dwg"
    doc.save(dwg)
    out = tmp_path / "hatch.dxf"
    ezdwg.to_dxf(str(dwg), str(out))
    auditor = recover.readfile(out)[1]
    assert not auditor.has_errors
    return out


def test_arc_edges_keep_the_stored_angles_and_direction(tmp_path) -> None:
    out = _convert(tmp_path, [{"edges": [
        {"type": "line", "start": (5, 0), "end": (5, 4)},
        {"type": "arc", "center": (3, 4), "radius": 2, "start_angle": 30, "end_angle": 120, "ccw": False},
        {"type": "arc", "center": (0, 0), "radius": 1, "start_angle": 10, "end_angle": 80, "ccw": True},
    ]}])
    groups = _hatch_groups(out)
    assert _values(groups, "72") == [1.0, 2.0, 2.0]  # edge types: no polyline path
    assert _values(groups, "50") == pytest.approx([30.0, 10.0])
    assert _values(groups, "51") == pytest.approx([120.0, 80.0])
    assert _values(groups, "40") == [2.0, 1.0]
    assert [value for group, value in groups if group == "73"][-2:] == ["0", "1"]


def test_ellipse_and_spline_edges_are_written_as_stored(tmp_path) -> None:
    knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]
    out = _convert(tmp_path, [{"edges": [
        {"type": "ellipse", "center": (0, 0), "major_axis": (5, 0), "ratio": 0.25,
         "start_angle": -60, "end_angle": 90, "ccw": True},
        {"type": "spline", "degree": 2, "rational": True, "knots": knots,
         "control_points": [(0, 5), (1, 7), (2, 5)], "weights": [1.0, 0.5, 1.0]},
    ]}])
    groups = _hatch_groups(out)
    assert _values(groups, "72") == [3.0, 4.0]
    assert _values(groups, "11")[0] == 5.0 and _values(groups, "40")[0] == 0.25
    # the DWG stores the parameters -60 and 90 degrees; DXF the angles they reach
    start = math.degrees(math.atan2(0.25 * math.sin(math.radians(-60)), math.cos(math.radians(-60))))
    assert _values(groups, "50") == pytest.approx([start])
    assert _values(groups, "51") == pytest.approx([90.0])
    assert _values(groups, "94") == [2.0]
    assert _values(groups, "40")[1:7] == knots
    assert _values(groups, "42") == [1.0, 0.5, 1.0]


def test_bulged_polyline_path_keeps_vertices_and_bulges(tmp_path) -> None:
    vertices = [(0.0, 0.0), (4.0, 0.0), (2.0, 3.0)]
    out = _convert(tmp_path, [{"points": vertices, "bulges": [0.5, 0.0, 0.0], "closed": True}])
    groups = _hatch_groups(out)
    assert _values(groups, "93") == [3.0]
    assert _values(groups, "42") == [0.5, 0.0, 0.0]


def test_hatch_without_flattened_points_is_written_from_its_boundary() -> None:
    # A hatch over the flattening budget decodes with empty points but a full boundary.
    doc = ezdxf.new("R2010")
    edges = [("line", (float(i), 0.0), (float(i + 1), 0.0)) for i in range(3)]
    edges.append(("arc", (1.5, 0.0), 1.5, 0.0, math.pi, True))
    dxf = {"solid_fill": True, "paths": [{"closed": True, "points": [], "boundary": ("edges", edges)}]}
    assert convert_module._write_hatch(doc.modelspace(), dxf, {})
    (path,) = doc.modelspace().query("HATCH")[0].paths
    assert [edge.EDGE_TYPE for edge in path.edges] == ["LineEdge"] * 3 + ["ArcEdge"]


def test_unusable_boundary_falls_back_to_the_flattened_points() -> None:
    doc = ezdxf.new("R2010")
    square = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 0.0, 0.0)]
    bad = ("edges", [("line", (0.0, 0.0), (math.nan, 0.0))])
    dxf = {"solid_fill": True, "paths": [{"closed": True, "points": square, "boundary": bad}]}
    assert convert_module._write_hatch(doc.modelspace(), dxf, {})
    (path,) = doc.modelspace().query("HATCH")[0].paths
    assert len(path.vertices) == 4


def test_boundary_is_attached_only_when_the_path_counts_match(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_hatch_entities",
        lambda _path: [
            (1, "SOLID", True, False, 0.0, (0.0, 0.0, 1.0), [(True, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])]),
            (2, "SOLID", True, False, 0.0, (0.0, 0.0, 1.0), [(True, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])]),
        ],
    )
    polyline = ("polyline", True, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)], [])
    monkeypatch.setattr(
        document_module.raw, "decode_hatch_boundaries", lambda _path: [(1, [polyline]), (2, [polyline, polyline])]
    )
    doc = document_module.Document(path="dummy_hatch.dwg", version="AC1018")
    first, second = doc.modelspace().query("HATCH")
    assert first.dxf["paths"][0]["boundary"] == polyline
    assert "boundary" not in second.dxf["paths"][0]


def test_sample_edge_boundaries_are_written_as_edge_paths(tmp_path) -> None:
    out = tmp_path / "sample.dxf"
    ezdwg.to_dxf(str(SAMPLES / "acadsharp/sample_AC1032.dwg"), str(out))
    hatches = ezdxf.readfile(out).modelspace().query("HATCH")
    kinds = {type(path).__name__ for hatch in hatches for path in hatch.paths}
    assert kinds == {"PolylinePath", "EdgePath"}


def test_clockwise_ellipse_edge_keeps_its_direction(tmp_path) -> None:
    out = _convert(tmp_path, [{"edges": [
        {"type": "ellipse", "center": (0, 0), "major_axis": (4, 0), "ratio": 0.5,
         "start_angle": 30, "end_angle": 120, "ccw": False},
        {"type": "line", "start": (-2, 1.7), "end": (3.4, 0.5)},
    ]}])
    groups = _hatch_groups(out)

    def angle(degrees: float) -> float:
        t = math.radians(degrees)
        return math.degrees(math.atan2(0.5 * math.sin(t), math.cos(t)))

    assert _values(groups, "50") == pytest.approx([angle(30)])
    assert _values(groups, "51") == pytest.approx([angle(120)])
    assert [value for group, value in groups if group == "73"][-1] == "0"


@pytest.mark.parametrize(
    "edge",
    [
        ("ellipse", (0.0, 0.0), (1.0, 0.0), 2.0, 0.0, math.pi, True),
        ("ellipse", (0.0, 0.0), (0.0, 0.0), 0.5, 0.0, math.pi, True),
        ("arc", (0.0, 0.0), 0.0, 0.0, math.pi, True),
    ],
    ids=["ratio-2", "zero-major-axis", "zero-radius"],
)
def test_invalid_edge_falls_back_without_a_partial_edge_path(edge) -> None:
    doc = ezdxf.new("R2010")
    square = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 0.0, 0.0)]
    boundary = ("edges", [("line", (0.0, 0.0), (1.0, 0.0)), edge])
    dxf = {"solid_fill": True, "paths": [{"closed": True, "points": square, "boundary": boundary}]}
    assert convert_module._write_hatch(doc.modelspace(), dxf, {})
    (path,) = doc.modelspace().query("HATCH")[0].paths
    assert len(path.vertices) == 4


def test_strict_to_dwg_leaves_out_a_hatch_over_the_flattening_budget(monkeypatch, tmp_path) -> None:
    doc = ezdwg.new("AC1015")
    for x in (0, 5):
        doc.modelspace().add_hatch([[(x, 0), (x + 1, 0), (x + 1, 1)]])
    source = str(tmp_path / "two_hatches.dwg")
    doc.save(source)
    first, second = sorted(row[0] for row in document_module.raw.decode_hatch_entities(source))[:2]
    square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
    rows = [
        (first, "SOLID", True, False, 0.0, (0.0, 0.0, 1.0), [(True, square)]),
        (second, "SOLID", True, False, 0.0, (0.0, 0.0, 1.0), [(True, [])]),
    ]
    edges = ("edges", [("line", (0.0, 0.0), (1.0, 0.0))])
    monkeypatch.setattr(document_module.raw, "decode_hatch_entities", lambda _path: rows)
    monkeypatch.setattr(
        document_module.raw,
        "decode_hatch_boundaries",
        lambda _path: [(first, [("polyline", True, square, [])]), (second, [edges])],
    )
    result = ezdwg.to_dwg(source, str(tmp_path / "out.dwg"), types="HATCH", strict=True)
    assert result.written_entities == 1


def test_to_dwg_counts_a_hatch_whose_empty_path_has_an_empty_boundary(monkeypatch, tmp_path) -> None:
    # An empty edge path is not the flattening budget's doing: the hatch is not left out silently.
    doc = ezdwg.new("AC1015")
    doc.modelspace().add_hatch([[(0, 0), (1, 0), (1, 1)]])
    source = str(tmp_path / "one_hatch.dwg")
    doc.save(source)
    (handle,) = [row[0] for row in document_module.raw.decode_hatch_entities(source)]
    square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
    monkeypatch.setattr(
        document_module.raw,
        "decode_hatch_entities",
        lambda _path: [(handle, "SOLID", True, False, 0.0, (0.0, 0.0, 1.0), [(True, square), (True, [])])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_hatch_boundaries",
        lambda _path: [(handle, [("polyline", True, square, []), ("edges", [])])],
    )
    result = ezdwg.to_dwg(source, str(tmp_path / "out.dwg"), types="HATCH")
    assert result.total_entities == 1
