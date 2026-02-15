from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._polyline_sequence_relationships.cache_clear()


def test_query_polyline_mesh_maps_vertices(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_mesh_with_vertices",
        lambda _path: [
            (
                0xB01,
                0x01,
                3,
                2,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 0.0, 0.0),
                    (2.0, 0.0, 0.0),
                    (0.0, 1.0, 0.0),
                    (1.0, 1.0, 0.0),
                    (2.0, 1.0, 0.0),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(0xB01, "POLYLINE_MESH", [0xB11, 0xB12, 0xB13, 0xB14], [], 0xBFF)],
    )

    doc = document_module.Document(path="dummy_mesh.dwg", version="AC1018")
    entities = list(doc.modelspace().query("POLYLINE_MESH"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "POLYLINE_MESH"
    assert entity.handle == 0xB01
    assert entity.dxf["m_vertex_count"] == 3
    assert entity.dxf["n_vertex_count"] == 2
    assert entity.dxf["closed"] is True
    assert len(entity.dxf["points"]) == 6
    assert entity.dxf["vertex_handles"] == [0xB11, 0xB12, 0xB13, 0xB14]
    assert entity.dxf["seqend_handle"] == 0xBFF


def test_query_polyline_pface_maps_faces(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_pface_with_faces",
        lambda _path: [
            (
                0xB02,
                4,
                1,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 0.0, 0.0),
                    (1.0, 1.0, 0.0),
                    (0.0, 1.0, 0.0),
                ],
                [(1, 2, 3, 4)],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(0xB02, "POLYLINE_PFACE", [0xC11, 0xC12, 0xC13, 0xC14], [0xC21], 0xCFF)],
    )

    doc = document_module.Document(path="dummy_pface.dwg", version="AC1018")
    entities = list(doc.modelspace().query("POLYLINE_PFACE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "POLYLINE_PFACE"
    assert entity.handle == 0xB02
    assert entity.dxf["num_vertices"] == 4
    assert entity.dxf["num_faces"] == 1
    assert entity.dxf["faces"] == [(1, 2, 3, 4)]
    assert entity.dxf["vertex_handles"] == [0xC11, 0xC12, 0xC13, 0xC14]
    assert entity.dxf["face_handles"] == [0xC21]
    assert entity.dxf["seqend_handle"] == 0xCFF


def test_query_vertex_pface_entities_include_owner_handles(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_pface_entities",
        lambda _path: [(0xC11, 0x00, 1.0, 2.0, 3.0)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_vertex_pface_face_entities",
        lambda _path: [(0xC21, 1, 2, 3, 4)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_sequence_members",
        lambda _path: [(0xB02, "POLYLINE_PFACE", [0xC11], [0xC21], 0xCFF)],
    )

    doc = document_module.Document(path="dummy_pface_owner.dwg", version="AC1018")
    entities = list(doc.modelspace().query("VERTEX_PFACE VERTEX_PFACE_FACE"))
    assert len(entities) == 2

    vertex = next(entity for entity in entities if entity.dxftype == "VERTEX_PFACE")
    face = next(entity for entity in entities if entity.dxftype == "VERTEX_PFACE_FACE")

    assert vertex.dxf["owner_handle"] == 0xB02
    assert vertex.dxf["owner_type"] == "POLYLINE_PFACE"
    assert face.dxf["owner_handle"] == 0xB02
    assert face.dxf["owner_type"] == "POLYLINE_PFACE"
