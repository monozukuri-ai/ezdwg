from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_polyline_3d_maps_vertices(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_3d_with_vertices",
        lambda _path: [
            (
                0xA01,
                0x09,
                True,
                [
                    (0.0, 0.0, 1.0),
                    (1.0, 0.0, 2.0),
                    (1.0, 1.0, 3.0),
                ],
            )
        ],
    )

    doc = document_module.Document(path="dummy_polyline3d.dwg", version="AC1018")
    entities = list(doc.modelspace().query("POLYLINE_3D"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "POLYLINE_3D"
    assert entity.handle == 0xA01
    assert entity.dxf["flags"] == 0x09
    assert entity.dxf["closed"] is True
    assert entity.dxf["points"][0] == (0.0, 0.0, 1.0)

