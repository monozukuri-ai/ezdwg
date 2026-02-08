from __future__ import annotations

import math

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_3dface_maps_points_and_flags(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_3dface_entities",
        lambda _path: [
            (
                0xC01,
                (0.0, 0.0, 1.0),
                (2.0, 0.0, 1.0),
                (2.0, 2.0, 1.0),
                (0.0, 2.0, 1.0),
                5,
            )
        ],
    )

    doc = document_module.Document(path="dummy_3dface.dwg", version="AC1018")
    entities = list(doc.modelspace().query("3DFACE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "3DFACE"
    assert entity.handle == 0xC01
    assert entity.dxf["points"][2] == (2.0, 2.0, 1.0)
    assert entity.dxf["invisible_edge_flags"] == 5


def test_query_solid_maps_points_and_extrusion(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_solid_entities",
        lambda _path: [
            (
                0xC02,
                (0.0, 0.0, 0.5),
                (1.0, 0.0, 0.5),
                (1.0, 1.0, 0.5),
                (0.0, 1.0, 0.5),
                0.2,
                (0.0, 0.0, 1.0),
            )
        ],
    )

    doc = document_module.Document(path="dummy_solid.dwg", version="AC1018")
    entities = list(doc.modelspace().query("SOLID"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "SOLID"
    assert entity.handle == 0xC02
    assert entity.dxf["thickness"] == 0.2
    assert entity.dxf["extrusion"] == (0.0, 0.0, 1.0)


def test_query_trace_maps_points_and_extrusion(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_trace_entities",
        lambda _path: [
            (
                0xC03,
                (3.0, 3.0, 1.0),
                (4.0, 3.0, 1.0),
                (4.0, 4.0, 1.0),
                (3.0, 4.0, 1.0),
                0.0,
                (0.0, 0.0, 1.0),
            )
        ],
    )

    doc = document_module.Document(path="dummy_trace.dwg", version="AC1018")
    entities = list(doc.modelspace().query("TRACE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "TRACE"
    assert entity.handle == 0xC03
    assert entity.dxf["points"][0] == (3.0, 3.0, 1.0)


def test_query_shape_maps_fields(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_shape_entities",
        lambda _path: [
            (
                0xC04,
                (10.0, 20.0, 0.0),
                1.5,
                math.pi / 2.0,
                0.8,
                math.pi / 6.0,
                0.1,
                42,
                (0.0, 0.0, 1.0),
                0x1234,
            )
        ],
    )

    doc = document_module.Document(path="dummy_shape.dwg", version="AC1018")
    entities = list(doc.modelspace().query("SHAPE"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "SHAPE"
    assert entity.handle == 0xC04
    assert entity.dxf["insert"] == (10.0, 20.0, 0.0)
    assert abs(entity.dxf["rotation"] - 90.0) < 1e-9
    assert abs(entity.dxf["oblique"] - 30.0) < 1e-9
    assert entity.dxf["shape_no"] == 42
    assert entity.dxf["shapefile_handle"] == 0x1234
