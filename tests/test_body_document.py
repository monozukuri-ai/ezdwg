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


def test_query_body_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(310, 0, 0, 0x27, "BODY", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_body_entities", lambda _path: [(310,)])
    monkeypatch.setattr(
        document_module.raw,
        "decode_entity_styles",
        lambda _path: [(310, 256, None, 7)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(7, 5, None)],
    )

    doc = document_module.Document(path="dummy_body.dwg", version="AC1021")
    entities = list(doc.modelspace().query("BODY"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "BODY"
    assert entity.handle == 310
    assert entity.dxf["acis_handles"] == []
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5


def test_ac1032_body_decode_smoke_zero_or_more() -> None:
    sample = Path(__file__).resolve().parents[1] / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert sample.exists(), f"missing sample: {sample}"

    rows = raw.decode_body_entities(str(sample), limit=256)
    assert all(row[0] > 0 for row in rows)
    known_handles = {handle for handle, _ in raw.list_object_map_entries(str(sample))}
    layer_handles = {handle for handle, _aci, _true in raw.decode_layer_colors(str(sample))}
    for _entity_handle, acis_handles in rows:
        assert all(handle in known_handles for handle in acis_handles)
        assert all(handle not in layer_handles for handle in acis_handles)

    doc = ezdwg.read(str(sample))
    assert sum(1 for _ in doc.modelspace().query("BODY")) == len(rows)
