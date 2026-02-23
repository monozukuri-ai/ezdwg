from __future__ import annotations

import math

import ezdwg.document as document_module


def test_normalize_types_uses_present_entity_types_when_types_none(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (1, 0, 0, 0x13, "LINE", "Entity"),
            (2, 0, 0, 0x11, "ARC", "Entity"),
            (3, 0, 0, 0x15, "DIM_LINEAR", "Entity"),
            (4, 0, 0, 0x33, "LAYER", "Object"),
        ],
    )

    types = document_module._normalize_types(None, "dummy.dwg")
    assert "LINE" in types
    assert "ARC" in types
    assert "DIMENSION" in types
    assert "LWPOLYLINE" not in types


def test_normalize_types_explicit_tokens_do_not_require_present_scan(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: (_ for _ in ()).throw(
            AssertionError("present-type scan should not be called for explicit tokens")
        ),
    )

    types = document_module._normalize_types("LINE ARC", "dummy.dwg")
    assert types == ["LINE", "ARC"]


def test_query_none_skips_absent_entity_decoders(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (1, 0, 0, 0x13, "LINE", "Entity"),
            (2, 0, 0, 0x11, "ARC", "Entity"),
        ],
    )
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_arc_circle_entities",
        lambda _path: (
            [(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            [(2, 0.0, 0.0, 0.0, 1.0, 0.0, math.pi / 2.0)],
            [],
        ),
    )
    monkeypatch.setattr(document_module.raw, "decode_line_entities", lambda _path: [(1, 0, 0, 0, 1, 0, 0)])
    monkeypatch.setattr(
        document_module.raw,
        "decode_arc_entities",
        lambda _path: [(2, 0, 0, 0, 1.0, 0.0, math.pi / 2.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_circle_entities",
        lambda _path: (_ for _ in ()).throw(AssertionError("CIRCLE decoder should not be called")),
    )

    doc = document_module.Document(path="dummy.dwg", version="AC1018")
    entities = list(doc.modelspace().query())
    assert [entity.dxftype for entity in entities] == ["LINE", "ARC"]


def test_query_uses_bulk_line_arc_circle_decoder(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()
    document_module._line_arc_circle_rows.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (1, 0, 0, 0x13, "LINE", "Entity"),
            (2, 0, 0, 0x11, "ARC", "Entity"),
            (3, 0, 0, 0x12, "CIRCLE", "Entity"),
        ],
    )
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_arc_circle_entities",
        lambda _path: (
            [(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            [(2, 0.0, 0.0, 0.0, 1.0, 0.0, math.pi / 2.0)],
            [(3, 2.0, 2.0, 0.0, 0.5)],
        ),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: (_ for _ in ()).throw(AssertionError("legacy LINE decoder should not be called")),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_arc_entities",
        lambda _path: (_ for _ in ()).throw(AssertionError("legacy ARC decoder should not be called")),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_circle_entities",
        lambda _path: (_ for _ in ()).throw(AssertionError("legacy CIRCLE decoder should not be called")),
    )

    doc = document_module.Document(path="dummy.dwg", version="AC1018")
    entities = list(doc.modelspace().query())
    assert [entity.dxftype for entity in entities] == ["LINE", "ARC", "CIRCLE"]


def test_query_single_type_does_not_force_bulk_line_arc_circle_decoder(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()
    document_module._line_arc_circle_rows.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(1, 0, 0, 0x13, "LINE", "Entity")],
    )
    monkeypatch.setattr(document_module, "_entity_style_map", lambda _path: {})
    monkeypatch.setattr(document_module, "_layer_color_map", lambda _path: {})
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_arc_circle_entities",
        lambda _path: (_ for _ in ()).throw(AssertionError("bulk decoder should not be called")),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_line_entities",
        lambda _path: [(1, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0)],
    )

    doc = document_module.Document(path="dummy.dwg", version="AC1018")
    entities = list(doc.modelspace().query("LINE"))
    assert [entity.dxftype for entity in entities] == ["LINE"]


def test_normalize_types_excludes_explicit_only_entities_by_default(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (1, 0, 0, 0x13, "LINE", "Entity"),
            (2, 0, 0, 0x04, "BLOCK", "Entity"),
            (3, 0, 0, 0x05, "ENDBLK", "Entity"),
            (4, 0, 0, 0x02, "VERTEX_2D", "Entity"),
            (5, 0, 0, 0x06, "SEQEND", "Entity"),
        ],
    )

    types = document_module._normalize_types(None, "dummy.dwg")
    assert "LINE" in types
    assert "BLOCK" not in types
    assert "ENDBLK" not in types
    assert "VERTEX_2D" not in types
    assert "SEQEND" not in types


def test_query_can_explicitly_fetch_block_endblk_seqend_and_vertex(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._polyline_sequence_relationships.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (10, 0, 0, 0x04, "BLOCK", "Entity"),
            (11, 0, 0, 0x05, "ENDBLK", "Entity"),
            (20, 0, 0, 0x04, "BLOCK", "Entity"),
            (21, 0, 0, 0x05, "ENDBLK", "Entity"),
            (30, 0, 0, 0x06, "SEQEND", "Entity"),
            (40, 0, 0, 0x02, "VERTEX_2D", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_block_entity_names",
        lambda _path: [
            (10, "BLOCK", "BLK_A"),
            (20, "BLOCK", "BLK_B"),
            (11, "ENDBLK", "BLK_A"),
            (21, "ENDBLK", "BLK_B"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_2d_entities",
        lambda _path: [
            (
                40,
                0,
                1.0,
                2.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(10, "POLYLINE_2D", [40], [], 30)],
    )

    doc = document_module.Document(path="dummy_block_vertex.dwg", version="AC1018")
    entities = list(doc.modelspace().query("BLOCK ENDBLK SEQEND VERTEX_2D"))

    assert [entity.dxftype for entity in entities] == [
        "BLOCK",
        "BLOCK",
        "ENDBLK",
        "ENDBLK",
        "SEQEND",
        "VERTEX_2D",
    ]

    block_entities = [entity for entity in entities if entity.dxftype == "BLOCK"]
    endblk_entities = [entity for entity in entities if entity.dxftype == "ENDBLK"]
    seqend_entity = next(entity for entity in entities if entity.dxftype == "SEQEND")
    vertex_entity = next(entity for entity in entities if entity.dxftype == "VERTEX_2D")
    assert [entity.dxf.get("name") for entity in block_entities] == ["BLK_A", "BLK_B"]
    assert [entity.dxf.get("name") for entity in endblk_entities] == ["BLK_A", "BLK_B"]
    assert seqend_entity.dxf["owner_handle"] == 10
    assert seqend_entity.dxf["owner_type"] == "POLYLINE_2D"
    assert vertex_entity.dxf["position"] == (1.0, 2.0, 0.0)
    assert vertex_entity.dxf["owner_handle"] == 10
    assert vertex_entity.dxf["owner_type"] == "POLYLINE_2D"


def test_query_wildcard_can_fetch_explicit_only_vertex_types(monkeypatch) -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(40, 0, 0, 0x02, "VERTEX_2D", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_2d_entities",
        lambda _path: [(40, 0, 1.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0)],
    )

    doc = document_module.Document(path="dummy_vertex_only.dwg", version="AC1018")
    entities = list(doc.modelspace().query("VERTEX_*"))

    assert len(entities) == 1
    assert entities[0].dxftype == "VERTEX_2D"
    assert entities[0].dxf["position"] == (1.0, 2.0, 0.0)
