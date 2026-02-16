from __future__ import annotations

import ezdwg.document as document_module


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_viewport_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x22, "VIEWPORT", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_viewport_entities",
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

    doc = document_module.Document(path="dummy_viewport.dwg", version="AC1021")
    entities = list(doc.modelspace().query("VIEWPORT"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "VIEWPORT"
    assert entity.handle == 100
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5


def test_query_none_can_include_viewport_when_present(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 0, 0, 0x22, "VIEWPORT", "Entity"),
            (200, 0, 0, 0x33, "LAYER", "Object"),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_viewport_entities", lambda _path: [(100,)])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_viewport_only.dwg", version="AC1021")
    entities = list(doc.modelspace().query())

    assert [entity.dxftype for entity in entities] == ["VIEWPORT"]
