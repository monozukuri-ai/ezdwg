from __future__ import annotations

from pathlib import Path

import ezdwg
import ezdwg.document as document_module
from ezdwg import raw


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._object_headers_with_type_map.cache_clear()
    document_module._acis_candidate_handles_map.cache_clear()
    document_module._acis_candidate_record_map.cache_clear()


def test_query_region_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(200, 0, 0, 0x25, "REGION", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_region_entities", lambda _path: [(200,)])
    monkeypatch.setattr(
        document_module.raw,
        "decode_entity_styles",
        lambda _path: [(200, 256, None, 7)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(7, 5, None)],
    )

    doc = document_module.Document(path="dummy_region.dwg", version="AC1021")
    entities = list(doc.modelspace().query("REGION"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "REGION"
    assert entity.handle == 200
    assert entity.dxf["acis_handles"] == []
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5


def test_query_region_exposes_acis_handles(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(201, 0, 0, 0x25, "REGION", "Entity")],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_region_entities",
        lambda _path: [(201, [9001, 9002])],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_region_handles.dwg", version="AC1021")
    entity = next(doc.modelspace().query("REGION"))
    assert entity.dxf["acis_handles"] == [9001, 9002]


def test_ac1032_region_decode_smoke() -> None:
    sample = Path(__file__).resolve().parents[1] / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert sample.exists(), f"missing sample: {sample}"

    rows = raw.decode_region_entities(str(sample), limit=256)
    assert len(rows) >= 1
    assert all(row[0] > 0 for row in rows)
    known_handles = {handle for handle, _ in raw.list_object_map_entries(str(sample))}
    layer_handles = {handle for handle, _aci, _true in raw.decode_layer_colors(str(sample))}
    for _entity_handle, acis_handles in rows:
        assert all(handle in known_handles for handle in acis_handles)
        assert all(handle not in layer_handles for handle in acis_handles)

    doc = ezdwg.read(str(sample))
    assert sum(1 for _ in doc.modelspace().query("REGION")) == len(rows)
