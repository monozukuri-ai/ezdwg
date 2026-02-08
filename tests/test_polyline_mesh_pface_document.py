from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


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

    doc = document_module.Document(path="dummy_pface.dwg", version="AC1018")
    entities = list(doc.modelspace().query("POLYLINE_PFACE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "POLYLINE_PFACE"
    assert entity.handle == 0xB02
    assert entity.dxf["num_vertices"] == 4
    assert entity.dxf["num_faces"] == 1
    assert entity.dxf["faces"] == [(1, 2, 3, 4)]
