from __future__ import annotations

import ezdwg.document as document_module
import ezdwg.raw as raw_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_leader_maps_points(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_leader_entities",
        lambda _path: [
            (0x900, 0, 1, [(0.0, 0.0, 0.0), (10.0, 2.0, 0.0), (12.0, 3.0, 0.0)])
        ],
    )

    doc = document_module.Document(path="dummy_leader.dwg", version="AC1018")
    entities = list(doc.modelspace().query("LEADER"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "LEADER"
    assert entity.handle == 0x900
    assert entity.dxf["annotation_type"] == 0
    assert entity.dxf["path_type"] == 1
    assert entity.dxf["points"][-1] == (12.0, 3.0, 0.0)


def test_query_hatch_maps_paths(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_hatch_entities",
        lambda _path: [
            (
                0x901,
                "ANSI31",
                False,
                True,
                2.5,
                (0.0, 0.0, 1.0),
                [
                    (True, [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]),
                    (False, [(1.0, 1.0), (2.0, 1.0)]),
                ],
            )
        ],
    )

    doc = document_module.Document(path="dummy_hatch.dwg", version="AC1018")
    entities = list(doc.modelspace().query("HATCH"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "HATCH"
    assert entity.dxf["pattern_name"] == "ANSI31"
    assert entity.dxf["solid_fill"] is False
    assert entity.dxf["associative"] is True
    assert entity.dxf["extrusion"] == (0.0, 0.0, 1.0)
    first_path = entity.dxf["paths"][0]
    assert first_path["closed"] is True
    assert first_path["points"][0] == first_path["points"][-1]
    assert first_path["points"][0][2] == 2.5
