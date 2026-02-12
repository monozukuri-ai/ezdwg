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
