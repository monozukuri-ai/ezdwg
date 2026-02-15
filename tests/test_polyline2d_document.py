from __future__ import annotations

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_polyline_2d_maps_vertex_data(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D01,
                0x0001,
                [
                    (0.0, 0.0, 0.0, 0.1, 0.2, 0.0, 0.0, 0),
                    (2.0, 0.0, 0.0, 0.2, 0.3, 0.5, 0.0, 0),
                    (2.0, 1.0, 0.0, 0.3, 0.4, 0.0, 0.0, 0),
                    (0.0, 0.0, 0.0, 0.1, 0.2, 0.0, 0.0, 0),
                ],
            )
        ],
    )

    doc = document_module.Document(path="dummy_polyline2d.dwg", version="AC1018")
    entities = list(doc.modelspace().query("POLYLINE_2D"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "POLYLINE_2D"
    assert entity.handle == 0x2D01
    assert entity.dxf["flags"] == 0x0001
    assert entity.dxf["closed"] is True
    # Closed polyline is represented by closed flag, not duplicated last point.
    assert entity.dxf["points"] == [
        (0.0, 0.0, 0.0),
        (2.0, 0.0, 0.0),
        (2.0, 1.0, 0.0),
    ]
    assert entity.dxf["bulges"] == [0.0, 0.5, 0.0]
    assert entity.dxf["widths"] == [(0.1, 0.2), (0.2, 0.3), (0.3, 0.4)]
    assert entity.dxf["vertex_flags"] == [0, 0, 0]


def test_query_polyline_2d_maps_interpreted_flags_and_interpolated_points(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertex_data",
        lambda _path: [
            (
                0x2D02,
                0x0001,
                [
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                ],
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_entities_interpreted",
        lambda _path: [
            (
                0x2D02,
                0x0001,
                5,
                "QuadraticBSpline",
                True,
                True,
                False,
                False,
                False,
                False,
                False,
                False,
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_polyline_2d_with_vertices_interpolated",
        lambda _path, _segments_per_span=8: [
            (
                0x2D02,
                0x0001,
                True,
                [
                    (0.0, 0.0, 0.0),
                    (1.0, 1.25, 0.0),
                    (2.0, 1.5, 0.0),
                    (3.0, 1.0, 0.0),
                    (4.0, 0.0, 0.0),
                    (0.0, 0.0, 0.0),
                ],
            )
        ],
    )

    doc = document_module.Document(path="dummy_polyline2d_interp.dwg", version="AC1018")
    entity = next(doc.modelspace().query("POLYLINE_2D"))

    assert entity.handle == 0x2D02
    assert entity.dxf["curve_type"] == 5
    assert entity.dxf["curve_type_label"] == "QuadraticBSpline"
    assert entity.dxf["curve_fit"] is True
    assert entity.dxf["spline_fit"] is False
    assert entity.dxf["should_interpolate"] is True
    assert entity.dxf["interpolation_applied"] is True
    assert entity.dxf["points"] == [
        (0.0, 0.0, 0.0),
        (2.0, 1.0, 0.0),
        (4.0, 0.0, 0.0),
    ]
    assert entity.dxf["interpolated_points"] == [
        (0.0, 0.0, 0.0),
        (1.0, 1.25, 0.0),
        (2.0, 1.5, 0.0),
        (3.0, 1.0, 0.0),
        (4.0, 0.0, 0.0),
    ]
