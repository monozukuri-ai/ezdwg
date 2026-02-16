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


def test_query_3dsolid_entity(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(300, 0, 0, 0x26, "3DSOLID", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_3dsolid_entities", lambda _path: [(300,)])
    monkeypatch.setattr(
        document_module.raw,
        "decode_entity_styles",
        lambda _path: [(300, 256, None, 7)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(7, 5, None)],
    )

    doc = document_module.Document(path="dummy_3dsolid.dwg", version="AC1021")
    entities = list(doc.modelspace().query("3DSOLID"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "3DSOLID"
    assert entity.handle == 300
    assert entity.dxf["acis_handles"] == []
    assert entity.dxf["layer_handle"] == 7
    assert entity.dxf["resolved_color_index"] == 5


def test_ac1032_3dsolid_decode_smoke() -> None:
    sample = Path(__file__).resolve().parents[1] / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert sample.exists(), f"missing sample: {sample}"

    rows = raw.decode_3dsolid_entities(str(sample), limit=256)
    assert len(rows) >= 1
    assert all(row[0] > 0 for row in rows)
    known_handles = {handle for handle, _ in raw.list_object_map_entries(str(sample))}
    layer_handles = {handle for handle, _aci, _true in raw.decode_layer_colors(str(sample))}
    for _entity_handle, acis_handles in rows:
        assert all(handle in known_handles for handle in acis_handles)
        assert all(handle not in layer_handles for handle in acis_handles)

    doc = ezdwg.read(str(sample))
    entities = list(doc.modelspace().query("3DSOLID"))
    assert len(entities) == len(rows)
    for entity in entities:
        assert "acis_candidate_edges" in entity.dxf
        assert "acis_primary_edges" in entity.dxf
        for record in entity.dxf.get("acis_candidate_records", []):
            assert "acis_parent_kind" in record
            assert "candidate_ref_handles" in record


def test_query_3dsolid_acis_candidate_handles_prefers_handle_order(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x26, "3DSOLID", "Entity"),
            (101, 1000, 0, 0x221, "UNKNOWN(0x221)", ""),
            (102, 1100, 0, 0x1F9, "UNKNOWN(0x1F9)", ""),
            (103, 1200, 0, 0x222, "UNKNOWN(0x222)", ""),
            (104, 20, 0, 0x13, "LINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_3dsolid_entities",
        lambda _path: [(100, [])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (101, 1000, 17, 0x221, b"\x00ACIS-HEADER\x00"),
            (103, 1200, 22, 0x222, b"\x01\x02\x03\x04"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_acis_candidate_infos",
        lambda _path, _handles, limit=None: [
            (101, 0x221, 17, "acis-text-header", [100, 103], 88),
            (103, 0x222, 22, "acis-payload-chunk", [100, 101, 104], 82),
        ],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_3dsolid_acis.dwg", version="AC1021")
    entity = next(doc.modelspace().query("3DSOLID"))
    assert entity.dxf["acis_candidate_handles"] == [101, 103]
    records = entity.dxf["acis_candidate_records"]
    edges = entity.dxf["acis_candidate_edges"]
    primary_edges = entity.dxf["acis_primary_edges"]
    assert [record["handle"] for record in records] == [101, 103]
    assert records[0]["type_code"] == 0x221
    assert records[0]["type_name"] == "UNKNOWN(0X221)"
    assert records[0]["record_size"] == 13
    assert records[0]["ascii_preview"] == "ACIS-HEADER"
    assert records[0]["likely_handle_refs"] == [100, 103]
    assert records[0]["entity_ref_handles"] == [100]
    assert records[0]["candidate_ref_handles"] == [103]
    assert records[0]["acis_parent_handle"] == 100
    assert records[0]["acis_parent_kind"] == "entity"
    assert records[0]["acis_parent_rule"] == "header-prefers-entity"
    assert records[0]["acis_ref_confidence"] == 88
    assert records[0]["acis_parent_ref_strategy"] == "stream"
    assert records[0]["acis_child_candidate_handles"] == [103]
    assert records[0]["likely_handle_ref_details"] == [
        {
            "handle": 100,
            "offset": 10,
            "data_size": 0,
            "type_code": 0x26,
            "type_name": "3DSOLID",
            "type_class": "ENTITY",
        },
        {
            "handle": 103,
            "offset": 1200,
            "data_size": 0,
            "type_code": 0x222,
            "type_name": "UNKNOWN(0X222)",
            "type_class": "",
        },
    ]
    assert records[0]["acis_role_hint"] == "acis-text-header"
    assert records[1]["ascii_preview"] is None
    assert records[1]["likely_handle_refs"] == [100, 101, 104]
    assert records[1]["entity_ref_handles"] == [100]
    assert records[1]["candidate_ref_handles"] == [101]
    assert records[1]["external_ref_handles"] == [104]
    assert records[1]["acis_parent_handle"] == 101
    assert records[1]["acis_parent_kind"] == "candidate"
    assert records[1]["acis_parent_rule"] == "payload-fallback-header"
    assert records[1]["acis_ref_confidence"] == 82
    assert records[1]["acis_parent_ref_strategy"] == "stream"
    assert records[1]["acis_child_candidate_handles"] == []
    assert records[1]["likely_handle_ref_details"] == [
        {
            "handle": 100,
            "offset": 10,
            "data_size": 0,
            "type_code": 0x26,
            "type_name": "3DSOLID",
            "type_class": "ENTITY",
        },
        {
            "handle": 101,
            "offset": 1000,
            "data_size": 0,
            "type_code": 0x221,
            "type_name": "UNKNOWN(0X221)",
            "type_class": "",
        },
        {
            "handle": 104,
            "offset": 20,
            "data_size": 0,
            "type_code": 0x13,
            "type_name": "LINE",
            "type_class": "ENTITY",
        }
    ]
    assert records[1]["acis_role_hint"] == "acis-payload-chunk"
    assert edges == [
        {"source": 101, "target": 100, "kind": "entity"},
        {"source": 101, "target": 103, "kind": "candidate"},
        {"source": 103, "target": 100, "kind": "entity"},
        {"source": 103, "target": 101, "kind": "candidate"},
        {"source": 103, "target": 104, "kind": "external"},
    ]
    assert primary_edges == [
        {
            "source": 101,
            "target": 100,
            "kind": "entity",
            "rule": "header-prefers-entity",
        },
        {
            "source": 103,
            "target": 101,
            "kind": "candidate",
            "rule": "payload-fallback-header",
        },
    ]


def test_query_3dsolid_acis_low_confidence_merges_byte_scan_refs(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x26, "3DSOLID", "Entity"),
            (103, 1200, 0, 0x222, "UNKNOWN(0x222)", ""),
            (104, 20, 0, 0x13, "LINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_3dsolid_entities",
        lambda _path: [(100, [])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (103, 1200, 22, 0x222, b"\x01\x02\x03\x04"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_acis_candidate_infos",
        lambda _path, _handles, limit=None: [
            (103, 0x222, 22, "acis-payload-chunk", [100], 12),
        ],
    )
    monkeypatch.setattr(
        document_module,
        "_extract_likely_handle_refs",
        lambda _raw_bytes, _known_handles, **_kwargs: [104, 100],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_3dsolid_low_conf.dwg", version="AC1021")
    entity = next(doc.modelspace().query("3DSOLID"))
    record = entity.dxf["acis_candidate_records"][0]

    assert record["handle"] == 103
    assert record["acis_ref_confidence"] == 12
    assert record["likely_handle_refs"] == [100, 104]
    assert record["acis_stream_handle_refs"] == [100]
    assert record["acis_scanned_handle_refs"] == [104, 100]
    assert record["acis_parent_ref_handles"] == [100]
    assert record["acis_parent_ref_strategy"] == "stream-scan-overlap"
    assert record["entity_ref_handles"] == [100]
    assert record["external_ref_handles"] == []


def test_query_3dsolid_acis_low_confidence_drops_uncorroborated_refs(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x26, "3DSOLID", "Entity"),
            (103, 1200, 0, 0x222, "UNKNOWN(0x222)", ""),
            (104, 20, 0, 0x13, "LINE", "Entity"),
            (105, 21, 0, 0x13, "LINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_3dsolid_entities",
        lambda _path: [(100, [])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (103, 1200, 22, 0x222, b"\x01\x02\x03\x04"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_acis_candidate_infos",
        lambda _path, _handles, limit=None: [
            (103, 0x222, 22, "acis-payload-chunk", [104], 9),
        ],
    )
    monkeypatch.setattr(
        document_module,
        "_extract_likely_handle_refs",
        lambda _raw_bytes, _known_handles, **_kwargs: [105],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_3dsolid_low_conf_drop.dwg", version="AC1021")
    entity = next(doc.modelspace().query("3DSOLID"))
    record = entity.dxf["acis_candidate_records"][0]

    assert record["handle"] == 103
    assert record["acis_ref_confidence"] == 9
    assert record["likely_handle_refs"] == [104, 105]
    assert record["acis_parent_ref_handles"] == []
    assert record["acis_parent_ref_strategy"] == "lowconf-drop"
    assert record["acis_parent_ref_strategy_effective"] == "lowconf-drop"
    assert record["entity_ref_handles"] == []
    assert record["candidate_ref_handles"] == []
    assert record["external_ref_handles"] == []
    assert record["acis_parent_kind"] == "none"
    assert entity.dxf["acis_primary_edges"] == []


def test_query_3dsolid_acis_low_confidence_entity_fallback(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x26, "3DSOLID", "Entity"),
            (103, 1200, 0, 0x222, "UNKNOWN(0x222)", ""),
            (104, 20, 0, 0x13, "LINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_3dsolid_entities",
        lambda _path: [(100, [])],
    )
    monkeypatch.setattr(
        document_module.raw,
        "read_object_records_by_handle",
        lambda _path, _handles, limit=None: [
            (103, 1200, 22, 0x222, b"\x01\x02\x03\x04"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_acis_candidate_infos",
        lambda _path, _handles, limit=None: [
            (103, 0x222, 22, "acis-payload-chunk", [104], 9),
        ],
    )
    monkeypatch.setattr(
        document_module,
        "_extract_likely_handle_refs",
        lambda _raw_bytes, _known_handles, **_kwargs: [100],
    )
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])

    doc = document_module.Document(path="dummy_3dsolid_low_conf_entity_fallback.dwg", version="AC1021")
    entity = next(doc.modelspace().query("3DSOLID"))
    record = entity.dxf["acis_candidate_records"][0]

    assert record["handle"] == 103
    assert record["acis_ref_confidence"] == 9
    assert record["likely_handle_refs"] == [104, 100]
    assert record["acis_parent_ref_handles"] == []
    assert record["acis_parent_ref_strategy"] == "lowconf-drop"
    assert record["acis_parent_ref_strategy_effective"] == "lowconf-entity-fallback"
    assert record["entity_ref_handles"] == [100]
    assert record["candidate_ref_handles"] == []
    assert record["external_ref_handles"] == []
    assert record["acis_parent_kind"] == "entity"
    assert record["acis_parent_rule"] == "payload-fallback-entity"
    assert entity.dxf["acis_primary_edges"] == [
        {
            "source": 103,
            "target": 100,
            "kind": "entity",
            "rule": "payload-fallback-entity",
        }
    ]
