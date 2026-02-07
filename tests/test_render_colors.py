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
