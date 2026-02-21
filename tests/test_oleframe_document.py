from __future__ import annotations

import ezdwg.document as document_module


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._oleframe_record_map.cache_clear()
    document_module._ole2frame_record_map.cache_clear()


def test_query_oleframe_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x2B, "OLEFRAME", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_oleframe_entities",
        lambda _path: [(100,)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_entity_styles",
        lambda _path: [(100, 256, None, 7)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(7, 5, None)],
    )

    doc = document_module.Document(path="dummy_oleframe.dwg", version="AC1021")
    entities = list(doc.modelspace().query("OLEFRAME"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "OLEFRAME"
    assert entity.handle == 100
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5


def test_query_ole2frame_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(200, 0, 0, 0x4A, "OLE2FRAME", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_ole2frame_entities",
        lambda _path: [(200,)],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_ole2frame.dwg", version="AC1021")
    entities = list(doc.modelspace().query("OLE2FRAME"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "OLE2FRAME"
    assert entity.handle == 200


def test_query_oleframe_exposes_record_diagnostics(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 3, 0, 0x2B, "OLEFRAME", "Entity"),
            (120, 1, 0, 0x13, "LINE", "Entity"),
            (130, 2, 0, 0x13, "LINE", "Entity"),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_oleframe_entities", lambda _path: [(100,)])
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (
                100,
                512,
                14,
                0x2B,
                b"\x78\x00\x00\x00\x82\x00\x00\x00OLEFRAME",
            )
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_oleframe_record.dwg", version="AC1021")
    entity = next(doc.modelspace().query("OLEFRAME"))

    assert entity.handle == 100
    assert entity.dxf["record_offset"] == 512
    assert entity.dxf["record_data_size"] == 14
    assert entity.dxf["record_type_code"] == 0x2B
    assert entity.dxf["record_size"] == 16
    assert entity.dxf["ascii_preview"] == "OLEFRAME"
    assert entity.dxf["likely_handle_refs"] == [120, 130]
    assert [item["handle"] for item in entity.dxf["likely_handle_ref_details"]] == [120, 130]


def test_query_ole2frame_exposes_record_fallback_diagnostics(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(200, 1024, 18, 0x4A, "OLE2FRAME", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_ole2frame_entities", lambda _path: [(200,)])
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_ole2frame_record.dwg", version="AC1021")
    entity = next(doc.modelspace().query("OLE2FRAME"))

    assert entity.handle == 200
    assert entity.dxf["record_offset"] == 1024
    assert entity.dxf["record_data_size"] == 18
    assert entity.dxf["record_type_code"] == 0x4A
    assert entity.dxf["record_size"] is None
    assert entity.dxf["ascii_preview"] is None
    assert entity.dxf["likely_handle_refs"] == []
    assert entity.dxf["likely_handle_ref_details"] == []


def test_query_none_can_include_oleframe_and_ole2frame_when_present(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 0, 0, 0x2B, "OLEFRAME", "Entity"),
            (200, 0, 0, 0x4A, "OLE2FRAME", "Entity"),
            (300, 0, 0, 0x33, "LAYER", "Object"),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_oleframe_entities", lambda _path: [(100,)])
    monkeypatch.setattr(document_module.raw, "decode_ole2frame_entities", lambda _path: [(200,)])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_oleframes_only.dwg", version="AC1021")
    entities = list(doc.modelspace().query())

    assert [entity.dxftype for entity in entities] == ["OLEFRAME", "OLE2FRAME"]
