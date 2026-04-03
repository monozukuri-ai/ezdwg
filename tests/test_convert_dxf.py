from __future__ import annotations

from collections import Counter
from pathlib import Path
import math
from typing import Any

import pytest

import ezdwg
import ezdwg.cli as cli_module
import ezdwg.convert as convert_module
import ezdwg.document as document_module
from ezdwg.entity import Entity
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


def test_to_dxf_skips_insert_related_scans_when_no_inserts(
    monkeypatch,
    tmp_path: Path,
) -> None:
    pytest.importorskip("ezdxf")

    def _unexpected_attrs_call(_layout):
        raise AssertionError("_insert_attributes_by_owner should not be called")

    def _unexpected_block_populate(_doc, _layout, **_kwargs):
        raise AssertionError("_populate_block_definitions should not be called")

    monkeypatch.setattr(convert_module, "_insert_attributes_by_owner", _unexpected_attrs_call)
    monkeypatch.setattr(convert_module, "_populate_block_definitions", _unexpected_block_populate)

    output = tmp_path / "line_out_fastpath.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "line_2007.dwg"),
        str(output),
        types="LINE",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert len(dxf_entities_of_type(output, "LINE")) == 1


def test_to_dxf_populates_blocks_for_dimension_anonymous_references(
    monkeypatch,
    tmp_path: Path,
) -> None:
    pytest.importorskip("ezdxf")

    captured: dict[str, int] = {"count": 0}

    def _capture_populate(_doc, _layout, **kwargs):
        refs = kwargs.get("reference_entities") or []
        captured["count"] = sum(1 for entity in refs if entity.dxftype == "DIMENSION")

    monkeypatch.setattr(convert_module, "_populate_block_definitions", _capture_populate)

    class _DummyLayout:
        doc = type("_Doc", (), {"path": "dummy.dwg", "decode_path": "dummy.dwg"})()

    entity = Entity(
        dxftype="DIMENSION",
        handle=1,
        dxf={
            "dimtype": "DIM_LINEAR",
            "anonymous_block_name": "*D1",
            "defpoint": (0.0, 0.0, 0.0),
            "defpoint2": (10.0, 0.0, 0.0),
            "defpoint3": (10.0, 10.0, 0.0),
        },
    )
    monkeypatch.setattr(
        convert_module,
        "_resolve_layout",
        lambda _source: ("dummy.dwg", _DummyLayout()),
    )
    monkeypatch.setattr(
        convert_module,
        "_resolve_export_entities",
        lambda *_args, **_kwargs: [entity],
    )

    output = tmp_path / "dim_ref_block_scan.dxf"
    result = convert_module.to_dxf("dummy.dwg", str(output), dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert captured["count"] == 1


def test_to_dxf_without_color_resolution_skips_style_decoders(
    monkeypatch,
    tmp_path: Path,
) -> None:
    pytest.importorskip("ezdxf")

    def _unexpected_style_decode(_path):
        raise AssertionError("decode_entity_styles should not be called")

    def _unexpected_layer_decode(_path):
        raise AssertionError("decode_layer_colors should not be called")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", _unexpected_style_decode)
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", _unexpected_layer_decode)
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    output = tmp_path / "line_no_color_resolve.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "line_2007.dwg"),
        str(output),
        types="LINE",
        dxf_version="R2010",
        preserve_colors=False,
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
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


def test_to_dxf_preserves_mtext_anchor_and_orientation(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    dummy_doc = type(
        "_Doc",
        (),
        {"path": "dummy_mtext_convert.dwg", "decode_path": "dummy_mtext_convert.dwg"},
    )()

    class _DummyLayout:
        doc = dummy_doc

    monkeypatch.setattr(
        convert_module,
        "_resolve_layout",
        lambda _source: ("dummy_mtext_convert.dwg", _DummyLayout()),
    )
    monkeypatch.setattr(
        convert_module,
        "_resolve_export_entities",
        lambda *_args, **_kwargs: [
            Entity(
                dxftype="MTEXT",
                handle=1,
                dxf={
                    "insert": (10.0, 20.0, 0.0),
                    "text": "ANCHOR",
                    "char_height": 2.5,
                    "rect_width": 42.0,
                    "attachment_point": 6,
                    "drawing_direction": 3,
                    "text_direction": (0.0, 1.0, 0.0),
                    "extrusion": (0.0, 0.0, 1.0),
                },
            ),
            Entity(
                dxftype="MTEXT",
                handle=2,
                dxf={
                    "insert": (1.0, 2.0, 0.0),
                    "text": "ROTATE",
                    "char_height": 1.25,
                    "attachment_point": 4,
                    "rotation": 30.0,
                },
            ),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    output = tmp_path / "mtext_anchor_out.dxf"
    result = convert_module.to_dxf(
        "dummy_mtext_convert.dwg",
        str(output),
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 2
    assert result.written_entities == 2

    dxf_doc = ezdxf.readfile(str(output))
    mtexts = {entity.text: entity for entity in dxf_doc.modelspace().query("MTEXT")}

    anchor = mtexts["ANCHOR"]
    assert tuple(anchor.dxf.insert) == pytest.approx((10.0, 20.0, 0.0))
    assert float(anchor.dxf.char_height) == pytest.approx(2.5)
    assert int(anchor.dxf.attachment_point) == 6
    assert int(anchor.dxf.flow_direction) == 3
    assert tuple(anchor.dxf.text_direction) == pytest.approx((0.0, 1.0, 0.0))
    assert tuple(anchor.dxf.extrusion) == pytest.approx((0.0, 0.0, 1.0))
    assert float(anchor.dxf.width) == pytest.approx(42.0)

    rotate = mtexts["ROTATE"]
    assert tuple(rotate.dxf.insert) == pytest.approx((1.0, 2.0, 0.0))
    assert int(rotate.dxf.attachment_point) == 4
    assert float(rotate.dxf.rotation) == pytest.approx(30.0)


def test_to_dxf_writes_ray_and_xline_entities(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (101, 10, 0, 0x28, "RAY", "Entity"),
            (102, 11, 0, 0x29, "XLINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_ray_entities",
        lambda _path: [(101, (1.0, 2.0, 0.0), (1.0, 0.0, 0.0))],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_xline_entities",
        lambda _path: [(102, (3.0, 4.0, 0.0), (0.0, 1.0, 0.0))],
    )

    output = tmp_path / "ray_xline_out.dxf"
    doc = document_module.Document(path="dummy_ray_xline.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), types="RAY XLINE", dxf_version="R2010")

    assert result.total_entities == 2
    assert result.written_entities == 2
    assert len(dxf_entities_of_type(output, "RAY")) == 1
    assert len(dxf_entities_of_type(output, "XLINE")) == 1

    ray = dxf_entities_of_type(output, "RAY")[0]
    assert abs(group_float(ray, "10") - 1.0) < 1.0e-6
    assert abs(group_float(ray, "20") - 2.0) < 1.0e-6
    assert abs(group_float(ray, "11") - 1.0) < 1.0e-6
    assert abs(group_float(ray, "21") - 0.0) < 1.0e-6

    xline = dxf_entities_of_type(output, "XLINE")[0]
    assert abs(group_float(xline, "10") - 3.0) < 1.0e-6
    assert abs(group_float(xline, "20") - 4.0) < 1.0e-6
    assert abs(group_float(xline, "11") - 0.0) < 1.0e-6
    assert abs(group_float(xline, "21") - 1.0) < 1.0e-6


def test_to_dxf_writes_leader_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(0x920, 10, 0, 0x1B, "LEADER", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_leader_entities",
        lambda _path: [
            (0x920, 1, 0, [(0.0, 0.0, 0.0), (10.0, 2.0, 0.0), (12.0, 3.0, 0.0)])
        ],
    )

    output = tmp_path / "leader_out.dxf"
    doc = document_module.Document(path="dummy_leader_convert.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), types="LEADER", dxf_version="R2010")

    assert result.total_entities == 1
    assert result.written_entities == 1
    leaders = dxf_entities_of_type(output, "LEADER")
    assert len(leaders) == 1
    assert len(dxf_entities_of_type(output, "POLYLINE")) == 0
    assert abs(group_float(leaders[0], "10") - 0.0) < 1.0e-6
    assert abs(group_float(leaders[0], "20") - 0.0) < 1.0e-6


def test_to_dxf_skips_region_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(201, 10, 0, 0x25, "REGION", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_region_entities", lambda _path: [(201,)])

    output = tmp_path / "region_out.dxf"
    doc = document_module.Document(path="dummy_region.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), types="REGION", dxf_version="R2010")

    assert result.total_entities == 1
    assert result.written_entities == 0
    assert result.skipped_entities == 1
    assert result.skipped_by_type == {"REGION": 1}
    assert len(dxf_entities_of_type(output, "REGION")) == 0


def test_to_dxf_skips_3dsolid_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(301, 10, 0, 0x26, "3DSOLID", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_3dsolid_entities", lambda _path: [(301,)])

    output = tmp_path / "3dsolid_out.dxf"
    doc = document_module.Document(path="dummy_3dsolid.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), types="3DSOLID", dxf_version="R2010")

    assert result.total_entities == 1
    assert result.written_entities == 0
    assert result.skipped_entities == 1
    assert result.skipped_by_type == {"3DSOLID": 1}
    assert len(dxf_entities_of_type(output, "3DSOLID")) == 0


def test_to_dxf_skips_body_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(302, 10, 0, 0x27, "BODY", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_body_entities", lambda _path: [(302,)])

    output = tmp_path / "body_out.dxf"
    doc = document_module.Document(path="dummy_body.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), types="BODY", dxf_version="R2010")

    assert result.total_entities == 1
    assert result.written_entities == 0
    assert result.skipped_entities == 1
    assert result.skipped_by_type == {"BODY": 1}
    assert len(dxf_entities_of_type(output, "BODY")) == 0


def test_to_dxf_skips_oleframe_and_ole2frame_entities(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (401, 10, 0, 0x2B, "OLEFRAME", "Entity"),
            (402, 11, 0, 0x4A, "OLE2FRAME", "Entity"),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_oleframe_entities", lambda _path: [(401,)])
    monkeypatch.setattr(document_module.raw, "decode_ole2frame_entities", lambda _path: [(402,)])

    output = tmp_path / "oleframes_out.dxf"
    doc = document_module.Document(path="dummy_oleframes.dwg", version="AC1021")
    result = ezdwg.to_dxf(
        doc,
        str(output),
        types="OLEFRAME OLE2FRAME",
        dxf_version="R2010",
    )

    assert result.total_entities == 2
    assert result.written_entities == 0
    assert result.skipped_entities == 2
    assert result.skipped_by_type == {"OLE2FRAME": 1, "OLEFRAME": 1}
    assert len(dxf_entities_of_type(output, "OLEFRAME")) == 0
    assert len(dxf_entities_of_type(output, "OLE2FRAME")) == 0


def test_to_dxf_skips_long_transaction_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(403, 10, 0, 0x4C, "LONG_TRANSACTION", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_long_transaction_entities",
        lambda _path: [(403,)],
    )

    output = tmp_path / "long_transaction_out.dxf"
    doc = document_module.Document(path="dummy_long_transaction.dwg", version="AC1021")
    result = ezdwg.to_dxf(
        doc,
        str(output),
        types="LONG_TRANSACTION",
        dxf_version="R2010",
    )

    assert result.total_entities == 1
    assert result.written_entities == 0
    assert result.skipped_entities == 1
    assert result.skipped_by_type == {"LONG_TRANSACTION": 1}
    assert len(dxf_entities_of_type(output, "LONG_TRANSACTION")) == 0


def test_to_dxf_default_query_skips_unsupported_types(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (101, 10, 0, 0x13, "LINE", "Entity"),
            (201, 11, 0, 0x25, "REGION", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(101, 1.0, 2.0, 0.0, 3.0, 4.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_arc_circle_entities",
        lambda _path: ([(101, 1.0, 2.0, 0.0, 3.0, 4.0, 0.0)], [], []),
    )
    region_decode_called = {"called": False}

    def _decode_region_entities(_path):
        region_decode_called["called"] = True
        return [(201,)]

    monkeypatch.setattr(document_module.raw, "decode_region_entities", _decode_region_entities)

    output = tmp_path / "default_skip_unsupported_out.dxf"
    doc = document_module.Document(path="dummy_default_skip_unsupported.dwg", version="AC1021")
    result = ezdwg.to_dxf(doc, str(output), dxf_version="R2010", strict=True)

    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert result.skipped_by_type == {}
    assert region_decode_called["called"] is False
    assert len(dxf_entities_of_type(output, "LINE")) == 1
    assert len(dxf_entities_of_type(output, "REGION")) == 0


def test_to_dxf_include_unsupported_keeps_skip_reporting(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (101, 10, 0, 0x13, "LINE", "Entity"),
            (201, 11, 0, 0x25, "REGION", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(101, 1.0, 2.0, 0.0, 3.0, 4.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_arc_circle_entities",
        lambda _path: ([(101, 1.0, 2.0, 0.0, 3.0, 4.0, 0.0)], [], []),
    )
    monkeypatch.setattr(document_module.raw, "decode_region_entities", lambda _path: [(201,)])

    output = tmp_path / "default_include_unsupported_out.dxf"
    doc = document_module.Document(path="dummy_default_include_unsupported.dwg", version="AC1021")
    result = ezdwg.to_dxf(
        doc,
        str(output),
        dxf_version="R2010",
        include_unsupported=True,
    )

    assert result.total_entities == 2
    assert result.written_entities == 1
    assert result.skipped_entities == 1
    assert result.skipped_by_type == {"REGION": 1}
    assert len(dxf_entities_of_type(output, "LINE")) == 1
    assert len(dxf_entities_of_type(output, "REGION")) == 0


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


def test_to_dxf_lwpolyline_flag_0x200_is_treated_as_closed(
    monkeypatch,
    tmp_path: Path,
) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    monkeypatch.setattr(
        document_module.raw,
        "decode_lwpolyline_entities",
        lambda _path: [
            (
                0x2200,
                0x0200,
                [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)],
                [],
                [],
                None,
            )
        ],
    )

    doc = document_module.Document(path="dummy_lwpolyline_0x200.dwg", version="AC1021")
    output = tmp_path / "lwpolyline_0x200_closed_out.dxf"
    result = ezdwg.to_dxf(doc, str(output), types="LWPOLYLINE", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    entities = dxf_entities_of_type(output, "LWPOLYLINE")
    assert len(entities) == 1
    assert group_float(entities[0], "70") == 1.0


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


def test_to_dxf_exports_block_definition_for_insert(tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    output = tmp_path / "insert_block_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "insert_2004.dwg"),
        str(output),
        types="INSERT",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    block = dxf_doc.blocks.get("BLK1")
    # BLK1 contains at least one drawable primitive in the source DWG.
    assert len(list(block)) > 0

    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "BLK1"


def test_to_dxf_flatten_inserts_writes_primitives_to_modelspace(tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    output = tmp_path / "insert_block_flattened_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "insert_2004.dwg"),
        str(output),
        types="INSERT",
        dxf_version="R2010",
        flatten_inserts=True,
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    assert len(list(dxf_doc.modelspace().query("INSERT"))) == 0
    assert len(list(dxf_doc.modelspace().query("LINE"))) >= 1


def test_cli_convert_flatten_inserts_flag(tmp_path: Path, capsys) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "insert_flatten_cli_out.dxf"
    code = cli_module._run_convert(
        str(SAMPLES / "insert_2004.dwg"),
        str(output),
        types="INSERT",
        dxf_version="R2010",
        strict=False,
        flatten_inserts=True,
    )
    captured = capsys.readouterr()

    assert code == 0
    assert "written_entities: 1" in captured.out
    assert len(dxf_entities_of_type(output, "INSERT")) == 0
    assert len(dxf_entities_of_type(output, "LINE")) >= 1


def test_flatten_modelspace_inserts_normalizes_suspicious_scaled_insert() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    big = doc.blocks.new(name="BIG_ABS")
    big.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    small = doc.blocks.new(name="SMALL_LOCAL")
    small.add_line((0.0, 0.0), (10.0, 0.0))
    modelspace = doc.modelspace()
    modelspace.add_blockref("BIG_ABS", (100.0, 200.0), dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0})
    modelspace.add_blockref("SMALL_LOCAL", (5.0, 5.0))

    convert_module._flatten_modelspace_inserts(modelspace)

    # Suspicious transformed insert is normalized then exploded.
    assert len(list(modelspace.query("INSERT"))) == 0
    lines = list(modelspace.query("LINE"))
    assert len(lines) == 2
    points = sorted(
        [
            (tuple(line.dxf.start), tuple(line.dxf.end))
            for line in lines
        ],
        key=lambda entry: entry[0][0],
    )
    assert points[0] == ((5.0, 5.0, 0.0), (15.0, 5.0, 0.0))
    assert points[1] == ((20100.0, 30200.0, 0.0), (20200.0, 30200.0, 0.0))


def test_flatten_modelspace_inserts_prunes_generated_outliers_with_reference_bbox() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    big = doc.blocks.new(name="BIG_ABS")
    big.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    small = doc.blocks.new(name="SMALL_LOCAL")
    small.add_line((0.0, 0.0), (10.0, 0.0))
    modelspace = doc.modelspace()
    # Pre-existing primitive defines reference bbox near origin.
    modelspace.add_line((0.0, 0.0), (100.0, 0.0))
    modelspace.add_blockref(
        "BIG_ABS",
        (100.0, 200.0),
        dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0},
    )
    modelspace.add_blockref("SMALL_LOCAL", (5.0, 5.0))

    convert_module._flatten_modelspace_inserts(modelspace)

    assert len(list(modelspace.query("INSERT"))) == 0
    lines = list(modelspace.query("LINE"))
    # Base line + exploded safe insert line. The far-away normalized insert line
    # should be pruned as an outlier.
    assert len(lines) == 2
    points = sorted(
        [
            (tuple(line.dxf.start), tuple(line.dxf.end))
            for line in lines
        ],
        key=lambda entry: entry[0][0],
    )
    assert points[0] == ((0.0, 0.0, 0.0), (100.0, 0.0, 0.0))
    assert points[1] == ((5.0, 5.0, 0.0), (15.0, 5.0, 0.0))


def test_flatten_modelspace_inserts_keeps_layout_pseudo_references() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_line((0.0, 0.0), (1.0, 0.0))
    modelspace.add_blockref(
        "*Model_Space",
        (100.0, 200.0),
        dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0, "rotation": 90.0},
    )

    convert_module._flatten_modelspace_inserts(modelspace)

    inserts = list(modelspace.query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "*Model_Space"


def test_prepare_insert_for_flatten_normalizes_modelspace_alias_rotation_90() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="__EZDWG_LAYOUT_ALIAS_MODEL_SPACE")
    block.add_line((20000.0, 30000.0), (20010.0, 30020.0))
    modelspace = doc.modelspace()
    insert = modelspace.add_blockref(
        "__EZDWG_LAYOUT_ALIAS_MODEL_SPACE",
        (100.0, 200.0),
        dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0, "rotation": 90.0},
    )

    drop_insert = convert_module._prepare_insert_for_flatten(modelspace, insert)

    assert drop_insert is False
    assert float(insert.dxf.xscale) == 1.0
    assert float(insert.dxf.yscale) == 1.0
    assert float(insert.dxf.zscale) == 1.0
    assert float(insert.dxf.rotation) == 0.0
    assert abs(float(insert.dxf.insert.x) - 80.0) < 1.0e-6
    assert abs(float(insert.dxf.insert.y) - 0.0) < 1.0e-6


def test_prepare_insert_for_flatten_normalizes_modelspace_alias_rotation_270() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="__EZDWG_LAYOUT_ALIAS_MODEL_SPACE")
    block.add_line((20000.0, 30000.0), (20010.0, 30020.0))
    modelspace = doc.modelspace()
    insert = modelspace.add_blockref(
        "__EZDWG_LAYOUT_ALIAS_MODEL_SPACE",
        (100.0, 200.0),
        dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0, "rotation": 270.0},
    )

    drop_insert = convert_module._prepare_insert_for_flatten(modelspace, insert)

    assert drop_insert is False
    assert float(insert.dxf.xscale) == 1.0
    assert float(insert.dxf.yscale) == 1.0
    assert float(insert.dxf.zscale) == 1.0
    assert float(insert.dxf.rotation) == 180.0
    assert abs(float(insert.dxf.insert.x) - 100.0) < 1.0e-6
    assert abs(float(insert.dxf.insert.y) - 180.0) < 1.0e-6


def test_prepare_insert_for_flatten_drops_suspicious_scaled_anonymous_dimension_insert() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D_BIG")
    block.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    modelspace = doc.modelspace()
    insert = modelspace.add_blockref(
        "*D_BIG",
        (100.0, 200.0),
        dxfattribs={"xscale": 60.0, "yscale": 60.0, "zscale": 60.0, "rotation": 0.0},
    )

    drop_insert = convert_module._prepare_insert_for_flatten(modelspace, insert)

    assert drop_insert is True


def test_restore_known_layout_frame_polylines_adds_missing_left_frames() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME"},
    )
    modelspace.add_lwpolyline(
        [
            (28450.163173, 1500.0),
            (51580.163173, 1500.0),
            (51580.163173, 17220.0),
            (28450.163173, 17220.0),
        ],
        close=True,
        dxfattribs={"layer": "FRAME"},
    )

    convert_module._restore_known_layout_frame_polylines(modelspace)

    rects = [
        convert_module._axis_aligned_lwpolyline_rect_bbox(entity)
        for entity in modelspace.query("LWPOLYLINE")
    ]

    def _has_rect(
        min_x: float,
        max_x: float,
        min_y: float,
        max_y: float,
        *,
        tol: float = 1.0e-6,
    ) -> bool:
        for rect in rects:
            if rect is None:
                continue
            rect_min_x, rect_max_x, rect_min_y, rect_max_y = rect
            if (
                abs(rect_min_x - min_x) <= tol
                and abs(rect_max_x - max_x) <= tol
                and abs(rect_min_y - min_y) <= tol
                and abs(rect_max_y - max_y) <= tol
            ):
                return True
        return False

    def _has_line(
        x1: float,
        y1: float,
        x2: float,
        y2: float,
        *,
        tol: float = 1.0e-6,
    ) -> bool:
        for entity in modelspace.query("LINE"):
            start = entity.dxf.start
            end = entity.dxf.end
            same = (
                abs(float(start.x) - x1) <= tol
                and abs(float(start.y) - y1) <= tol
                and abs(float(end.x) - x2) <= tol
                and abs(float(end.y) - y2) <= tol
            )
            reverse = (
                abs(float(start.x) - x2) <= tol
                and abs(float(start.y) - y2) <= tol
                and abs(float(end.x) - x1) <= tol
                and abs(float(end.y) - y1) <= tol
            )
            if same or reverse:
                return True
        return False

    assert _has_rect(1050.0, 24180.0, 600.0, 17220.0)
    assert _has_rect(0.0, 25230.0, 0.0, 17820.0)
    assert not _has_rect(28450.163173, 51580.163173, 600.0, 17220.0)
    assert _has_line(14850.0, 1500.0, 24180.0, 1500.0)
    assert _has_line(19830.0, 975.0, 22380.0, 975.0)
    assert _has_line(22380.0, 600.0, 22380.0, 1500.0)


def test_restore_known_layout_frame_polylines_noop_without_signature() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_lwpolyline(
        [(3000.0, 2000.0), (18000.0, 2000.0), (18000.0, 12000.0), (3000.0, 12000.0)],
        close=True,
        dxfattribs={"layer": "FRAME"},
    )
    before = len(list(modelspace.query("LWPOLYLINE")))

    convert_module._restore_known_layout_frame_polylines(modelspace)

    after = len(list(modelspace.query("LWPOLYLINE")))
    assert after == before


def test_restore_known_layout_frame_polylines_removes_misaligned_title_ghost_lines() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME"},
    )
    # Known malformed fragment observed in this Open30-derived series.
    ghost_lines = [
        modelspace.add_line((18894.215, 1540.66), (18894.215, 590.66)),
        modelspace.add_line((16124.216, 1540.66), (16124.216, 590.66)),
        modelspace.add_line((18894.215, 590.66), (16124.216, 590.66)),
        modelspace.add_line((18524.215, 1540.66), (18524.215, 890.66)),
        modelspace.add_line((15824.216, 1540.66), (15824.216, 890.66)),
        modelspace.add_line((18524.215, 890.66), (15824.216, 890.66)),
    ]

    convert_module._restore_known_layout_frame_polylines(modelspace)

    remaining_ids = {id(entity) for entity in modelspace}
    for ghost in ghost_lines:
        assert id(ghost) not in remaining_ids


def test_restore_known_layout_frame_polylines_removes_top_left_ghost_lines() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME"},
    )
    ghost_lines = [
        modelspace.add_line((-3590.659, 18524.215), (-890.66, 18524.215)),
        modelspace.add_line((-890.66, 17874.215), (-890.66, 18524.215)),
        modelspace.add_line((-590.66, 17944.215), (-590.66, 18894.215)),
    ]

    convert_module._restore_known_layout_frame_polylines(modelspace)

    remaining_ids = {id(entity) for entity in modelspace}
    for ghost in ghost_lines:
        assert id(ghost) not in remaining_ids


def test_prune_generated_entities_outside_known_sheet_windows_removes_outside_noise() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    # Signature rectangles for this drawing series.
    modelspace.add_lwpolyline(
        [(0.0, 0.0), (25230.0, 0.0), (25230.0, 17820.0), (0.0, 17820.0)],
        close=True,
    )
    modelspace.add_lwpolyline(
        [(28450.0, 1500.0), (51580.0, 1500.0), (51580.0, 17220.0), (28450.0, 17220.0)],
        close=True,
    )

    original_inside = modelspace.add_line((1000.0, 1000.0), (1200.0, 1000.0))
    original_ids = {id(original_inside)}

    generated_inside = modelspace.add_line((29000.0, 2000.0), (29100.0, 2000.0))
    outside_short = modelspace.add_line((-890.66, 17874.215), (-890.66, 18524.215))
    outside_mid = modelspace.add_line((26643.933, 5793.961), (26643.933, 5423.961))
    outside_long = modelspace.add_line((-3590.659, 18524.215), (-590.66, 18524.215))
    very_long_outside = modelspace.add_line((25000.0, -1000.0), (32050.0, -1000.0))
    outside_text = modelspace.add_text("NOISE", dxfattribs={"insert": (26600.0, 5600.0)})

    convert_module._prune_generated_entities_outside_known_sheet_windows(
        modelspace,
        original_ids,
    )

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(original_inside) in remaining_ids
    assert id(generated_inside) in remaining_ids
    assert id(outside_short) not in remaining_ids
    assert id(outside_mid) not in remaining_ids
    assert id(outside_long) not in remaining_ids
    assert id(outside_text) not in remaining_ids
    assert id(very_long_outside) in remaining_ids


def test_prune_generated_entities_outside_known_sheet_windows_accepts_canonical_1050_gap() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    modelspace.add_lwpolyline(
        [(0.0, 0.0), (25230.0, 0.0), (25230.0, 17820.0), (0.0, 17820.0)],
        close=True,
    )
    # Canonical right sheet window with a narrow inter-sheet gap.
    modelspace.add_lwpolyline(
        [(26280.0, 1500.0), (49410.0, 1500.0), (49410.0, 17220.0), (26280.0, 17220.0)],
        close=True,
    )

    generated_inside = modelspace.add_line((27000.0, 2500.0), (27100.0, 2500.0))
    outside_between_windows = modelspace.add_line((25700.0, 2500.0), (25800.0, 2500.0))
    outside_top_left = modelspace.add_line((-900.0, 18500.0), (-900.0, 17880.0))

    convert_module._prune_generated_entities_outside_known_sheet_windows(
        modelspace,
        original_entity_ids=set(),
    )

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(generated_inside) in remaining_ids
    assert id(outside_between_windows) not in remaining_ids
    assert id(outside_top_left) not in remaining_ids


def test_realign_generated_right_sheet_window_moves_generated_right_entities() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    modelspace.add_lwpolyline(
        [(0.0, 0.0), (25230.0, 0.0), (25230.0, 17820.0), (0.0, 17820.0)],
        close=True,
    )
    right_frame = modelspace.add_lwpolyline(
        [(28450.163, 1500.0), (51580.163, 1500.0), (51580.163, 17220.0), (28450.163, 17220.0)],
        close=True,
    )

    generated_line = modelspace.add_line((30000.163, 5000.0), (30100.163, 5000.0))
    original_line = modelspace.add_line((30500.0, 6000.0), (30600.0, 6000.0))
    original_ids = {id(original_line)}

    convert_module._realign_generated_right_sheet_window(modelspace, original_ids)

    start = generated_line.dxf.start
    end = generated_line.dxf.end
    assert abs(float(start.x) - 27830.0) <= 1.0e-3
    assert abs(float(end.x) - 27930.0) <= 1.0e-3

    # Original entities stay untouched.
    original_start = original_line.dxf.start
    assert abs(float(original_start.x) - 30500.0) <= 1.0e-6

    # Right frame should be shifted to the canonical expected window.
    rect = convert_module._axis_aligned_lwpolyline_rect_bbox(right_frame)
    assert rect is not None
    min_x, max_x, _min_y, _max_y = rect
    assert abs(min_x - 26280.0) <= 1.0e-3
    assert abs(max_x - 49410.0) <= 1.0e-3


def test_dedupe_large_axis_aligned_lwpolyline_rectangles_removes_duplicates() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    first = modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME", "color": 5},
    )
    duplicate = modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME", "color": 5},
    )
    # Different style should be preserved.
    different_color = modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
        dxfattribs={"layer": "FRAME", "color": 3},
    )

    convert_module._dedupe_large_axis_aligned_lwpolyline_rectangles(modelspace)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(first) in remaining_ids
    assert id(duplicate) not in remaining_ids
    assert id(different_color) in remaining_ids


def test_ensure_layout_pseudo_block_alias_skips_acad_detailviewstyle_insert() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    source = doc.blocks.get("*Model_Space")
    if source is None:
        pytest.skip("modelspace block is unavailable")
    try:
        doc.blocks.get("ACAD_DETAILVIEWSTYLE")
    except Exception:
        doc.blocks.new(name="ACAD_DETAILVIEWSTYLE")
    source.add_line((0.0, 0.0), (10.0, 0.0))
    source.add_blockref("ACAD_DETAILVIEWSTYLE", (5.0, 5.0))

    alias_name = convert_module._ensure_layout_pseudo_block_alias(doc, "*Model_Space")

    assert alias_name is not None
    alias = doc.blocks.get(alias_name)
    assert alias is not None
    assert len(list(alias.query("LINE"))) == 1
    assert len(list(alias.query("INSERT"))) == 0


def test_prune_flatten_tiny_generated_clusters_drops_external_small_clusters() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 7000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    kept_generated = modelspace.add_line((300.0, 300.0), (320.0, 300.0))
    dropped_generated = modelspace.add_line((14000.0, 5000.0), (14020.0, 5000.0))
    dropped_generated_2 = modelspace.add_line((14100.0, 5050.0), (14120.0, 5050.0))

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(kept_generated) in remaining_ids
    assert id(dropped_generated) not in remaining_ids
    assert id(dropped_generated_2) not in remaining_ids


def test_prune_flatten_tiny_generated_clusters_keeps_internal_small_clusters() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 7000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    kept_generated = modelspace.add_line((350.0, 350.0), (360.0, 350.0))
    kept_generated_2 = modelspace.add_line((420.0, 330.0), (430.0, 330.0))

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(kept_generated) in remaining_ids
    assert id(kept_generated_2) in remaining_ids


def test_prune_flatten_tiny_generated_clusters_drops_external_annotation_noise() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 7000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    outside_point = modelspace.add_point((14000.0, 5000.0))
    outside_text = modelspace.add_text("X", dxfattribs={"insert": (14100.0, 5050.0)})
    outside_short = modelspace.add_line((14200.0, 5060.0), (14280.0, 5060.0))
    outside_long = modelspace.add_line((14300.0, 5000.0), (15350.0, 5000.0))

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(outside_point) not in remaining_ids
    assert id(outside_text) not in remaining_ids
    assert id(outside_short) not in remaining_ids
    assert id(outside_long) in remaining_ids


def test_prune_flatten_tiny_generated_clusters_keeps_original_external_annotations() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 7000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    original_outside_text = modelspace.add_text("KEEP", dxfattribs={"insert": (14000.0, 5000.0)})
    original_outside_short = modelspace.add_line((14200.0, 5060.0), (14280.0, 5060.0))
    original_entity_ids.add(id(original_outside_text))
    original_entity_ids.add(id(original_outside_short))

    generated_outside_text = modelspace.add_text("DROP", dxfattribs={"insert": (14100.0, 5050.0)})

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(original_outside_text) in remaining_ids
    assert id(original_outside_short) in remaining_ids
    assert id(generated_outside_text) not in remaining_ids


def test_prune_flatten_tiny_generated_clusters_drops_implausible_original_extent() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 7000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    implausible_arc = modelspace.add_arc((1.0, 1.0), 3.0e20, 0.0, 180.0)
    implausible_ellipse = modelspace.add_ellipse(
        center=(-2.0e20, 2000.0),
        major_axis=(10.0, 0.0),
        ratio=0.5,
    )
    original_entity_ids.add(id(implausible_arc))
    original_entity_ids.add(id(implausible_ellipse))
    generated_noise = modelspace.add_text("DROP", dxfattribs={"insert": (14000.0, 5050.0)})

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(implausible_arc) not in remaining_ids
    assert id(implausible_ellipse) not in remaining_ids
    assert id(generated_noise) not in remaining_ids


def test_prune_flatten_tiny_generated_clusters_drops_near_origin_medium_lines() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = 5000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 12000.0 + float(index % 26) * 120.0
        y = float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    near_origin_medium = modelspace.add_line((0.0, 0.0), (-1500.0, 0.0))
    far_medium = modelspace.add_line((18000.0, 5000.0), (19500.0, 5000.0))

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(near_origin_medium) not in remaining_ids
    assert id(far_medium) in remaining_ids


def test_prune_flatten_tiny_generated_clusters_keeps_footer_band_entities() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    original_entity_ids: set[int] = set()
    for index in range(260):
        x = 5000.0 + float(index % 26) * 120.0
        y = 7000.0 + float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))
    for index in range(260):
        x = 12000.0 + float(index % 26) * 120.0
        y = 7000.0 + float(index // 26) * 120.0
        line = modelspace.add_line((x, y), (x + 20.0, y))
        original_entity_ids.add(id(line))

    modelspace.add_lwpolyline(
        [(1050.0, 1500.0), (24180.0, 1500.0), (24180.0, 17220.0), (1050.0, 17220.0)],
        close=True,
    )

    footer_text = modelspace.add_text("TITLE", dxfattribs={"insert": (6000.0, 700.0)})
    footer_short = modelspace.add_line((6500.0, 800.0), (6600.0, 800.0))
    origin_text = modelspace.add_text("NOISE", dxfattribs={"insert": (300.0, -80.0)})
    origin_short = modelspace.add_line((250.0, -50.0), (320.0, -50.0))

    convert_module._prune_flatten_tiny_generated_clusters(modelspace, original_entity_ids)

    remaining_ids = {id(entity) for entity in modelspace}
    assert id(footer_text) in remaining_ids
    assert id(footer_short) in remaining_ids
    assert id(origin_text) not in remaining_ids
    assert id(origin_short) not in remaining_ids


def test_cli_convert_passes_dim_block_policy(monkeypatch, tmp_path: Path) -> None:
    captured: dict[str, Any] = {}

    def _fake_to_dxf(source_path, output_path, **kwargs):
        captured["source_path"] = source_path
        captured["output_path"] = output_path
        captured["kwargs"] = kwargs
        return convert_module.ConvertResult(
            source_path=str(source_path),
            output_path=str(output_path),
            total_entities=1,
            written_entities=1,
            skipped_entities=0,
            skipped_by_type={},
        )

    monkeypatch.setattr(cli_module, "to_dxf", _fake_to_dxf)

    code = cli_module._run_convert(
        str(SAMPLES / "line_2007.dwg"),
        str(tmp_path / "line_cli_dim_policy_out.dxf"),
        types="LINE",
        dxf_version="R2010",
        strict=False,
        dim_block_policy="legacy",
    )

    assert code == 0
    assert captured["kwargs"]["dim_block_policy"] == "legacy"


def test_to_dxf_insert_writes_linked_attribs(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x13, "LINE", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x07, "INSERT", "Entity"),
            (210, 14, 0, 0x02, "ATTRIB", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_ATTR"), (101, "ENDBLK", "BLK_ATTR")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [(200, 5.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_ATTR")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_attrib_entities",
        lambda _path: [
            (
                210,
                "VAL1",
                "TAG1",
                None,
                (5.0, 5.0, 0.0),
                None,
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 2.5, 0.0, 1.0),
                (0, 0, 0),
                0,
                False,
                (None, 200),
            )
        ],
    )

    output = tmp_path / "insert_attrib_out.dxf"
    doc = document_module.Document(path="dummy_insert_attrib.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "BLK_ATTR"
    assert len(inserts[0].attribs) == 1
    assert inserts[0].attribs[0].dxf.tag == "TAG1"
    assert inserts[0].attribs[0].dxf.text == "VAL1"
    assert abs(float(inserts[0].attribs[0].dxf.height) - 2.5) < 1.0e-9
    assert len(dxf_entities_of_type(output, "POINT")) == 0


def test_to_dxf_block_export_writes_attdef_entity(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x03, "ATTDEF", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_ATTDEF"), (101, "ENDBLK", "BLK_ATTDEF")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [(200, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_ATTDEF")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_attdef_entities",
        lambda _path: [
            (
                110,
                "Default",
                "NAME",
                "Enter name",
                (1.0, 2.0, 0.0),
                None,
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 2.0, 0.0, 1.0),
                (0, 0, 0),
                0,
                False,
                (None, 100),
            )
        ],
    )

    output = tmp_path / "insert_attdef_out.dxf"
    doc = document_module.Document(path="dummy_insert_attdef.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    block = dxf_doc.blocks.get("BLK_ATTDEF")
    attdefs = list(block.query("ATTDEF"))
    assert len(attdefs) == 1
    assert attdefs[0].dxf.tag == "NAME"
    assert attdefs[0].dxf.text == "Default"
    assert len(list(block.query("TEXT"))) == 0


def test_to_dxf_writes_minsert_as_insert_array(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x13, "LINE", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x08, "MINSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_ARRAY"), (101, "ENDBLK", "BLK_ARRAY")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_minsert_entities",
        lambda _path: [
            (
                200,
                5.0,
                6.0,
                0.0,
                1.0,
                1.0,
                1.0,
                0.0,
                (4, 5, 2.5, 3.5, "BLK_ARRAY"),
            )
        ],
    )

    output = tmp_path / "minsert_out.dxf"
    doc = document_module.Document(path="dummy_minsert_convert.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="MINSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert len(dxf_entities_of_type(output, "POINT")) == 0

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 1
    insert = inserts[0]
    assert insert.dxf.name == "BLK_ARRAY"
    assert int(insert.dxf.column_count) == 4
    assert int(insert.dxf.row_count) == 5
    assert abs(float(insert.dxf.column_spacing) - 2.5) < 1.0e-9
    assert abs(float(insert.dxf.row_spacing) - 3.5) < 1.0e-9

    block = dxf_doc.blocks.get("BLK_ARRAY")
    assert len(list(block.query("LINE"))) == 1


def test_to_dxf_writes_minsert_with_attribs_as_expanded_inserts(
    monkeypatch, tmp_path: Path
) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x13, "LINE", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x08, "MINSERT", "Entity"),
            (210, 14, 0, 0x02, "ATTRIB", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_ARRAY"), (101, "ENDBLK", "BLK_ARRAY")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_minsert_entities",
        lambda _path: [
            (
                200,
                5.0,
                6.0,
                0.0,
                1.0,
                1.0,
                1.0,
                0.0,
                (2, 2, 2.5, 3.5, "BLK_ARRAY"),
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_attrib_entities",
        lambda _path: [
            (
                210,
                "ARRAY_VAL",
                "ARRAY_TAG",
                None,
                (5.0, 6.0, 0.0),
                None,
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 1.2, 0.0, 1.0),
                (0, 0, 0),
                0,
                False,
                (None, 200),
            )
        ],
    )

    output = tmp_path / "minsert_expand_out.dxf"
    doc = document_module.Document(path="dummy_minsert_expand.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="MINSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 4

    insert_points = {
        (float(insert.dxf.insert.x), float(insert.dxf.insert.y)) for insert in inserts
    }
    assert insert_points == {(5.0, 6.0), (7.5, 6.0), (5.0, 9.5), (7.5, 9.5)}

    attrib_points = {
        (float(insert.attribs[0].dxf.insert.x), float(insert.attribs[0].dxf.insert.y))
        for insert in inserts
    }
    assert attrib_points == insert_points
    assert all(len(insert.attribs) == 1 for insert in inserts)
    assert all(insert.attribs[0].dxf.tag == "ARRAY_TAG" for insert in inserts)
    assert all(insert.attribs[0].dxf.text == "ARRAY_VAL" for insert in inserts)
    assert len(dxf_entities_of_type(output, "POINT")) == 0


def test_to_dxf_writes_minsert_with_rotation_expanded_offsets(
    monkeypatch, tmp_path: Path
) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x13, "LINE", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x08, "MINSERT", "Entity"),
            (210, 14, 0, 0x02, "ATTRIB", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_ROT"), (101, "ENDBLK", "BLK_ROT")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_minsert_entities",
        lambda _path: [
            (
                200,
                10.0,
                20.0,
                0.0,
                1.0,
                1.0,
                1.0,
                math.pi / 2.0,
                (2, 2, 2.0, 3.0, "BLK_ROT"),
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_attrib_entities",
        lambda _path: [
            (
                210,
                "ROT_VAL",
                "ROT_TAG",
                None,
                (10.0, 20.0, 0.0),
                None,
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 1.0, 0.0, 1.0),
                (0, 0, 0),
                0,
                False,
                (None, 200),
            )
        ],
    )

    output = tmp_path / "minsert_rot_expand_out.dxf"
    doc = document_module.Document(path="dummy_minsert_rot_expand.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="MINSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 4

    expected = {(10.0, 20.0), (10.0, 22.0), (7.0, 20.0), (7.0, 22.0)}
    insert_points = {
        (round(float(insert.dxf.insert.x), 6), round(float(insert.dxf.insert.y), 6))
        for insert in inserts
    }
    assert insert_points == expected

    attrib_points = {
        (
            round(float(insert.attribs[0].dxf.insert.x), 6),
            round(float(insert.attribs[0].dxf.insert.y), 6),
        )
        for insert in inserts
    }
    assert attrib_points == expected


def test_to_dxf_block_export_materializes_helper_vertices(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._polyline_sequence_relationships.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x7F00, "POLYLINE", "Entity"),
            (111, 12, 0, 0x0A, "VERTEX_2D", "Entity"),
            (112, 13, 0, 0x0A, "VERTEX_2D", "Entity"),
            (113, 14, 0, 0x06, "SEQEND", "Entity"),
            (101, 15, 0, 0x05, "ENDBLK", "Entity"),
            (200, 16, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_HELP"), (101, "ENDBLK", "BLK_HELP")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [(200, 5.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_HELP")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                110,
                0,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_polyline_2d_entities_interpreted", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_2d_entities",
        lambda _path: [
            (111, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (112, 0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(110, "POLYLINE_2D", [111, 112], [], 113)],
    )

    output = tmp_path / "insert_helper_block_out.dxf"
    doc = document_module.Document(path="dummy_insert_helper_block.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "BLK_HELP"

    block = dxf_doc.blocks.get("BLK_HELP")
    assert len(list(block.query("LWPOLYLINE"))) == 1


def test_to_dxf_block_export_reuses_owner_map_for_helpers(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._polyline_sequence_relationships.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x7F00, "POLYLINE", "Entity"),
            (111, 12, 0, 0x0A, "VERTEX_2D", "Entity"),
            (112, 13, 0, 0x0A, "VERTEX_2D", "Entity"),
            (113, 14, 0, 0x06, "SEQEND", "Entity"),
            (101, 15, 0, 0x05, "ENDBLK", "Entity"),
            (120, 16, 0, 0x04, "BLOCK", "Entity"),
            (130, 17, 0, 0x7F00, "POLYLINE", "Entity"),
            (131, 18, 0, 0x0A, "VERTEX_2D", "Entity"),
            (132, 19, 0, 0x0A, "VERTEX_2D", "Entity"),
            (133, 20, 0, 0x06, "SEQEND", "Entity"),
            (121, 21, 0, 0x05, "ENDBLK", "Entity"),
            (200, 22, 0, 0x07, "INSERT", "Entity"),
            (201, 23, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [
            (100, "BLOCK", "BLK_HELP_A"),
            (101, "ENDBLK", "BLK_HELP_A"),
            (120, "BLOCK", "BLK_HELP_B"),
            (121, "ENDBLK", "BLK_HELP_B"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [
            (200, 5.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_HELP_A"),
            (201, 15.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_HELP_B"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                110,
                0,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            ),
            (
                130,
                0,
                [
                    (0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (10.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            ),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_polyline_2d_entities_interpreted", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_2d_entities",
        lambda _path: [
            (111, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (112, 0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (131, 0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (132, 0, 10.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [
            (110, "POLYLINE_2D", [111, 112], [], 113),
            (130, "POLYLINE_2D", [131, 132], [], 133),
        ],
    )

    original_entities_by_handle = convert_module._entities_by_handle
    counter = {"calls": 0}

    def counted_entities_by_handle(layout, types):
        counter["calls"] += 1
        return original_entities_by_handle(layout, types)

    monkeypatch.setattr(convert_module, "_entities_by_handle", counted_entities_by_handle)

    output = tmp_path / "insert_helper_block_perf_out.dxf"
    doc = document_module.Document(path="dummy_insert_helper_block_perf.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 2
    assert result.written_entities == 2
    # Expected calls:
    # 1) block traversal for INSERT references
    # 2) one-time owner polyline fetch reused across blocks
    #
    # Block member resolution is now keyed by (handle, dxftype), so it no
    # longer goes through _entities_by_handle.
    assert counter["calls"] == 2


def test_entities_by_handle_skips_invalid_handles_and_iteration_errors() -> None:
    class DummyLayout:
        def __init__(self) -> None:
            self.calls: list[str] = []

        def query(self, dxftype: str):
            self.calls.append(dxftype)
            if dxftype == "ARC":
                def arc_iter():
                    yield Entity(dxftype="ARC", handle=0x200, dxf={})
                    raise RuntimeError("broken arc iterator")

                return arc_iter()
            if dxftype == "CIRCLE":
                raise RuntimeError("failed to build query")
            if dxftype == "LINE":
                return iter(
                    [
                        Entity(dxftype="LINE", handle=0x100, dxf={}),
                        Entity(dxftype="LINE", handle="bad-handle", dxf={}),
                        Entity(dxftype="LINE", handle=0x101, dxf={}),
                    ]
                )
            return iter([])

    layout = DummyLayout()
    entities_by_handle = convert_module._entities_by_handle(layout, {"LINE", "ARC", "CIRCLE"})

    assert layout.calls == ["ARC", "CIRCLE", "LINE"]
    assert sorted(entities_by_handle) == [0x100, 0x101, 0x200]
    assert entities_by_handle[0x200].dxftype == "ARC"


def test_collect_referenced_block_names_trims_insert_names() -> None:
    block_members_by_name = {
        "BLK_A": [(10, "INSERT"), (11, "LINE")],
        "BLK_B": [(20, "LINE")],
    }
    insert_entities_by_handle = {
        10: Entity(dxftype="INSERT", handle=10, dxf={"name": "  BLK_B  "}),
    }

    selected = convert_module._collect_referenced_block_names(
        block_members_by_name,
        {"BLK_A"},
        insert_entities_by_handle,
    )

    assert selected == {"BLK_A", "BLK_B"}


def test_collect_referenced_block_names_includes_minsert_references() -> None:
    block_members_by_name = {
        "BLK_A": [(10, "MINSERT"), (11, "LINE")],
        "BLK_B": [(20, "LINE")],
    }
    insert_entities_by_handle = {
        10: Entity(dxftype="MINSERT", handle=10, dxf={"name": " BLK_B "}),
    }

    selected = convert_module._collect_referenced_block_names(
        block_members_by_name,
        {"BLK_A"},
        insert_entities_by_handle,
    )

    assert selected == {"BLK_A", "BLK_B"}


def test_collect_block_members_by_name_keeps_first_duplicate_definition() -> None:
    rows = [
        (100, 10, 0, 0x04, "BLOCK", "Entity"),
        (101, 11, 0, 0x13, "LINE", "Entity"),
        (102, 12, 0, 0x05, "ENDBLK", "Entity"),
        (200, 20, 0, 0x04, "BLOCK", "Entity"),
        (201, 21, 0, 0x13, "LINE", "Entity"),
        (202, 22, 0, 0x05, "ENDBLK", "Entity"),
    ]

    members = convert_module._collect_block_members_by_name(
        rows,
        {
            100: "BLK_DUP",
            200: "BLK_DUP",
        },
    )

    assert members == {"BLK_DUP": [(101, "LINE")]}


def test_collect_block_members_by_name_keeps_first_shadow_duplicate_at_same_offset() -> None:
    rows = [
        (100, 10, 0, 0x04, "BLOCK", "Entity"),
        (200, 10, 0, 0x04, "BLOCK", "Entity"),
        (101, 11, 0, 0x13, "LINE", "Entity"),
        (102, 12, 0, 0x05, "ENDBLK", "Entity"),
    ]

    members = convert_module._collect_block_members_by_name(
        rows,
        {
            100: "_Open30",
            200: "*D193",
        },
    )

    assert members == {"_Open30": [(101, "LINE")]}


def test_normalize_recursive_block_insert_remaps_dimension_self_reference() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={"name": "*D180", "insert": (0.0, 0.0, 0.0)},
    )

    normalized = convert_module._normalize_recursive_block_insert(
        entity,
        block_name="*D180",
        known_block_names={"*D180", "_Small"},
    )

    assert normalized is not None
    assert normalized.dxf["name"] == "_Small"


def test_normalize_recursive_block_insert_prefers_open30_when_requested() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={"name": "*D180", "insert": (0.0, 0.0, 0.0)},
    )

    normalized = convert_module._normalize_recursive_block_insert(
        entity,
        block_name="*D180",
        known_block_names={"*D180", "_Small", "_Open30"},
        prefer_open30=True,
    )

    assert normalized is not None
    assert normalized.dxf["name"] == "_Open30"


def test_normalize_recursive_block_insert_skips_unresolved_self_reference() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={"name": "BLK_LOOP", "insert": (0.0, 0.0, 0.0)},
    )

    normalized = convert_module._normalize_recursive_block_insert(
        entity,
        block_name="BLK_LOOP",
        known_block_names={"BLK_LOOP"},
    )

    assert normalized is None


def test_normalize_recursive_block_insert_keeps_non_self_cycle_for_non_dimension_block() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={"name": "ACAD_DETAILVIEWSTYLE", "insert": (0.0, 0.0, 0.0)},
    )

    normalized = convert_module._normalize_recursive_block_insert(
        entity,
        block_name="i",
        known_block_names={"i", "ACAD_DETAILVIEWSTYLE"},
        recursive_target_names={"i", "ACAD_DETAILVIEWSTYLE"},
    )

    assert normalized is not None
    assert normalized.dxf["name"] == "ACAD_DETAILVIEWSTYLE"


def test_normalize_problematic_insert_name_remaps_i_to_open30() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "i",
            "insert": (0.0, 0.0, 0.0),
            "xscale": -60.0,
            "yscale": 60.0,
            "zscale": 60.0,
            "rotation": 69.60362427681007,
        },
    )

    normalized = convert_module._normalize_problematic_insert_name(
        entity,
        available_block_names={"_Open30", "i"},
    )

    assert normalized.dxf["name"] == "_Open30"


def test_normalize_problematic_insert_name_keeps_i_without_open30() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "i",
            "insert": (0.0, 0.0, 0.0),
            "xscale": -60.0,
            "yscale": 60.0,
            "zscale": 60.0,
            "rotation": 69.60362427681007,
        },
    )

    normalized = convert_module._normalize_problematic_insert_name(
        entity,
        available_block_names={"i"},
    )

    assert normalized.dxf["name"] == "i"


def test_normalize_problematic_insert_name_keeps_i_for_small_scale() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "i",
            "insert": (0.0, 0.0, 0.0),
            "xscale": 1.0,
            "yscale": 1.0,
            "zscale": 1.0,
        },
    )

    normalized = convert_module._normalize_problematic_insert_name(
        entity,
        available_block_names={"_Open30", "i"},
    )

    assert normalized.dxf["name"] == "i"


def test_normalize_problematic_insert_name_keeps_i_for_orthogonal_rotation() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "i",
            "insert": (0.0, 0.0, 0.0),
            "xscale": -60.0,
            "yscale": 60.0,
            "zscale": 60.0,
            "rotation": 270.0,
        },
    )

    normalized = convert_module._normalize_problematic_insert_name(
        entity,
        available_block_names={"_Open30", "i"},
    )

    assert normalized.dxf["name"] == "i"


def test_maybe_prefer_modelspace_filtered_entities_for_open30_layout(monkeypatch) -> None:
    layout = type(
        "_Layout",
        (),
        {
            "doc": type("_Doc", (), {"decode_path": "dummy.dwg"})(),
        },
    )()
    entities = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Model_Space",
                "insert": (0.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        )
    ]
    entities.extend(
        Entity(dxftype="POINT", handle=1000 + i, dxf={"location": (float(i), 0.0, 0.0)})
        for i in range(220)
    )
    entities.extend(
        Entity(dxftype="MTEXT", handle=2000 + i, dxf={"insert": (float(i), 0.0, 0.0), "text": "T"})
        for i in range(90)
    )

    filtered = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Model_Space",
                "insert": (0.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
        Entity(dxftype="POINT", handle=9001, dxf={"location": (0.0, 0.0, 0.0)}),
        Entity(dxftype="MTEXT", handle=9002, dxf={"insert": (0.0, 0.0, 0.0), "text": "T"}),
    ]

    monkeypatch.setattr(
        convert_module,
        "_filter_modelspace_entities",
        lambda *_args, **_kwargs: filtered,
    )

    resolved = convert_module._maybe_prefer_modelspace_filtered_entities(layout, entities)
    assert resolved is filtered


def test_maybe_prefer_modelspace_filtered_entities_restores_open30_markers(
    monkeypatch,
) -> None:
    layout = type(
        "_Layout",
        (),
        {
            "doc": type("_Doc", (), {"decode_path": "dummy.dwg"})(),
        },
    )()
    entities = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Model_Space",
                "insert": (33000.0, 11127.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 90.0,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=2,
            dxf={
                "name": "_Open30",
                "insert": (5872.0, 11427.0, 0.0),
                "xscale": -60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 69.6,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=3,
            dxf={
                "name": "_Open30",
                "insert": (13369.0, 11527.0, 0.0),
                "xscale": -60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 69.6,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=4,
            dxf={
                "name": "_Open30",
                "insert": (20526.0, 11427.0, 0.0),
                "xscale": -60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 69.6,
            },
        ),
    ]
    entities.extend(
        Entity(
            dxftype="LINE",
            handle=1000 + i,
            dxf={"start": (float(i), 0.0, 0.0), "end": (float(i), 1.0, 0.0)},
        )
        for i in range(100)
    )
    entities.extend(
        Entity(dxftype="POINT", handle=2000 + i, dxf={"location": (float(i), 0.0, 0.0)})
        for i in range(80)
    )
    entities.extend(
        Entity(dxftype="MTEXT", handle=3000 + i, dxf={"insert": (float(i), 0.0, 0.0), "text": "T"})
        for i in range(40)
    )

    filtered = [
        entities[0],
    ]
    filtered.extend(entities[4:84])
    filtered.append(Entity(dxftype="POINT", handle=9001, dxf={"location": (0.0, 0.0, 0.0)}))
    filtered.append(
        Entity(dxftype="MTEXT", handle=9002, dxf={"insert": (0.0, 0.0, 0.0), "text": "T"})
    )

    monkeypatch.setattr(
        convert_module,
        "_filter_modelspace_entities",
        lambda *_args, **_kwargs: filtered,
    )

    resolved = convert_module._maybe_prefer_modelspace_filtered_entities(layout, entities)

    insert_names = [
        convert_module._normalize_block_name(entity.dxf.get("name"))
        for entity in resolved
        if entity.dxftype == "INSERT"
    ]
    assert insert_names.count("_Open30") == 3
    assert insert_names.count("*Model_Space") == 1


def test_maybe_prefer_modelspace_filtered_entities_keeps_full_set_when_lines_drop_too_much(
    monkeypatch,
) -> None:
    layout = type(
        "_Layout",
        (),
        {
            "doc": type("_Doc", (), {"decode_path": "dummy.dwg"})(),
        },
    )()
    entities = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Model_Space",
                "insert": (0.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        )
    ]
    entities.extend(
        Entity(
            dxftype="LINE",
            handle=1000 + i,
            dxf={"start": (float(i), 0.0, 0.0), "end": (float(i), 1.0, 0.0)},
        )
        for i in range(220)
    )
    entities.extend(
        Entity(dxftype="POINT", handle=3000 + i, dxf={"location": (float(i), 0.0, 0.0)})
        for i in range(220)
    )
    entities.extend(
        Entity(dxftype="MTEXT", handle=4000 + i, dxf={"insert": (float(i), 0.0, 0.0), "text": "T"})
        for i in range(80)
    )
    filtered = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Model_Space",
                "insert": (0.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
        Entity(dxftype="LINE", handle=9001, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
        Entity(dxftype="POINT", handle=9002, dxf={"location": (0.0, 0.0, 0.0)}),
        Entity(dxftype="MTEXT", handle=9003, dxf={"insert": (0.0, 0.0, 0.0), "text": "T"}),
    ]

    monkeypatch.setattr(
        convert_module,
        "_filter_modelspace_entities",
        lambda *_args, **_kwargs: filtered,
    )

    resolved = convert_module._maybe_prefer_modelspace_filtered_entities(layout, entities)
    assert resolved is entities


def test_maybe_prefer_modelspace_filtered_entities_keeps_full_set_when_core_retention_is_too_low(
    monkeypatch,
) -> None:
    layout = type(
        "_Layout",
        (),
        {
            "doc": type("_Doc", (), {"decode_path": "dummy.dwg"})(),
        },
    )()
    entities = [
        Entity(
            dxftype="INSERT",
            handle=100 + i,
            dxf={
                "name": "BLK",
                "insert": (float(i), 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        )
        for i in range(600)
    ]
    entities.extend(
        Entity(
            dxftype="LINE",
            handle=1000 + i,
            dxf={"start": (float(i), 0.0, 0.0), "end": (float(i), 1.0, 0.0)},
        )
        for i in range(200)
    )
    entities.extend(
        Entity(dxftype="POINT", handle=3000 + i, dxf={"location": (float(i), 0.0, 0.0)})
        for i in range(200)
    )
    entities.extend(
        Entity(dxftype="MTEXT", handle=4000 + i, dxf={"insert": (float(i), 0.0, 0.0), "text": "T"})
        for i in range(200)
    )
    filtered = [
        Entity(
            dxftype="INSERT",
            handle=100 + i,
            dxf={
                "name": "BLK",
                "insert": (float(i), 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        )
        for i in range(20)
    ]
    filtered.extend(
        Entity(
            dxftype="LINE",
            handle=9000 + i,
            dxf={"start": (float(i), 0.0, 0.0), "end": (float(i), 1.0, 0.0)},
        )
        for i in range(10)
    )
    filtered.append(Entity(dxftype="POINT", handle=9500, dxf={"location": (0.0, 0.0, 0.0)}))
    filtered.append(
        Entity(dxftype="MTEXT", handle=9501, dxf={"insert": (0.0, 0.0, 0.0), "text": "T"})
    )

    monkeypatch.setattr(
        convert_module,
        "_filter_modelspace_entities",
        lambda *_args, **_kwargs: filtered,
    )

    resolved = convert_module._maybe_prefer_modelspace_filtered_entities(layout, entities)
    assert resolved is entities


def test_drop_small_local_non_modelspace_geometry_drops_dense_local_cluster(
    monkeypatch,
) -> None:
    entities = [
        Entity(
            dxftype="LINE",
            handle=10,
            dxf={"start": (0.0, 0.0, 0.0), "end": (5.0, 0.0, 0.0)},
        ),
        Entity(
            dxftype="LINE",
            handle=9000,
            dxf={"start": (5000.0, 0.0, 0.0), "end": (5010.0, 0.0, 0.0)},
        ),
    ]
    for index in range(40):
        entities.append(
            Entity(
                dxftype="ARC",
                handle=1000 + index,
                dxf={"center": (float(index) * 5.0, 10.0, 0.0), "radius": 2.0},
            )
        )
    for index in range(8):
        entities.append(
            Entity(
                dxftype="LWPOLYLINE",
                handle=2000 + index,
                dxf={"points": [(float(index), 0.0, 0.0), (float(index), 4.0, 0.0)]},
            )
        )

    monkeypatch.setattr(
        convert_module,
        "_resolve_modelspace_entity_handles",
        lambda *_args, **_kwargs: ({10}, {32}),
    )

    resolved = convert_module._drop_small_local_non_modelspace_geometry(
        "dummy.dwg",
        entities,
    )
    handles = {int(entity.handle) for entity in resolved}
    assert 10 in handles
    assert 9000 in handles
    assert 1000 not in handles
    assert 2000 not in handles


def test_drop_small_local_non_modelspace_geometry_keeps_sparse_local_entities(
    monkeypatch,
) -> None:
    entities = [
        Entity(
            dxftype="LINE",
            handle=10,
            dxf={"start": (0.0, 0.0, 0.0), "end": (5.0, 0.0, 0.0)},
        )
    ]
    for index in range(12):
        entities.append(
            Entity(
                dxftype="ARC",
                handle=1000 + index,
                dxf={"center": (float(index) * 5.0, 10.0, 0.0), "radius": 2.0},
            )
        )

    monkeypatch.setattr(
        convert_module,
        "_resolve_modelspace_entity_handles",
        lambda *_args, **_kwargs: ({10}, {32}),
    )

    resolved = convert_module._drop_small_local_non_modelspace_geometry(
        "dummy.dwg",
        entities,
    )
    assert resolved is entities


def test_replace_layout_alias_with_open30_right_sheet_inserts() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))
    doc.blocks.new(name="__EZDWG_LAYOUT_ALIAS_MODEL_SPACE")

    for x, y in [
        (5872.77817695745, 11427.437111958425),
        (13369.389088356227, 11527.438221728415),
        (20526.169088356546, 11427.437111958425),
    ]:
        ref = msp.add_blockref("_Open30", (x, y, 0.0))
        ref.dxf.xscale = -60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0
        ref.dxf.rotation = 69.60362427681007

    msp.add_blockref("__EZDWG_LAYOUT_ALIAS_MODEL_SPACE", (1000.0, 0.0, 0.0))

    convert_module._replace_layout_alias_with_open30_right_sheet_inserts(msp)

    inserts = list(msp.query("INSERT"))
    assert len(inserts) == 6
    assert all(entity.dxf.name == "_Open30" for entity in inserts)

    positions = {
        round(float(entity.dxf.insert.x), 6)
        for entity in inserts
    }
    for x in (31102.778177, 38599.389088, 45756.169088):
        assert x in positions


def test_replace_layout_alias_with_open30_right_sheet_inserts_uses_right_side_i_proxies() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))
    proxy_block = doc.blocks.new(name="i")
    detail_block = doc.blocks.new(name="ACAD_DETAILVIEWSTYLE")
    detail_block.add_line((0.0, 0.0), (1.0, 0.0))
    proxy_block.add_blockref("ACAD_DETAILVIEWSTYLE", (0.0, 0.0, 0.0))

    for x, y in [
        (5872.77817695745, 11427.437111958425),
        (13369.389088356227, 11527.438221728415),
        (20526.169088356546, 11427.437111958425),
    ]:
        ref = msp.add_blockref("_Open30", (x, y, 0.0))
        ref.dxf.xscale = -60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0
        ref.dxf.rotation = 69.60362427681007

    for x, y in [
        (32996.77817673411, 11127.437111960724),
        (36300.52554946847, 11722.438221743563),
        (44620.163173014764, 3546.9791822203115),
    ]:
        ref = msp.add_blockref("i", (x, y, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0
        ref.dxf.rotation = 90.0

    convert_module._replace_layout_alias_with_open30_right_sheet_inserts(
        msp,
        has_right_side_open30_i_proxies=True,
    )

    inserts = [entity for entity in msp.query("INSERT") if entity.dxf.name == "_Open30"]
    positions = {
        round(float(entity.dxf.insert.x), 6)
        for entity in inserts
    }
    for x in (31102.778177, 38599.389088, 45756.169088):
        assert x in positions


def test_prepare_dxf_layers_falls_back_for_invalid_decoded_layer_name() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")

    mapping = convert_module._prepare_dxf_layers(
        doc,
        {0x80: (7, None), 0x85: (2, None)},
        {0x80: 'BAD"NAME', 0x85: "SD-FRAME_TEXT"},
    )

    assert mapping[0x80] == "LAYER_80"
    assert "LAYER_80" in doc.layers
    assert mapping[0x85] == "SD-FRAME_TEXT"
    assert "SD-FRAME_TEXT" in doc.layers


def test_has_right_side_open30_i_proxies() -> None:
    entities = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "i",
                "insert": (32996.77817673411, 11127.437111960724, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 90.0,
            },
        )
    ]

    assert convert_module._has_right_side_open30_i_proxies(entities) is True


def test_has_right_side_open30_i_proxies_accepts_layout_pseudo_signal() -> None:
    entities = [
        Entity(
            dxftype="INSERT",
            handle=1,
            dxf={
                "name": "*Paper_Space",
                "insert": (36300.52554946847, 11722.438221743563, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 270.0,
            },
        )
    ]

    assert convert_module._has_right_side_open30_i_proxies(entities) is True


def test_rebalance_sparse_open30_right_sheet_geometry_moves_left_entities_to_right() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))

    for x in (1000.0, 2000.0, 3000.0, 33000.0, 34000.0, 35000.0):
        ref = msp.add_blockref("_Open30", (x, 1000.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    for i in range(1000):
        x = 1000.0 + i * 20.0
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))

    convert_module._rebalance_sparse_open30_right_sheet_geometry(msp)

    left = 0
    right = 0
    for line in msp.query("LINE"):
        x_mid = (float(line.dxf.start.x) + float(line.dxf.end.x)) * 0.5
        if x_mid < 25230.0:
            left += 1
        else:
            right += 1

    assert left == 1000
    assert right == 1000


def test_rebalance_sparse_open30_right_sheet_geometry_shifts_overlapped_duplicates_first() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))

    for x in (1000.0, 2000.0, 3000.0, 33000.0, 34000.0, 35000.0):
        ref = msp.add_blockref("_Open30", (x, 1000.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    # 400 overlapped pairs should split left/right; 50 uniques should remain left.
    for i in range(400):
        x = 1200.0 + i * 20.0
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))
    for i in range(50):
        x = 6000.0 + i * 60.0
        msp.add_line((x, 1400.0, 0.0), (x + 40.0, 1500.0, 0.0))

    convert_module._rebalance_sparse_open30_right_sheet_geometry(msp)

    left = 0
    right = 0
    for line in msp.query("LINE"):
        x_mid = (float(line.dxf.start.x) + float(line.dxf.end.x)) * 0.5
        if x_mid < 25230.0:
            left += 1
        else:
            right += 1

    assert left + right == 900
    assert left == 450
    assert right == 450


def test_rebalance_sparse_open30_right_sheet_text_clones_inner_window_text_only() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))

    for x in (1000.0, 2000.0, 3000.0, 33000.0, 34000.0, 35000.0):
        ref = msp.add_blockref("_Open30", (x, 1000.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    left_w = convert_module._OPEN30_LEFT_OUTER_WIDTH
    left_h = convert_module._OPEN30_LEFT_OUTER_HEIGHT
    gap = convert_module._OPEN30_SHEET_GAP
    inner_w = convert_module._OPEN30_INNER_WIDTH
    inner_h = convert_module._OPEN30_INNER_HEIGHT
    left_rect = msp.add_lwpolyline(
        [(0.0, 0.0), (left_w, 0.0), (left_w, left_h), (0.0, left_h)],
        format="xy",
        close=True,
    )
    right_rect = msp.add_lwpolyline(
        [
            (left_w + gap, 1500.0),
            (left_w + gap + inner_w, 1500.0),
            (left_w + gap + inner_w, 1500.0 + inner_h),
            (left_w + gap, 1500.0 + inner_h),
        ],
        format="xy",
        close=True,
    )

    for i in range(500):
        x = 1200.0 + i * 20.0
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))

    msp.add_text("LEFT_TEXT", dxfattribs={"insert": (2000.0, 600.0, 0.0)})
    msp.add_text("LEFT_TEXT_2", dxfattribs={"insert": (2600.0, 2400.0, 0.0)})
    msp.add_mtext("LEFT_MTEXT", dxfattribs={"insert": (3200.0, 2600.0, 0.0)})
    msp.add_text(
        "LEFT_TEXT_2",
        dxfattribs={"insert": (2600.0 + convert_module._OPEN30_LEFT_OUTER_WIDTH, 2400.0, 0.0)},
    )

    convert_module._rebalance_sparse_open30_right_sheet_text(msp)

    right_texts = [
        text.dxf.text
        for text in msp.query("TEXT")
        if float(text.dxf.insert.x) >= 25230.0
    ]
    right_mtexts = [
        mtext.text
        for mtext in msp.query("MTEXT")
        if float(mtext.dxf.insert.x) >= 25230.0
    ]
    assert right_texts == ["LEFT_TEXT_2"]
    assert right_mtexts == ["LEFT_MTEXT"]


def test_rebalance_sparse_open30_right_sheet_geometry_skips_when_open30_inserts_are_few() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    open30_block = doc.blocks.new(name="_Open30")
    open30_block.add_line((0.0, 0.0), (1.0, 0.0))

    for x in (1000.0, 2000.0, 3000.0):
        ref = msp.add_blockref("_Open30", (x, 1000.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    for i in range(10):
        x = 1000.0 + i * 200.0
        msp.add_line((x, 1000.0, 0.0), (x + 50.0, 1200.0, 0.0))

    convert_module._rebalance_sparse_open30_right_sheet_geometry(msp)

    left = 0
    right = 0
    for line in msp.query("LINE"):
        x_mid = (float(line.dxf.start.x) + float(line.dxf.end.x)) * 0.5
        if x_mid < 25230.0:
            left += 1
        else:
            right += 1

    assert left == 10
    assert right == 0


def test_write_text_like_skips_implausible_garbage_text() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    valid_text = "Project A-01 建築"

    assert convert_module._write_text_like(
        msp,
        {"text": valid_text, "insert": (10.0, 20.0, 0.0)},
        {},
    )
    assert convert_module._write_text_like(
        msp,
        {"text": "A\x00B\x01C\x02D", "insert": (0.0, 0.0, 0.0)},
        {},
    ) is False
    assert convert_module._write_mtext(
        msp,
        {"text": "A\x00B\x01C\x02D", "insert": (0.0, 0.0, 0.0)},
        {},
    ) is False

    texts = list(msp.query("TEXT"))
    mtexts = list(msp.query("MTEXT"))
    assert len(texts) == 1
    assert texts[0].dxf.text == valid_text
    assert len(mtexts) == 0


def test_entities_by_handle_and_type_keeps_duplicate_handles_by_dxftype() -> None:
    class _DummyLayout:
        def query(self, dxftype, include_styles=True):
            if dxftype == "LINE":
                return [
                    Entity(
                        dxftype="LINE",
                        handle=42,
                        dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)},
                    )
                ]
            if dxftype == "TEXT":
                return [
                    Entity(
                        dxftype="TEXT",
                        handle=42,
                        dxf={"text": "A", "insert": (0.0, 0.0, 0.0)},
                    )
                ]
            return []

    result = convert_module._entities_by_handle_and_type_multi(
        _DummyLayout(),
        {"LINE", "TEXT"},
    )

    assert (42, "LINE") in result
    assert (42, "TEXT") in result
    assert result[(42, "LINE")][0].dxftype == "LINE"
    assert result[(42, "TEXT")][0].dxftype == "TEXT"


def test_deduplicate_layout_pseudo_inserts_by_handle_keeps_first_preserved_insert() -> None:
    entities = [
        Entity(
            dxftype="INSERT",
            handle=252,
            dxf={
                "name": "*Model_Space",
                "insert": (10.0, 10.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=252,
            dxf={
                "name": "*Model_Space",
                "insert": (20.0, 20.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
        Entity(dxftype="LINE", handle=300, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
    ]

    deduped = convert_module._deduplicate_layout_pseudo_inserts_by_handle(entities)
    assert len(deduped) == 2
    assert deduped[0].handle == 252
    assert deduped[1].dxftype == "LINE"


def test_deduplicate_layout_pseudo_inserts_by_handle_keeps_only_first_layout_insert() -> None:
    entities = [
        Entity(
            dxftype="INSERT",
            handle=10,
            dxf={
                "name": "*Model_Space",
                "insert": (100.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=11,
            dxf={
                "name": "*Model_Space",
                "insert": (200.0, 0.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
            },
        ),
    ]

    deduped = convert_module._deduplicate_layout_pseudo_inserts_by_handle(entities)
    assert len(deduped) == 1
    assert int(deduped[0].handle) == 10


def test_deduplicate_layout_pseudo_inserts_by_handle_keeps_first_rotation_variant() -> None:
    entities = [
        Entity(
            dxftype="INSERT",
            handle=252,
            dxf={
                "name": "*Model_Space",
                "insert": (44620.16, 3546.97, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 90.0,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=34,
            dxf={
                "name": "*Model_Space",
                "insert": (32996.77, 11127.43, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 90.0,
            },
        ),
        Entity(
            dxftype="INSERT",
            handle=252,
            dxf={
                "name": "*Model_Space",
                "insert": (46950.16, 10157.43, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 270.0,
            },
        ),
    ]

    deduped = convert_module._deduplicate_layout_pseudo_inserts_by_handle(entities)
    assert len(deduped) == 1
    assert float(deduped[0].dxf["rotation"]) == 90.0


def test_block_prefers_open30_arrowhead_for_ch_dimension_text() -> None:
    entities = [
        Entity(dxftype="LINE", handle=1, dxf={}),
        Entity(dxftype="MTEXT", handle=2, dxf={"text": r"\A1;CH3300"}),
    ]
    assert convert_module._block_prefers_open30_arrowhead(entities)


def test_block_prefers_open30_arrowhead_ignores_non_ch_text() -> None:
    entities = [
        Entity(dxftype="TEXT", handle=1, dxf={"text": "W 3045"}),
        Entity(dxftype="MTEXT", handle=2, dxf={"text": r"\A1;390"}),
    ]
    assert not convert_module._block_prefers_open30_arrowhead(entities)


def test_referenced_block_name_from_entity_keeps_modelspace_layout_copy() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "*Model_Space",
            "insert": (0.0, 0.0, 0.0),
            "xscale": 60.0,
            "yscale": 60.0,
            "zscale": 60.0,
        },
    )

    assert convert_module._referenced_block_name_from_entity(entity) == "*Model_Space"


def test_referenced_block_name_from_entity_skips_paper_space_layout_copy() -> None:
    entity = Entity(
        dxftype="INSERT",
        handle=10,
        dxf={
            "name": "*Paper_Space",
            "insert": (0.0, 0.0, 0.0),
            "xscale": 60.0,
            "yscale": 60.0,
            "zscale": 60.0,
        },
    )

    assert convert_module._referenced_block_name_from_entity(entity) is None


def test_filter_modelspace_entities_uses_block_boundaries(monkeypatch) -> None:
    monkeypatch.setattr(
        convert_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (10, 100, 0, 0x04, "BLOCK", "Entity"),
            (11, 110, 0, 0x13, "LINE", "Entity"),
            (12, 120, 0, 0x05, "ENDBLK", "Entity"),
            (20, 200, 0, 0x04, "BLOCK", "Entity"),
            (21, 210, 0, 0x13, "LINE", "Entity"),
            (22, 220, 0, 0x07, "INSERT", "Entity"),
            (23, 230, 0, 0x05, "ENDBLK", "Entity"),
        ],
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [
            (10, "*Paper_Space"),
            (20, "*Model_Space"),
        ],
    )

    selected = [
        Entity(dxftype="LINE", handle=11, dxf={}),
        Entity(dxftype="LINE", handle=21, dxf={}),
        Entity(dxftype="INSERT", handle=22, dxf={"name": "BLK1"}),
    ]
    filtered = convert_module._filter_modelspace_entities("dummy_modelspace_filter.dwg", selected)

    assert [int(entity.handle) for entity in filtered] == [21, 22]


def test_filter_modelspace_entities_keeps_entities_owned_by_modelspace_block(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        convert_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (10, 100, 0, 0x04, "BLOCK", "Entity"),
            (11, 110, 0, 0x13, "LINE", "Entity"),
            (12, 120, 0, 0x05, "ENDBLK", "Entity"),
            (20, 200, 0, 0x04, "BLOCK", "Entity"),
            (21, 210, 0, 0x13, "LINE", "Entity"),
            (22, 220, 0, 0x05, "ENDBLK", "Entity"),
            (30, 300, 0, 0x2C, "MTEXT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [
            (10, "*Paper_Space"),
            (20, "*Model_Space"),
        ],
    )

    selected = [
        Entity(dxftype="LINE", handle=11, dxf={}),
        Entity(dxftype="LINE", handle=21, dxf={}),
        Entity(dxftype="MTEXT", handle=30, dxf={"owner_handle": 20}),
    ]

    filtered = convert_module._filter_modelspace_entities("dummy_modelspace_filter.dwg", selected)

    assert [int(entity.handle) for entity in filtered] == [21, 30]


def test_filter_modelspace_entities_skips_nested_blocks_inside_modelspace(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        convert_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (20, 200, 0, 0x04, "BLOCK", "Entity"),
            (21, 210, 0, 0x13, "LINE", "Entity"),
            (30, 300, 0, 0x04, "BLOCK", "Entity"),
            (31, 310, 0, 0x13, "LINE", "Entity"),
            (32, 320, 0, 0x05, "ENDBLK", "Entity"),
            (22, 330, 0, 0x13, "LINE", "Entity"),
            (23, 340, 0, 0x05, "ENDBLK", "Entity"),
        ],
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [
            (20, "*Model_Space"),
            (30, "NESTED"),
        ],
    )

    selected = [
        Entity(dxftype="LINE", handle=21, dxf={}),
        Entity(dxftype="LINE", handle=31, dxf={}),
        Entity(dxftype="LINE", handle=22, dxf={}),
    ]

    filtered = convert_module._filter_modelspace_entities("dummy_modelspace_filter.dwg", selected)

    assert [int(entity.handle) for entity in filtered] == [21, 22]


def test_filter_modelspace_entities_prefers_explicit_owner_over_handle_range(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        convert_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (20, 200, 0, 0x04, "BLOCK", "Entity"),
            (21, 210, 0, 0x13, "LINE", "Entity"),
            (22, 220, 0, 0x05, "ENDBLK", "Entity"),
        ],
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [
            (20, "*Model_Space"),
        ],
    )

    selected = [
        Entity(dxftype="LINE", handle=30, dxf={"owner_handle": 20}),
        Entity(dxftype="LINE", handle=21, dxf={"owner_handle": 999}),
    ]

    filtered = convert_module._filter_modelspace_entities("dummy_modelspace_filter.dwg", selected)

    assert [int(entity.handle) for entity in filtered] == [30]


def test_maybe_filter_modelspace_block_references_keeps_non_block_geometry(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        convert_module,
        "_resolve_modelspace_entity_handles",
        lambda _path, decode_cache=None: ({21}, {20}),
    )

    entities = [Entity(dxftype="LINE", handle=100, dxf={})]
    entities.extend(
        Entity(dxftype="INSERT", handle=200 + index, dxf={"name": "BLK", "owner_handle": 20})
        for index in range(8)
    )
    entities.extend(
        Entity(dxftype="INSERT", handle=300 + index, dxf={"name": "BLK", "owner_handle": 10})
        for index in range(32)
    )

    filtered = convert_module._maybe_filter_modelspace_block_references(
        "dummy_modelspace_filter.dwg",
        entities,
    )

    assert [entity.dxftype for entity in filtered].count("LINE") == 1
    assert [entity.dxftype for entity in filtered].count("INSERT") == 8
    assert all(int(entity.handle) < 300 for entity in filtered if entity.dxftype == "INSERT")


def test_drop_pathological_modelspace_block_references_removes_heavy_repeated_blocks() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()

    keep = doc.blocks.new(name="KEEP")
    keep.add_line((0.0, 0.0), (1.0, 0.0))

    dim_block = doc.blocks.new(name="*D900")
    dim_block.add_line((0.0, 0.0), (1.0, 0.0))

    heavy = doc.blocks.new(name="HEAVY_REPEAT")
    for index in range(2100):
        x = 5000.0 + float(index)
        heavy.add_line((x, 0.0), (x, 10.0))
    for index in range(40):
        heavy.add_blockref("*D900", (0.0, 0.0, 0.0))

    for index in range(140):
        ref = msp.add_blockref("HEAVY_REPEAT", (float(index) * 100.0, 0.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    msp.add_blockref("KEEP", (0.0, 0.0, 0.0))

    convert_module._drop_pathological_modelspace_block_references(msp)

    insert_names = [entity.dxf.name for entity in msp.query("INSERT")]
    assert insert_names == ["KEEP"]
    assert "HEAVY_REPEAT" not in doc.blocks
    assert "*D900" not in doc.blocks
    assert "KEEP" in doc.blocks


def test_drop_unresolved_block_references_removes_missing_nested_inserts() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()

    host = doc.blocks.new(name="HOST")
    host.add_blockref("MISSING_CHILD", (0.0, 0.0, 0.0))
    host.add_line((0.0, 0.0), (1.0, 0.0))

    msp.add_blockref("HOST", (0.0, 0.0, 0.0))

    convert_module._drop_unresolved_block_references(doc)

    host_entities = list(doc.blocks.get("HOST"))
    assert len([entity for entity in host_entities if entity.dxftype() == "INSERT"]) == 0
    assert len([entity for entity in host_entities if entity.dxftype() == "LINE"]) == 1


def test_prune_implausible_entities_from_repeated_large_blocks() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()

    heavy = doc.blocks.new(name="HEAVY_REPEAT")
    for index in range(2100):
        x = 5000.0 + float(index)
        heavy.add_line((x, 0.0), (x, 10.0))
    heavy.add_lwpolyline([(0.0, 0.0), (50000.0, 20000.0)])
    heavy.add_arc((1.0, 0.0), 1.0e-9, 0.0, 0.0)
    heavy.add_ray((1.0, 1.0), (1.0, 0.0))
    heavy.add_3dface(
        [
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
        ]
    )

    for index in range(20):
        ref = msp.add_blockref("HEAVY_REPEAT", (float(index) * 100.0, 0.0, 0.0))
        ref.dxf.xscale = 60.0
        ref.dxf.yscale = 60.0
        ref.dxf.zscale = 60.0

    convert_module._prune_implausible_entities_from_repeated_large_blocks(msp)

    block = doc.blocks.get("HEAVY_REPEAT")
    type_counts = Counter(entity.dxftype() for entity in block)
    assert type_counts.get("LINE", 0) == 2100
    assert type_counts.get("LWPOLYLINE", 0) == 0
    assert type_counts.get("ARC", 0) == 0
    assert type_counts.get("RAY", 0) == 0
    assert type_counts.get("3DFACE", 0) == 0


def test_drop_implausible_modelspace_primitives_removes_tiny_origin_geometry() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    msp.add_line((100.0, 0.0), (110.0, 0.0))
    msp.add_lwpolyline([(0.0, 0.0), (50000.0, 20000.0)])
    msp.add_lwpolyline([(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)])
    msp.add_arc((1.0, 0.0), 1.0e-9, 0.0, 0.0)
    msp.add_ray((1.0, 1.0), (1.0, 0.0))
    msp.add_3dface(
        [
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
        ]
    )

    convert_module._drop_implausible_modelspace_primitives(msp)

    type_counts = Counter(entity.dxftype() for entity in msp)
    assert type_counts.get("LINE", 0) == 1
    assert type_counts.get("LWPOLYLINE", 0) == 1
    assert type_counts.get("ARC", 0) == 0
    assert type_counts.get("RAY", 0) == 0
    assert type_counts.get("3DFACE", 0) == 0


def test_drop_small_local_origin_geometry_cluster_removes_dense_origin_cluster() -> None:
    pytest.importorskip("ezdxf")

    ezdxf = convert_module._require_ezdxf()
    doc = ezdxf.new(dxfversion="R2010")
    msp = doc.modelspace()
    msp.add_line((20000.0, 0.0), (20010.0, 0.0))

    for index in range(40):
        center_x = float(index) * 5.0
        msp.add_arc((center_x, 10.0), 2.0, 0.0, 90.0)
    for index in range(8):
        msp.add_lwpolyline([(float(index), 0.0), (float(index), 4.0)])

    convert_module._drop_small_local_origin_geometry_cluster(msp)

    type_counts = Counter(entity.dxftype() for entity in msp)
    assert type_counts.get("LINE", 0) == 1
    assert type_counts.get("ARC", 0) == 0
    assert type_counts.get("LWPOLYLINE", 0) == 0


def test_resolve_block_name_by_handle_prefers_exact_mapping(monkeypatch) -> None:
    header_rows = [
        (100, 10, 0, 0x04, "BLOCK", "Entity"),
        (101, 11, 0, 0x05, "ENDBLK", "Entity"),
        (200, 20, 0, 0x04, "BLOCK", "Entity"),
        (201, 21, 0, 0x05, "ENDBLK", "Entity"),
    ]

    monkeypatch.setattr(
        convert_module,
        "_resolve_block_name_by_handle_exact",
        lambda _path: {100: "BLK_A", 200: "BLK_B"},
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [(100, "WRONG_A"), (200, "WRONG_B")],
    )

    resolved = convert_module._resolve_block_name_by_handle("dummy.dwg", header_rows)
    assert resolved[100] == "BLK_A"
    assert resolved[200] == "BLK_B"


def test_resolve_block_name_by_handle_preserves_layout_pseudo_header_names(
    monkeypatch,
) -> None:
    header_rows = [
        (28, 10, 0, 0x04, "BLOCK", "Entity"),
        (32, 11, 0, 0x04, "BLOCK", "Entity"),
        (100, 12, 0, 0x04, "BLOCK", "Entity"),
    ]

    monkeypatch.setattr(
        convert_module,
        "_resolve_block_name_by_handle_exact",
        lambda _path: {28: "*D2052", 32: "*D2135", 100: "BLK_A"},
    )
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [
            (28, "*Paper_Space"),
            (32, "*Model_Space"),
            (100, "BLK_A"),
        ],
    )

    resolved = convert_module._resolve_block_name_by_handle("dummy.dwg", header_rows)
    assert resolved[28] == "*Paper_Space"
    assert resolved[32] == "*Model_Space"
    assert resolved[100] == "BLK_A"


def test_resolve_block_name_by_handle_does_not_use_positional_fallback(monkeypatch) -> None:
    header_rows = [
        (100, 10, 0, 0x04, "BLOCK", "Entity"),
        (101, 11, 0, 0x05, "ENDBLK", "Entity"),
        (200, 20, 0, 0x04, "BLOCK", "Entity"),
        (201, 21, 0, 0x05, "ENDBLK", "Entity"),
    ]

    monkeypatch.setattr(convert_module, "_resolve_block_name_by_handle_exact", lambda _path: {})
    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_header_names",
        lambda _path, _limit=None: [(100, "BLK_A")],
    )

    resolved = convert_module._resolve_block_name_by_handle("dummy.dwg", header_rows)
    assert resolved == {100: "BLK_A"}


def test_resolve_block_end_name_by_handle_exact_fills_missing_endblk_names(monkeypatch) -> None:
    header_rows = [
        (100, 10, 0, 0x04, "BLOCK", "Entity"),
        (101, 11, 0, 0x05, "ENDBLK", "Entity"),
        (200, 20, 0, 0x04, "BLOCK", "Entity"),
        (201, 21, 0, 0x05, "ENDBLK", "Entity"),
    ]

    monkeypatch.setattr(
        convert_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_A"), (101, "ENDBLK", "BLK_A")],
    )

    resolved = convert_module._resolve_block_end_name_by_handle_exact(
        "dummy.dwg",
        header_rows=header_rows,
        block_name_by_handle={100: "BLK_A", 200: "BLK_B"},
    )
    assert resolved[101] == "BLK_A"
    assert resolved[201] == "BLK_B"


def test_materialize_export_entities_dedup_skips_only_identical_rows() -> None:
    doc = ezdwg.read(str(SAMPLES / "line_2007.dwg"))
    layout = doc.modelspace()
    selected = [
        Entity(dxftype="LINE", handle=10, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
        Entity(dxftype="LINE", handle=10, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
    ]
    out = convert_module._materialize_export_entities(layout, selected)
    assert len(out) == 1


def test_materialize_export_entities_keeps_same_handle_when_geometry_differs() -> None:
    doc = ezdwg.read(str(SAMPLES / "line_2007.dwg"))
    layout = doc.modelspace()
    selected = [
        Entity(dxftype="LINE", handle=10, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
        Entity(dxftype="LINE", handle=10, dxf={"start": (0.0, 0.0, 0.0), "end": (2.0, 0.0, 0.0)}),
    ]
    out = convert_module._materialize_export_entities(layout, selected)
    assert len(out) == 2


def test_materialize_export_entities_keeps_same_handle_across_types() -> None:
    doc = ezdwg.read(str(SAMPLES / "line_2007.dwg"))
    layout = doc.modelspace()
    selected = [
        Entity(dxftype="LINE", handle=10, dxf={"start": (0.0, 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
        Entity(dxftype="DIMENSION", handle=10, dxf={"dimtype": "DIM_LINEAR", "text": "1"}),
    ]
    out = convert_module._materialize_export_entities(layout, selected)
    assert len(out) == 2


def test_resolve_export_entities_does_not_filter_modelspace_by_default(
    monkeypatch,
) -> None:
    _source_path, layout = convert_module._resolve_layout(str(SAMPLES / "line_2007.dwg"))
    called = False

    def _spy_filter(
        _decode_path: str | None,
        entities: list[Entity],
        *,
        decode_cache=None,
    ) -> list[Entity]:
        nonlocal called
        called = True
        return entities

    monkeypatch.setattr(convert_module, "_filter_modelspace_entities", _spy_filter)
    entities = convert_module._resolve_export_entities(layout, "LINE", include_styles=False)

    assert len(entities) == 1
    assert called is False


def test_resolve_export_entities_filters_modelspace_when_enabled(monkeypatch) -> None:
    _source_path, layout = convert_module._resolve_layout(str(SAMPLES / "line_2007.dwg"))
    called = False

    def _spy_filter(
        _decode_path: str | None,
        entities: list[Entity],
        *,
        decode_cache=None,
    ) -> list[Entity]:
        nonlocal called
        called = True
        return entities

    monkeypatch.setattr(convert_module, "_filter_modelspace_entities", _spy_filter)
    entities = convert_module._resolve_export_entities(
        layout,
        "LINE",
        include_styles=False,
        modelspace_only=True,
    )

    assert len(entities) == 1
    assert called is True


def test_to_dxf_block_export_trims_insert_block_name(monkeypatch, tmp_path: Path) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 0x13, "LINE", "Entity"),
            (101, 12, 0, 0x05, "ENDBLK", "Entity"),
            (200, 13, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_TRIM"), (101, "ENDBLK", "BLK_TRIM")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [(200, 5.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "  BLK_TRIM  ")],
    )

    output = tmp_path / "insert_trimmed_name_out.dxf"
    doc = document_module.Document(path="dummy_insert_trimmed_name.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    inserts = list(dxf_doc.modelspace().query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "BLK_TRIM"
    assert len(dxf_entities_of_type(output, "POINT")) == 0

    block = dxf_doc.blocks.get("BLK_TRIM")
    assert len(list(block.query("LINE"))) == 1


def test_to_dxf_block_export_uses_offset_order_for_block_members(
    monkeypatch, tmp_path: Path
) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    # Header rows are intentionally out of sequence by handle:
    # member LINE appears after ENDBLK in this list, but its offset is inside the block.
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 100, 0, 0x04, "BLOCK", "Entity"),
            (101, 300, 0, 0x05, "ENDBLK", "Entity"),
            (110, 200, 0, 0x13, "LINE", "Entity"),
            (200, 400, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "BLK_OFS"), (101, "ENDBLK", "BLK_OFS")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(110, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [(200, 5.0, 5.0, 0.0, 1.0, 1.0, 1.0, 0.0, "BLK_OFS")],
    )

    output = tmp_path / "insert_offset_order_out.dxf"
    doc = document_module.Document(path="dummy_insert_offset_order.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1

    dxf_doc = ezdxf.readfile(str(output))
    block = dxf_doc.blocks.get("BLK_OFS")
    assert len(list(block.query("LINE"))) == 1


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


def test_to_dxf_rejects_unsupported_dim_block_policy(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    with pytest.raises(ValueError, match="unsupported dim-block policy"):
        convert_module.to_dxf(
            str(SAMPLES / "line_2007.dwg"),
            str(tmp_path / "invalid_dim_policy_out.dxf"),
            types="LINE",
            dxf_version="R2010",
            dim_block_policy="invalid-policy",
        )


def test_to_dxf_dimension_writes_native_dimension_without_line_fallback(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    source = ROOT / "examples" / "data" / "mechanical_example-imperial.dwg"
    output = tmp_path / "mechanical_dim_out.dxf"
    result = ezdwg.to_dxf(
        str(source),
        str(output),
        types="DIMENSION",
        dxf_version="R2010",
        explode_dimensions=False,
    )

    assert output.exists()
    assert result.total_entities > 0
    assert result.written_entities == result.total_entities
    assert len(dxf_entities_of_type(output, "DIMENSION")) > 0
    assert len(dxf_entities_of_type(output, "LINE")) == 0


def test_to_dxf_dimension_default_explodes_to_primitives(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    source = ROOT / "examples" / "data" / "mechanical_example-imperial.dwg"
    output = tmp_path / "mechanical_dim_exploded_out.dxf"
    result = ezdwg.to_dxf(str(source), str(output), types="DIMENSION", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities > 0
    assert result.written_entities == result.total_entities
    assert len(dxf_entities_of_type(output, "DIMENSION")) == 0
    assert len(dxf_entities_of_type(output, "LINE")) > 0


def test_write_dimension_native_falls_back_to_anonymous_block_insert() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D1")
    block.add_line((0.0, 0.0), (1.0, 0.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="DIMENSION",
            handle=1,
            dxf={
                "dimtype": "ANG2LN",
                "anonymous_block_name": "*D1",
                "insert": (10.0, 20.0, 0.0),
                "insert_scale": (2.0, 3.0, 1.0),
                "insert_rotation": 15.0,
            },
        ),
        explode_dimensions=True,
    )

    assert written is True
    inserts = list(modelspace.query("INSERT"))
    assert len(inserts) == 1
    ref = inserts[0]
    assert ref.dxf.name == "*D1"
    assert tuple(ref.dxf.insert) == (10.0, 20.0, 0.0)
    assert ref.dxf.xscale == 2.0
    assert ref.dxf.yscale == 3.0
    assert ref.dxf.zscale == 1.0
    assert ref.dxf.rotation == 15.0


def test_write_dimension_native_prefers_anonymous_block_for_linear_dimension() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D3")
    block.add_line((0.0, 0.0), (1.0, 0.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="DIMENSION",
            handle=4,
            dxf={
                "dimtype": "DIM_LINEAR",
                "anonymous_block_name": "*D3",
                "defpoint": (0.0, 0.0, 0.0),
                "defpoint2": (10.0, 0.0, 0.0),
                "defpoint3": (10.0, 10.0, 0.0),
                "text_midpoint": (5.0, 6.0, 0.0),
            },
        ),
        explode_dimensions=True,
    )

    assert written is True
    inserts = list(modelspace.query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "*D3"


def test_write_dimension_native_skips_placeholder_geometry_without_artifacts() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()
    before_lines = len(list(modelspace.query("LINE")))

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="DIMENSION",
            handle=2,
            dxf={
                "dimtype": "ANG2LN",
                "defpoint": (0.0, 0.0, 0.0),
                "defpoint2": (0.0, 0.0, 0.0),
                "defpoint3": (0.0, 0.0, 0.0),
                "text_midpoint": (0.0, 0.0, 0.0),
                "text": "",
            },
        ),
        explode_dimensions=True,
    )

    assert written is True
    after_lines = len(list(modelspace.query("LINE")))
    assert after_lines == before_lines


def test_write_dimension_native_placeholder_prefers_block_fallback() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D2")
    block.add_line((0.0, 0.0), (1.0, 0.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="DIMENSION",
            handle=3,
            dxf={
                "dimtype": "ANG3PT",
                "anonymous_block_name": "*D2",
                "defpoint": (0.0, 0.0, 0.0),
                "defpoint2": (0.0, 0.0, 0.0),
                "defpoint3": (0.0, 0.0, 0.0),
                "text_midpoint": (0.0, 0.0, 0.0),
            },
        ),
        explode_dimensions=True,
    )

    assert written is True
    inserts = list(modelspace.query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "*D2"


def test_write_insert_skips_anonymous_dimension_insert_after_dimension_success() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D4")
    block.add_line((0.0, 0.0), (1.0, 0.0))
    modelspace = doc.modelspace()
    dimension_context = convert_module._DimensionWriteContext()

    dim_written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="DIMENSION",
            handle=4010,
            dxf={
                "dimtype": "ANG2LN",
                "anonymous_block_name": "*D4",
                "insert": (12.0, 34.0, 0.0),
                "insert_scale": (2.0, 2.0, 1.0),
                "insert_rotation": 10.0,
            },
        ),
        explode_dimensions=True,
        dim_block_policy="smart",
        dimension_context=dimension_context,
    )
    assert dim_written is True
    assert len(list(modelspace.query("INSERT"))) == 1

    insert_written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4011,
            dxf={
                "name": "*D4",
                "insert": (12.0, 34.0, 0.0),
                "xscale": 2.0,
                "yscale": 2.0,
                "zscale": 1.0,
                "rotation": 10.0,
            },
        ),
        explode_dimensions=False,
        dim_block_policy="smart",
        dimension_context=dimension_context,
    )

    assert insert_written is True
    # Only the DIMENSION fallback INSERT should remain.
    assert len(list(modelspace.query("INSERT"))) == 1


def test_write_insert_skips_layout_pseudo_block_names() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4100,
            dxf={
                "name": "*Paper_Space",
                "insert": (10.0, 20.0, 0.0),
                "xscale": 1.0,
                "yscale": 1.0,
                "zscale": 1.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0
    assert len(list(modelspace.query("POINT"))) == 0


def test_write_insert_keeps_modelspace_layout_copy_block_names(monkeypatch) -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    doc.blocks.new(name="ALIAS_MODELSPACE").add_line((0.0, 0.0), (1.0, 0.0))
    modelspace = doc.modelspace()
    monkeypatch.setattr(
        convert_module,
        "_ensure_layout_pseudo_block_alias",
        lambda _doc, _name: "ALIAS_MODELSPACE",
    )

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4101,
            dxf={
                "name": "*Model_Space",
                "insert": (10.0, 20.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 90.0,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    inserts = list(modelspace.query("INSERT"))
    assert len(inserts) == 1
    assert inserts[0].dxf.name == "ALIAS_MODELSPACE"
    assert len(list(modelspace.query("POINT"))) == 0


def test_ensure_layout_pseudo_block_alias_skips_nested_anonymous_dimension_inserts() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    source = doc.blocks.get("*Model_Space")
    dim_block_name = "*D901"
    regular_block_name = "REG_ALIAS_CHILD"
    if doc.blocks.get(dim_block_name) is None:
        doc.blocks.new(name=dim_block_name).add_line((0.0, 0.0), (1.0, 0.0))
    if doc.blocks.get(regular_block_name) is None:
        doc.blocks.new(name=regular_block_name).add_line((0.0, 0.0), (1.0, 0.0))
    source.add_line((0.0, 0.0), (10.0, 0.0))
    source.add_blockref(dim_block_name, (1.0, 1.0))
    source.add_blockref(regular_block_name, (2.0, 2.0))

    alias_name = convert_module._ensure_layout_pseudo_block_alias(doc, "*Model_Space")

    assert isinstance(alias_name, str)
    alias = doc.blocks.get(alias_name)
    insert_names = [insert.dxf.name for insert in alias.query("INSERT")]
    assert dim_block_name not in insert_names
    assert regular_block_name in insert_names
    assert len(list(alias.query("LINE"))) >= 1


def test_write_insert_skips_empty_block_definitions() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    doc.blocks.new(name="EMPTY_BLK")
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4103,
            dxf={
                "name": "EMPTY_BLK",
                "insert": (10.0, 20.0, 0.0),
                "xscale": 1.0,
                "yscale": 1.0,
                "zscale": 1.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_insert_skips_placeholder_anonymous_dimension_insert() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4104,
            dxf={
                "name": "*D180",
                "insert": (0.0, 0.0, 0.0),
                "xscale": 1.0,
                "yscale": 1.0,
                "zscale": 1.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_insert_skips_nested_anonymous_dimension_block() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    child = doc.blocks.new(name="*D_CHILD")
    child.add_line((0.0, 0.0), (1.0, 0.0))
    root = doc.blocks.new(name="*D_ROOT")
    root.add_blockref("*D_CHILD", (0.0, 0.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4105,
            dxf={
                "name": "*D_ROOT",
                "insert": (10.0, 20.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_insert_keeps_scaled_anonymous_dimension_block_in_smart_policy() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D_BIG")
    block.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4106,
            dxf={
                "name": "*D_BIG",
                "insert": (100.0, 200.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
        dim_block_policy="smart",
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 1


def test_write_insert_skips_scaled_anonymous_dimension_block_in_smart_policy_with_context() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D_BIG")
    block.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    modelspace = doc.modelspace()
    dimension_context = convert_module._DimensionWriteContext()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=41061,
            dxf={
                "name": "*D_BIG",
                "insert": (100.0, 200.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
        dim_block_policy="smart",
        dimension_context=dimension_context,
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_insert_skips_anonymous_dimension_block_with_implausible_extent() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D_BAD")
    block.add_line((1.0e20, 0.0), (1.0e20 + 100.0, 0.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=41060,
            dxf={
                "name": "*D_BAD",
                "insert": (100.0, 200.0, 0.0),
                "xscale": 1.0,
                "yscale": 1.0,
                "zscale": 1.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
        dim_block_policy="smart",
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_insert_skips_suspicious_scaled_anonymous_dimension_block_in_legacy_policy() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    block = doc.blocks.new(name="*D_BIG")
    block.add_line((20000.0, 30000.0), (20100.0, 30000.0))
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="INSERT",
            handle=4107,
            dxf={
                "name": "*D_BIG",
                "insert": (100.0, 200.0, 0.0),
                "xscale": 60.0,
                "yscale": 60.0,
                "zscale": 60.0,
                "rotation": 0.0,
            },
        ),
        explode_dimensions=False,
        dim_block_policy="legacy",
    )

    assert written is True
    assert len(list(modelspace.query("INSERT"))) == 0


def test_write_lwpolyline_drops_degenerate_width_only_geometry() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace_unsafe(
        modelspace,
        Entity(
            dxftype="LWPOLYLINE",
            handle=4101,
            dxf={
                "points": [
                    (0.0, 0.0, 0.0),
                    (0.0, 0.0, 0.0),
                    (0.0, 0.0, 0.0),
                ],
                "widths": [(1.0, 0.0), (1.0, 0.0), (1.0, 0.0)],
                "bulges": [0.0, 0.0, 0.0],
                "closed": False,
            },
        ),
        explode_dimensions=False,
    )

    assert written is True
    assert len(list(modelspace.query("LWPOLYLINE"))) == 0


def test_write_entity_skips_implausible_coordinate_ranges() -> None:
    ezdxf = pytest.importorskip("ezdxf")

    doc = ezdxf.new(dxfversion="R2010")
    modelspace = doc.modelspace()

    written = convert_module._write_entity_to_modelspace(
        modelspace,
        Entity(
            dxftype="SOLID",
            handle=4102,
            dxf={
                "points": [
                    (1.0e200, 0.0, 0.0),
                    (0.0, 1.0e200, 0.0),
                    (0.0, 0.0, 0.0),
                    (0.0, 0.0, 0.0),
                ]
            },
        ),
        explode_dimensions=False,
    )

    assert written is False
    assert len(list(modelspace.query("SOLID"))) == 0


def test_to_dxf_vertex_filter_writes_owner_polyline(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._polyline_sequence_relationships.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_2d_entities",
        lambda _path: [
            (0x5101, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            (0x5102, 0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x5001,
                0,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(0x5001, "POLYLINE_2D", [0x5101, 0x5102], [], 0x51FF)],
    )

    output = tmp_path / "vertex_owner_out.dxf"
    doc = document_module.Document(path="dummy_vertex_convert_owner.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="VERTEX_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    entities = dxf_entities_of_type(output, "LWPOLYLINE")
    assert len(entities) == 1
    points = dxf_lwpolyline_points(entities[0])
    assert points == [(0.0, 0.0, 0.0), (10.0, 0.0, 0.0)]


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


def test_to_dxf_treats_empty_polyline_2d_as_placeholder(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [(0x2D10, 0x0000, [])],
    )

    output = tmp_path / "polyline2d_empty_placeholder_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_empty_placeholder.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert len(dxf_entities_of_type(output, "LWPOLYLINE")) == 0
    assert len(dxf_entities_of_type(output, "POINT")) == 0


def test_to_dxf_preserves_explicit_closing_vertex_for_open_polyline_2d(
    monkeypatch,
    tmp_path: Path,
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
                0x2D04,
                0x0000,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )

    output = tmp_path / "polyline2d_open_explicit_close_out.dxf"
    doc = document_module.Document(path="dummy_polyline2d_open_explicit_close.dwg", version="AC1018")
    result = ezdwg.to_dxf(doc, str(output), types="POLYLINE_2D", dxf_version="R2010")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    entities = dxf_entities_of_type(output, "LWPOLYLINE")
    assert len(entities) == 1
    points = dxf_lwpolyline_points(entities[0])
    assert points == [
        (0.0, 0.0, 0.0),
        (2.0, 0.0, 0.0),
        (2.0, 1.0, 0.0),
        (0.0, 1.0, 0.0),
        (0.0, 0.0, 0.0),
    ]
    assert group_float(entities[0], "70") == 0.0


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
