from __future__ import annotations

import math

import ezdwg.document as document_module
import ezdwg.render as render_module
from ezdwg.document import Document, Layout


def test_build_dimension_common_dxf_converts_angles_to_degrees() -> None:
    common = document_module._build_dimension_common_dxf(
        user_text="<>",
        text_midpoint=(10.0, 20.0, 0.0),
        insert_point=(11.0, 21.0, 0.0),
        extrusion=(0.0, 0.0, 1.0),
        insert_scale=(1.0, 1.0, 1.0),
        text_rotation=math.pi / 2.0,
        horizontal_direction=math.pi,
        dim_flags=3,
        actual_measurement=25.4,
        attachment_point=5,
        line_spacing_style=1,
        line_spacing_factor=1.0,
        insert_rotation=math.pi / 4.0,
        dimstyle_handle=0x10,
        anonymous_block_handle=0x20,
    )

    assert common["text"] == "<>"
    assert common["text_rotation"] == 90.0
    assert common["horizontal_direction"] == 180.0
    assert common["insert_rotation"] == 45.0
    assert common["actual_measurement"] == 25.4


def test_dimension_entity_contains_common_mapping(monkeypatch) -> None:
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_linear_entities",
        lambda _path: [
            (
                0x100,
                "<>",
                (1.0, 2.0, 0.0),
                (3.0, 2.0, 0.0),
                (3.0, 4.0, 0.0),
                (2.0, 3.0, 0.0),
                None,
                ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
                (math.pi / 6.0, math.pi / 3.0, math.pi / 4.0, math.pi / 2.0),
                (0, 12.5, None, None, None, math.pi / 8.0),
                (None, None),
            )
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_dim_radius_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_diameter_entities", lambda _path: [])

    doc = Document(
        path="dummy.dwg",
        version="AC1018",
        decode_path="dummy.dwg",
        decode_version="AC1018",
    )
    layout = Layout(doc=doc, name="MODELSPACE")
    entities = list(layout.query("DIMENSION"))
    assert len(entities) == 1

    dxf = entities[0].dxf
    assert dxf["dimtype"] == "LINEAR"
    assert dxf["common"]["text"] == dxf["text"]
    assert dxf["common"]["text_rotation"] == dxf["text_rotation"]
    assert dxf["common"]["horizontal_direction"] == dxf["horizontal_direction"]
    assert dxf["common"]["insert_rotation"] == dxf["insert_rotation"]
    assert dxf["oblique_angle"] == 45.0
    assert dxf["angle"] == 90.0


def test_dimension_entity_merges_linear_and_diameter(monkeypatch) -> None:
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_linear_entities",
        lambda _path: [
            (
                200,
                "<>",
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (2.0, 0.0, 0.0),
                (1.0, 0.5, 0.0),
                None,
                ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
                (0.0, 0.0, 0.0, 0.0),
                (0, 2.0, None, None, None, 0.0),
                (None, None),
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_radius_entities",
        lambda _path: [
            (
                150,
                "<>",
                (20.0, 20.0, 0.0),
                (20.0, 20.0, 0.0),
                (22.0, 20.0, 0.0),
                (21.0, 20.8, 0.0),
                None,
                ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
                (0.0, 0.0, 0.0, 0.0),
                (0, 2.0, None, None, None, 0.0),
                (None, None),
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_diameter_entities",
        lambda _path: [
            (
                100,
                "<>",
                (10.0, 10.0, 0.0),
                (8.0, 10.0, 0.0),
                (12.0, 10.0, 0.0),
                (10.0, 11.0, 0.0),
                None,
                ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
                (0.0, 0.0, 0.0, 0.0),
                (0, 4.0, None, None, None, 0.0),
                (None, None),
            )
        ],
    )

    doc = Document(
        path="dummy.dwg",
        version="AC1018",
        decode_path="dummy.dwg",
        decode_version="AC1018",
    )
    layout = Layout(doc=doc, name="MODELSPACE")
    entities = list(layout.query("DIMENSION"))

    assert [e.handle for e in entities] == [100, 150, 200]
    assert entities[0].dxf["dimtype"] == "DIAMETER"
    assert entities[1].dxf["dimtype"] == "RADIUS"
    assert entities[2].dxf["dimtype"] == "LINEAR"


def test_dimension_value_reads_common_mapping() -> None:
    dxf = {"common": {"actual_measurement": 7.25}}
    assert render_module._dimension_value(dxf, "actual_measurement") == 7.25
    assert render_module._resolve_dimension_text(dxf, "<>") == "7.25"


def test_dimension_entity_merges_extended_variants(monkeypatch) -> None:
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    base_row = (
        "<>",
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (2.0, 0.0, 0.0),
        (1.0, 0.5, 0.0),
        None,
        ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
        (0.0, 0.0, 0.0, 0.0),
        (0, 2.0, None, None, None, 0.0),
        (None, None),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_linear_entities",
        lambda _path: [(110, *base_row)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_ordinate_entities",
        lambda _path: [(90, *base_row)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_aligned_entities",
        lambda _path: [(100, *base_row)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_ang3pt_entities",
        lambda _path: [(80, *base_row)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_ang2ln_entities",
        lambda _path: [(70, *base_row)],
    )
    monkeypatch.setattr(document_module.raw, "decode_dim_radius_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_diameter_entities", lambda _path: [])

    doc = Document(
        path="dummy.dwg",
        version="AC1018",
        decode_path="dummy.dwg",
        decode_version="AC1018",
    )
    layout = Layout(doc=doc, name="MODELSPACE")
    entities = list(layout.query("DIMENSION"))

    assert [e.handle for e in entities] == [70, 80, 90, 100, 110]
    assert [e.dxf["dimtype"] for e in entities] == [
        "ANG2LN",
        "ANG3PT",
        "ORDINATE",
        "ALIGNED",
        "LINEAR",
    ]


def test_dimension_entity_bulk_decoder_merges_legacy_subtypes_without_duplicates(
    monkeypatch,
) -> None:
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    row_linear = (
        321,
        "<>",
        (1.0, 1.0, 0.0),
        (0.0, 0.0, 0.0),
        (2.0, 0.0, 0.0),
        (1.0, 0.5, 0.0),
        None,
        ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
        (0.0, 0.0, 0.0, 0.0),
        (0, 2.0, None, None, None, 0.0),
        (None, None),
    )
    row_ang3pt = (
        400,
        "<>",
        (2.0, 2.0, 0.0),
        (1.0, 1.0, 0.0),
        (3.0, 1.0, 0.0),
        (2.0, 1.5, 0.0),
        None,
        ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0)),
        (0.0, 0.0, 0.0, 0.0),
        (0, 2.0, None, None, None, 0.0),
        (None, None),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dimension_entities",
        lambda _path: [("LINEAR", row_linear)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_dim_linear_entities",
        lambda _path: [row_linear],  # duplicate by handle should be deduplicated
    )
    monkeypatch.setattr(document_module.raw, "decode_dim_ordinate_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_aligned_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_ang3pt_entities", lambda _path: [row_ang3pt])
    monkeypatch.setattr(document_module.raw, "decode_dim_ang2ln_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_radius_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_dim_diameter_entities", lambda _path: [])

    doc = Document(
        path="dummy.dwg",
        version="AC1018",
        decode_path="dummy.dwg",
        decode_version="AC1018",
    )
    layout = Layout(doc=doc, name="MODELSPACE")
    entities = list(layout.query("DIMENSION"))
    assert len(entities) == 2
    assert [entity.handle for entity in entities] == [321, 400]
    assert [entity.dxf["dimtype"] for entity in entities] == ["LINEAR", "ANG3PT"]
