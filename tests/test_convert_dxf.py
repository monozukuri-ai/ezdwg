from __future__ import annotations

from pathlib import Path
import math

import pytest

import ezdwg
import ezdwg.cli as cli_module
import ezdwg.convert as convert_module
import ezdwg.document as document_module
from tests._dxf_helpers import dxf_entities_of_type, group_float
from tests._dxf_helpers import dxf_lwpolyline_points


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def test_to_dxf_writes_line_entity(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "line_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "line_2007.dwg"),
        str(output),
        types="LINE",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert len(dxf_entities_of_type(output, "LINE")) == 1


def test_document_export_dxf_writes_arc_angles(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    source = SAMPLES / "arc_2007.dwg"
    output = tmp_path / "arc_out.dxf"

    doc = ezdwg.read(str(source))
    source_arc = next(doc.modelspace().query("ARC")).dxf
    result = doc.export_dxf(str(output), types="ARC")

    assert result.total_entities == 1
    arcs = dxf_entities_of_type(output, "ARC")
    assert len(arcs) == 1
    out_arc = arcs[0]
    assert abs(group_float(out_arc, "50") - float(source_arc["start_angle"])) < 1.0e-6
    assert abs(group_float(out_arc, "51") - float(source_arc["end_angle"])) < 1.0e-6


def test_cli_convert_writes_lwpolyline(tmp_path: Path, capsys) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "polyline_out.dxf"
    code = cli_module._run_convert(
        str(SAMPLES / "polyline2d_line_2007.dwg"),
        str(output),
        types="LWPOLYLINE",
        dxf_version="R2010",
        strict=False,
    )
    captured = capsys.readouterr()

    assert code == 0
    assert "written_entities: 1" in captured.out
    assert len(dxf_entities_of_type(output, "LWPOLYLINE")) == 1


def test_to_dxf_writes_r14_lwpolyline_vertices(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "polyline_r14_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "polyline2d_line_R14.dwg"),
        str(output),
        types="LWPOLYLINE",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    entities = dxf_entities_of_type(output, "LWPOLYLINE")
    assert len(entities) == 1

    points = dxf_lwpolyline_points(entities[0])
    assert len(points) == 3
    assert abs(points[0][0] - 50.0) < 1.0e-6
    assert abs(points[0][1] - 50.0) < 1.0e-6
    assert abs(points[1][0] - 100.0) < 1.0e-6
    assert abs(points[1][1] - 100.0) < 1.0e-6
    assert abs(points[2][0] - 150.0) < 1.0e-6
    assert abs(points[2][1] - 50.0) < 1.0e-6


def test_to_dxf_writes_r14_ellipse(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "ellipse_r14_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "ellipse_R14.dwg"),
        str(output),
        types="ELLIPSE",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    entities = dxf_entities_of_type(output, "ELLIPSE")
    assert len(entities) == 1
    ellipse = entities[0]
    assert abs(group_float(ellipse, "10") - 100.0) < 1.0e-6
    assert abs(group_float(ellipse, "20") - 100.0) < 1.0e-6
    assert abs(group_float(ellipse, "30")) < 1.0e-6
    assert abs(group_float(ellipse, "11") + 50.0) < 1.0e-6
    assert abs(group_float(ellipse, "21") + 50.0) < 1.0e-6
    assert abs(group_float(ellipse, "31")) < 1.0e-6
    assert abs(group_float(ellipse, "40") - 0.4242640687119286) < 1.0e-9
    assert abs(group_float(ellipse, "41")) < 1.0e-6
    assert abs(group_float(ellipse, "42") - (2.0 * 3.141592653589793)) < 1.0e-6


def test_to_dxf_writes_insert_as_point_fallback(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "insert_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "insert_2004.dwg"),
        str(output),
        types="INSERT",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    inserts = dxf_entities_of_type(output, "INSERT")
    assert len(inserts) == 1
    assert len(dxf_entities_of_type(output, "POINT")) == 0
    assert abs(group_float(inserts[0], "10") - 100.0) < 1.0e-6
    assert abs(group_float(inserts[0], "20") - 50.0) < 1.0e-6
    assert abs(group_float(inserts[0], "30")) < 1.0e-6
    assert abs(group_float(inserts[0], "41") - 2.0) < 1.0e-6
    assert abs(group_float(inserts[0], "42") - 1.5) < 1.0e-6
    assert abs(group_float(inserts[0], "50") - 15.0) < 1.0e-6


def test_read_insert_exposes_block_name() -> None:
    doc = ezdwg.read(str(SAMPLES / "insert_2004.dwg"))
    entities = list(doc.modelspace().query("INSERT"))
    assert len(entities) == 1
    assert entities[0].dxf.get("name") == "BLK1"


def test_to_dxf_strict_raises_on_skipped_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(convert_module, "_write_entity_to_modelspace", lambda *_args, **_kwargs: False)

    with pytest.raises(ValueError, match="failed to convert"):
        convert_module.to_dxf(
            str(SAMPLES / "line_2007.dwg"),
            str(tmp_path / "strict_out.dxf"),
            types="LINE",
            strict=True,
        )


def test_to_dxf_dimension_writes_native_dimension_without_line_fallback(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    source = ROOT / "examples" / "data" / "mechanical_example-imperial.dwg"
    output = tmp_path / "mechanical_dim_out.dxf"
    result = ezdwg.to_dxf(str(source), str(output), types="DIMENSION", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities > 0
    assert result.written_entities == result.total_entities
    assert len(dxf_entities_of_type(output, "DIMENSION")) > 0
    assert len(dxf_entities_of_type(output, "LINE")) == 0


def test_to_dxf_writes_polyline_2d_as_lwpolyline(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D02,
                0x0001,
                [
                    (0.0, 0.0, 0.0, 0.1, 0.2, 0.0, 0.0, 0),
                    (2.0, 0.0, 0.0, 0.2, 0.3, 0.5, 0.0, 0),
                    (2.0, 1.0, 0.0, 0.3, 0.4, 0.0, 0.0, 0),
                    (0.0, 0.0, 0.0, 0.1, 0.2, 0.0, 0.0, 0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    entities = dxf_entities_of_type(output, "LWPOLYLINE")
    assert len(entities) == 1
    points = dxf_lwpolyline_points(entities[0])
    assert points == [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 1.0, 0.0)]


def test_to_dxf_writes_polyline_2d_curve_fit_as_spline(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D03,
                0x0003,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D03,
                0x0003,
                5,
                "QuadraticBSpline",
                True,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D03,
                0x0003,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 1.25, 0.0),
                    (2.0, 1.5, 0.0),
                    (3.0, 1.0, 0.0),
                    (4.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_curve_fit_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_curve_fit.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert len(dxf_entities_of_type(output, "SPLINE")) == 1
    assert len(dxf_entities_of_type(output, "LWPOLYLINE")) == 0


def test_to_dxf_polyline_2d_spline_prefers_control_vertices(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D04,
                0x0000,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (1.0, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 1),
                    (2.0, 1.1, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (3.0, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 8),
                    (4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D04,
                0x0000,
                6,
                "CubicBSpline",
                False,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D04,
                0x0000,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (0.8, 0.6, 0.0),
                    (1.6, 1.0, 0.0),
                    (2.4, 0.9, 0.0),
                    (3.2, 0.5, 0.0),
                    (4.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_curve_ctrl_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_curve_ctrl.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1

    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    xs = [float(value) for code, value in groups if code == "11"]
    ys = [float(value) for code, value in groups if code == "21"]
    assert xs == [0.0, 2.0, 4.0]
    assert ys == [0.0, 1.1, 0.0]


def test_to_dxf_polyline_2d_spline_uses_tangent_dirs(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D05,
                0x0002,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0x12),
                    (2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.3, 0x10),
                    (4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.57079632679, 0x12),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D05,
                0x0002,
                6,
                "CubicBSpline",
                False,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D05,
                0x0002,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 0.9, 0.0),
                    (2.0, 1.2, 0.0),
                    (3.0, 0.8, 0.0),
                    (4.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_curve_tangent_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_curve_tangent.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1

    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    # Tangent-aware path uses CAD control frame export:
    # control points at code 10/20, no fit points at code 11/21.
    assert any(code == "10" for code, _ in groups)
    assert not any(code == "11" for code, _ in groups)


def test_polyline_2d_tangent_angle_unit_detects_degree_values() -> None:
    unit = convert_module._polyline_2d_tangent_angle_unit([0.0, 90.0, 180.0])
    assert unit == "deg"


def test_polyline_2d_tangent_angle_unit_defaults_to_radian() -> None:
    unit = convert_module._polyline_2d_tangent_angle_unit([0.0, math.pi / 2.0, -1.2])
    assert unit == "rad"


def test_polyline_2d_open_uniform_knot_vector() -> None:
    knots = convert_module._open_uniform_knot_vector(4, 2)
    assert knots == [0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]


def test_to_dxf_polyline_2d_curve_type_writes_control_spline(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D06,
                0x0000,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D06,
                0x0000,
                6,
                "CubicBSpline",
                False,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D06,
                0x0000,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 0.8, 0.0),
                    (2.0, 1.1, 0.0),
                    (3.0, 0.7, 0.0),
                    (4.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_curve_type_ctrl_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_curve_type_ctrl.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1
    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    assert any(code == "10" for code, _ in groups)
    assert not any(code == "11" for code, _ in groups)
    assert any(code == "40" for code, _ in groups)


def test_to_dxf_polyline_2d_closed_curve_type_sets_closed_spline_flag(
    monkeypatch, tmp_path: Path
) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D07,
                0x0001,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 16),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D07,
                0x0001,
                6,
                "CubicBSpline",
                True,
                False,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [],
    )

    output = tmp_path / "polyline2d_closed_curve_type_ctrl_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_closed_curve_type.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1
    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    flags = int(next(value for code, value in groups if code == "70"))
    assert (flags & 1) == 1
    assert any(code == "10" for code, _ in groups)
    assert not any(code == "11" for code, _ in groups)


def test_to_dxf_polyline_2d_closed_curve_fit_sets_closed_spline_flag(
    monkeypatch, tmp_path: Path
) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D08,
                0x0003,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D08,
                0x0003,
                5,
                "QuadraticBSpline",
                True,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D08,
                0x0003,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, -0.2, 0.0),
                    (2.0, 0.0, 0.0),
                    (2.2, 1.0, 0.0),
                    (2.0, 2.0, 0.0),
                    (1.0, 2.2, 0.0),
                    (0.0, 2.0, 0.0),
                    (-0.2, 1.0, 0.0),
                    (0.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_closed_curve_fit_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_closed_curve_fit.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1
    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    flags = int(next(value for code, value in groups if code == "70"))
    assert (flags & 1) == 1
    assert (flags & 2) == 2
    assert any(code == "10" for code, _ in groups)
    assert not any(code == "11" for code, _ in groups)


def test_to_dxf_polyline_2d_closed_curve_fit_with_tangents_keeps_periodic_frame(
    monkeypatch, tmp_path: Path
) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D09,
                0x0003,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0x02),
                    (2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0x00),
                    (2.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0x00),
                    (0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.57079632679, 0x02),
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0x00),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D09,
                0x0003,
                5,
                "QuadraticBSpline",
                True,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D09,
                0x0003,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, -0.1, 0.0),
                    (2.0, 0.0, 0.0),
                    (2.1, 1.0, 0.0),
                    (2.0, 2.0, 0.0),
                    (1.0, 2.1, 0.0),
                    (0.0, 2.0, 0.0),
                    (-0.1, 1.0, 0.0),
                    (0.0, 0.0, 0.0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_closed_curve_fit_tangent_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_closed_curve_fit_tangent.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    splines = dxf_entities_of_type(output, "SPLINE")
    assert len(splines) == 1
    groups = splines[0]["groups"]
    assert isinstance(groups, list)
    flags = int(next(value for code, value in groups if code == "70"))
    assert (flags & 1) == 1
    assert (flags & 2) == 2
    # Closed fit path should stay on control-frame representation.
    assert any(code == "10" for code, _ in groups)
    assert not any(code == "11" for code, _ in groups)
