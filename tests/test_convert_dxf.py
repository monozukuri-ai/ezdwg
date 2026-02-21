from __future__ import annotations

from pathlib import Path
import math

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
    # 2) all block member entities by type
    # 3) one-time owner polyline fetch reused across blocks
    assert counter["calls"] == 3


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
