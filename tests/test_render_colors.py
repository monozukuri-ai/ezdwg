from __future__ import annotations

from types import SimpleNamespace

import ezdwg.render as render_module


class _FakeLines:
    def get_next_color(self):
        return "C0"


class _FakeAx:
    def __init__(self) -> None:
        self._get_lines = _FakeLines()

    def autoscale(self, _enabled: bool) -> None:
        return None

    def set_title(self, _title: str) -> None:
        return None

    def set_aspect(self, _aspect: str, adjustable: str = "box") -> None:
        return None


class _FakeMTextAx(_FakeAx):
    def get_facecolor(self):
        return (0.9, 0.9, 0.9, 1.0)


class _FakeLayout:
    def __init__(self, entities):
        self._entities = list(entities)

    def query(self, _types=None):
        return iter(self._entities)


def test_resolve_dwg_color_prefers_true_color_over_aci() -> None:
    color = render_module._resolve_dwg_color(
        {
            "resolved_color_index": 1,
            "resolved_true_color": 0x00123456,
        }
    )
    assert color == "#123456"


def test_aci_7_resolves_to_black() -> None:
    color = render_module._resolve_dwg_color({"resolved_color_index": 7})
    assert color == "#000000"


def test_plot_layout_falls_back_to_black_for_unresolved_color(monkeypatch) -> None:
    captured: list[str | None] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_line",
        lambda _ax, _start, _end, _line_width, color=None: captured.append(color),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="LINE",
                dxf={
                    "start": (0.0, 0.0, 0.0),
                    "end": (1.0, 0.0, 0.0),
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == ["#000000"]


def test_plot_layout_dimension_uses_black_by_default(monkeypatch) -> None:
    captured: list[str | None] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_dimension",
        lambda _ax, _dxf, _line_width, color=None: captured.append(color),
    )

    layout = _FakeLayout(
        [SimpleNamespace(dxftype="DIMENSION", dxf={"resolved_color_index": 1})]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == ["black"]


def test_plot_layout_dimension_uses_entity_color_when_requested(monkeypatch) -> None:
    captured: list[str | None] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_dimension",
        lambda _ax, _dxf, _line_width, color=None: captured.append(color),
    )

    layout = _FakeLayout(
        [SimpleNamespace(dxftype="DIMENSION", dxf={"resolved_color_index": 1})]
    )
    ax = _FakeAx()

    render_module.plot_layout(
        layout,
        ax=ax,
        show=False,
        auto_fit=False,
        equal=False,
        dimension_color=None,
    )

    assert captured == ["#ff0000"]


def test_build_lwpolyline_path_expands_bulge_arc() -> None:
    path = render_module._build_lwpolyline_path(
        [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)],
        bulges=[1.0, 0.0],
        closed=False,
        arc_segments=32,
    )
    assert len(path) > 2
    assert path[0] == (0.0, 0.0)
    assert path[-1] == (1.0, 0.0)
    assert max(abs(y) for _x, y in path) > 0.45


def test_resolve_mtext_background_bbox_uses_true_color() -> None:
    bbox = render_module._resolve_mtext_background_bbox(
        _FakeMTextAx(),
        {
            "background_flags": 1,
            "background_true_color": 0x0000FF00,
            "background_transparency": 128,
        },
    )
    assert bbox is not None
    assert bbox["facecolor"] == "#00ff00"
    assert 0.45 <= bbox["alpha"] <= 0.55


def test_plot_layout_mtext_passes_background(monkeypatch) -> None:
    captured: list[dict | None] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_text",
        lambda _ax, _insert, _text, _height, _rotation, color=None, background=None: captured.append(
            background
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="MTEXT",
                dxf={
                    "insert": (1.0, 2.0, 0.0),
                    "text": "abc",
                    "char_height": 1.0,
                    "rotation": 0.0,
                    "background_flags": 1,
                    "background_true_color": 0x00FF0000,
                },
            )
        ]
    )
    ax = _FakeMTextAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert len(captured) == 1
    assert captured[0] is not None
    assert captured[0]["facecolor"] == "#ff0000"


def test_plot_layout_attrib_uses_text_drawer(monkeypatch) -> None:
    captured: list[str] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_text",
        lambda _ax, _insert, text, _height, _rotation, color=None, background=None: captured.append(
            text
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="ATTRIB",
                dxf={"insert": (1.0, 2.0, 0.0), "text": "TAGVAL", "height": 2.0, "rotation": 0.0},
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == ["TAGVAL"]


def test_plot_layout_minsert_uses_point_drawer(monkeypatch) -> None:
    captured: list[tuple[float, float, float]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_point",
        lambda _ax, location, _line_width, color=None: captured.append(location),
    )

    layout = _FakeLayout([SimpleNamespace(dxftype="MINSERT", dxf={"insert": (3.0, 4.0, 0.0)})])
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [(3.0, 4.0, 0.0)]


def test_plot_layout_spline_uses_polyline_drawer(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="SPLINE",
                dxf={"points": [(0.0, 0.0, 0.0), (1.0, 1.0, 0.0)], "closed": True},
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [([(0.0, 0.0, 0.0), (1.0, 1.0, 0.0)], True)]


def test_plot_layout_leader_uses_polyline_drawer(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="LEADER",
                dxf={"points": [(0.0, 0.0, 0.0), (3.0, 2.0, 0.0)]},
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [([(0.0, 0.0, 0.0), (3.0, 2.0, 0.0)], False)]


def test_plot_layout_hatch_uses_polyline_drawer_for_each_path(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="HATCH",
                dxf={
                    "paths": [
                        {"points": [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], "closed": False},
                        {"points": [(2.0, 2.0, 0.0), (3.0, 2.0, 0.0)], "closed": True},
                    ]
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [
        ([(0.0, 0.0, 0.0), (1.0, 0.0, 0.0)], False),
        ([(2.0, 2.0, 0.0), (3.0, 2.0, 0.0)], True),
    ]


def test_plot_layout_polyline_3d_uses_polyline_drawer(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="POLYLINE_3D",
                dxf={
                    "points": [(0.0, 0.0, 1.0), (1.0, 0.0, 2.0), (1.0, 1.0, 3.0)],
                    "closed": True,
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [([(0.0, 0.0, 1.0), (1.0, 0.0, 2.0), (1.0, 1.0, 3.0)], True)]


def test_plot_layout_polyline_mesh_uses_polyline_drawer(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="POLYLINE_MESH",
                dxf={
                    "points": [
                        (0.0, 0.0, 0.0),
                        (1.0, 0.0, 0.0),
                        (2.0, 0.0, 0.0),
                        (0.0, 1.0, 0.0),
                        (1.0, 1.0, 0.0),
                        (2.0, 1.0, 0.0),
                    ],
                    "m_vertex_count": 3,
                    "n_vertex_count": 2,
                    "closed": False,
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    # 2 horizontal + 3 vertical grid lines
    assert len(captured) == 5


def test_plot_layout_polyline_pface_uses_face_edges(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="POLYLINE_PFACE",
                dxf={
                    "vertices": [
                        (0.0, 0.0, 0.0),
                        (1.0, 0.0, 0.0),
                        (1.0, 1.0, 0.0),
                        (0.0, 1.0, 0.0),
                    ],
                    "faces": [(1, 2, 3, 4)],
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [
        ([(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)], True)
    ]


def test_plot_layout_3dface_uses_face_drawer(monkeypatch) -> None:
    captured: list[tuple[list, int]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_3dface",
        lambda _ax, points, invisible_edge_flags, _line_width, color=None: captured.append(
            (list(points), int(invisible_edge_flags))
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="3DFACE",
                dxf={
                    "points": [
                        (0.0, 0.0, 0.0),
                        (2.0, 0.0, 0.0),
                        (2.0, 2.0, 0.0),
                        (0.0, 2.0, 0.0),
                    ],
                    "invisible_edge_flags": 6,
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [
        (
            [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 2.0, 0.0), (0.0, 2.0, 0.0)],
            6,
        )
    ]


def test_plot_layout_solid_uses_closed_polyline(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), closed)
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="SOLID",
                dxf={
                    "points": [
                        (0.0, 0.0, 0.0),
                        (1.0, 0.0, 0.0),
                        (1.0, 1.0, 0.0),
                        (0.0, 1.0, 0.0),
                    ]
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [
        ([(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)], True)
    ]


def test_plot_layout_shape_uses_point_drawer(monkeypatch) -> None:
    captured: list[tuple[float, float, float]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_point",
        lambda _ax, location, _line_width, color=None: captured.append(location),
    )

    layout = _FakeLayout([SimpleNamespace(dxftype="SHAPE", dxf={"insert": (9.0, 8.0, 0.0)})])
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [(9.0, 8.0, 0.0)]


def test_plot_layout_tolerance_uses_text_drawer(monkeypatch) -> None:
    captured: list[tuple[str, float, float]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_text",
        lambda _ax, _insert, text, height, rotation, color=None, background=None: captured.append(
            (text, float(height), float(rotation))
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="TOLERANCE",
                dxf={
                    "insert": (1.0, 2.0, 0.0),
                    "text": "{\\Fgdt;jIS0.7x;1.0}",
                    "height": 2.0,
                    "rotation": 15.0,
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [("{\\Fgdt;jIS0.7x;1.0}", 2.0, 15.0)]


def test_plot_layout_mline_uses_polyline_drawer(monkeypatch) -> None:
    captured: list[tuple[list, bool]] = []
    monkeypatch.setattr(render_module, "_require_matplotlib", lambda: object())
    monkeypatch.setattr(
        render_module,
        "_draw_polyline",
        lambda _ax, points, _line_width, color=None, bulges=None, closed=False, arc_segments=64: captured.append(
            (list(points), bool(closed))
        ),
    )

    layout = _FakeLayout(
        [
            SimpleNamespace(
                dxftype="MLINE",
                dxf={
                    "points": [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 1.0, 0.0)],
                    "closed": True,
                },
            )
        ]
    )
    ax = _FakeAx()

    render_module.plot_layout(layout, ax=ax, show=False, auto_fit=False, equal=False)

    assert captured == [([(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 1.0, 0.0)], True)]
