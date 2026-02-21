from __future__ import annotations

import ezdwg.document as document_module


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._long_transaction_record_map.cache_clear()


def test_query_long_transaction_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x4C, "LONG_TRANSACTION", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_long_transaction_entities",
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

    doc = document_module.Document(path="dummy_long_transaction.dwg", version="AC1021")
    entities = list(doc.modelspace().query("LONG_TRANSACTION"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "LONG_TRANSACTION"
    assert entity.handle == 100
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5
    assert entity.dxf["owner_handle"] is None
    assert entity.dxf["reactor_handles"] == []
    assert entity.dxf["xdic_obj_handle"] is None
    assert entity.dxf["extra_handles"] == []


def test_query_long_transaction_exposes_handle_topology(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(110, 0, 0, 0x4C, "LONG_TRANSACTION", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_long_transaction_entities",
        lambda _path: [(110, 12, [13, 14], 15, 16, 17, 18, [19, 20])],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_long_transaction_handles.dwg", version="AC1021")
    entity = next(doc.modelspace().query("LONG_TRANSACTION"))

    assert entity.handle == 110
    assert entity.dxf["owner_handle"] == 12
    assert entity.dxf["reactor_handles"] == [13, 14]
    assert entity.dxf["xdic_obj_handle"] == 15
    assert entity.dxf["ltype_handle"] == 16
    assert entity.dxf["plotstyle_handle"] == 17
    assert entity.dxf["material_handle"] == 18
    assert entity.dxf["extra_handles"] == [19, 20]
    assert entity.dxf["decoded_handle_refs"] == [12, 13, 14, 15, 16, 17, 18, 19, 20]


def test_query_long_transaction_exposes_record_diagnostics(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (120, 1, 0, 0x13, "LINE", "Entity"),
            (130, 2, 0, 0x13, "LINE", "Entity"),
            (140, 3, 0, 0x4C, "LONG_TRANSACTION", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_long_transaction_entities",
        lambda _path: [(140, None, [], None, None, None, None, [])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (
                140,
                1024,
                12,
                0x4C,
                b"\x78\x00\x00\x00\x82\x00\x00\x00LONGTX",
            )
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_long_transaction_record.dwg", version="AC1021")
    entity = next(doc.modelspace().query("LONG_TRANSACTION"))

    assert entity.handle == 140
    assert entity.dxf["record_offset"] == 1024
    assert entity.dxf["record_data_size"] == 12
    assert entity.dxf["record_type_code"] == 0x4C
    assert entity.dxf["record_size"] == 14
    assert entity.dxf["ascii_preview"] == "LONGTX"
    assert entity.dxf["likely_handle_refs"] == [120, 130]
    assert [item["handle"] for item in entity.dxf["likely_handle_ref_details"]] == [120, 130]


def test_query_none_can_include_long_transaction_when_present(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 0, 0, 0x4C, "LONG_TRANSACTION", "Entity"),
            (200, 0, 0, 0x33, "LAYER", "Object"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_long_transaction_entities",
        lambda _path: [(100,)],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_long_transaction_only.dwg", version="AC1021")
    entities = list(doc.modelspace().query())

    assert [entity.dxftype for entity in entities] == ["LONG_TRANSACTION"]
