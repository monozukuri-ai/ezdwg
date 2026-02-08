from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_tolerance_maps_fields(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_tolerance_entities",
        lambda _path: [
            (
                0xD01,
                "{\\Fgdt;jIS0.7x;12.34}",
                (10.0, 20.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, 0.0, 1.0),
                2.5,
                0.7,
                0x500,
            )
        ],
    )

    doc = document_module.Document(path="dummy_tol.dwg", version="AC1018")
    entities = list(doc.modelspace().query("TOLERANCE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "TOLERANCE"
    assert entity.handle == 0xD01
    assert entity.dxf["insert"] == (10.0, 20.0, 0.0)
    assert entity.dxf["height"] == 2.5
    assert entity.dxf["dimstyle_handle"] == 0x500
    assert abs(entity.dxf["rotation"] - 90.0) < 1e-9


def test_query_mline_maps_vertices(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_mline_entities",
        lambda _path: [
            (
                0xD02,
                1.0,
                1,
                (0.0, 0.0, 0.0),
                (0.0, 0.0, 1.0),
                3,
                2,
                [
                    ((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)),
                    ((2.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)),
                    ((2.0, 2.0, 0.0), (0.0, 1.0, 0.0), (-1.0, 0.0, 0.0)),
                ],
                0x501,
            )
        ],
    )

    doc = document_module.Document(path="dummy_mline.dwg", version="AC1018")
    entities = list(doc.modelspace().query("MLINE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "MLINE"
    assert entity.handle == 0xD02
    assert entity.dxf["scale"] == 1.0
    assert entity.dxf["line_count"] == 2
    assert entity.dxf["closed"] is True
    assert entity.dxf["points"] == [
        (0.0, 0.0, 0.0),
        (2.0, 0.0, 0.0),
        (2.0, 2.0, 0.0),
    ]
    assert entity.dxf["mlinestyle_handle"] == 0x501
