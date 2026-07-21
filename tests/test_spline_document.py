from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_spline_prefers_fit_points(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_spline_entities",
        lambda _path: [
            (
                0x501,
                (2, 3, False, False, False),
                (1.0e-7, None, None),
                [],
                [(0.0, 0.0, 0.0), (3.0, 3.0, 0.0)],
                [],
                [(1.0, 2.0, 0.0), (2.0, 1.0, 0.0)],
            )
        ],
    )

    doc = document_module.Document(path="dummy_spline_fit.dwg", version="AC1018")
    entities = list(doc.modelspace().query("SPLINE"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert entities[0].dxftype == "SPLINE"
    assert dxf["scenario"] == 2
    assert dxf["degree"] == 3
    assert dxf["points"] == [(1.0, 2.0, 0.0), (2.0, 1.0, 0.0)]
    assert entities[0].to_points() == [(1.0, 2.0, 0.0), (2.0, 1.0, 0.0)]


def test_query_spline_uses_control_points_when_fit_absent(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_spline_entities",
        lambda _path: [
            (
                0x502,
                (1, 3, True, True, False),
                (None, 1.0e-7, 1.0e-7),
                [0.0, 0.0, 1.0, 1.0],
                [(0.0, 0.0, 0.0), (1.0, 1.0, 0.0), (2.0, 0.0, 0.0)],
                [1.0, 1.0, 1.0],
                [],
            )
        ],
    )

    doc = document_module.Document(path="dummy_spline_ctrl.dwg", version="AC1018")
    entities = list(doc.modelspace().query("SPLINE"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert dxf["rational"] is True
    assert dxf["closed"] is True
    assert dxf["points"][0] == dxf["points"][-1]
    assert len(dxf["points"]) == 4
    assert entities[0].to_points() == [
        (0.0, 0.0, 0.0),
        (1.0, 1.0, 0.0),
        (2.0, 0.0, 0.0),
        (0.0, 0.0, 0.0),
    ]
