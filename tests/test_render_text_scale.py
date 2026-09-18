from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import pytest

import ezdwg
import ezdwg.document as document_module
import ezdwg.render as render_module


ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture
def pyplot():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    yield plt
    plt.close("all")


@pytest.mark.parametrize("dpi,span", [(72, 100), (150, 100), (150, 200), (300, 100)])
def test_text_pixel_height_tracks_drawing_scale_and_dpi(pyplot, dpi, span) -> None:
    import numpy as np

    figure = pyplot.figure(figsize=(4, 4), dpi=dpi)
    ax = figure.add_axes((0, 0, 1, 1))
    ax.set(xlim=(0, span), ylim=(0, span))
    ax.set_axis_off()
    render_module._draw_text(ax, (10, 10), "H", 25.2, 0)
    figure.canvas.draw()

    pixels = np.asarray(figure.canvas.buffer_rgba())
    ys, _ = np.where(pixels[:, :, :3].min(axis=2) < 128)
    assert len(ys) > 0
    # Actual raster ink, not just a passed fontsize: a 25.2-unit capital must
    # occupy 25.2/span of the drawing viewport, with antialiasing tolerance.
    assert ys.max() - ys.min() + 1 == pytest.approx(4 * dpi * 25.2 / span, abs=2)


def test_text_scales_after_zoom_and_figure_resize(pyplot) -> None:
    figure, ax = pyplot.subplots()
    ax.set(xlim=(0, 100), ylim=(0, 100))
    render_module._draw_text(ax, (10, 10), "H", 0.25, 0)
    figure.canvas.draw()
    patch = ax.patches[-1]
    before = patch.get_window_extent().height
    ax.set_ylim(0, 50)
    figure.canvas.draw()
    assert patch.get_window_extent().height == pytest.approx(before * 2)
    figure.set_size_inches(12.8, 9.6)
    figure.canvas.draw()
    assert patch.get_window_extent().height == pytest.approx(before * 4)


def test_dimension_text_is_centered_on_saved_midpoint(pyplot) -> None:
    _, ax = pyplot.subplots()
    render_module._draw_dimension(ax, {
        "dimtype": "LINEAR", "defpoint": (0, 40, 0),
        "defpoint2": (0, 0, 0), "defpoint3": (4610, 0, 0),
        "text_midpoint": (2305, 60, 0), "text": "4610", "char_height": 25.2,
        "attachment_point": 5,
    }, 0.5)
    bounds = ax.patches[-1].get_path().get_extents()
    assert (bounds.x0 + bounds.x1) / 2 == pytest.approx(2305)
    assert (bounds.y0 + bounds.y1) / 2 == pytest.approx(60)


def test_dimension_ticks_and_extension_do_not_grow_with_length(pyplot) -> None:
    tick_lengths = []
    for length in (100, 4610):
        _, ax = pyplot.subplots()
        render_module._draw_dimension(ax, {
            "dimtype": "LINEAR", "defpoint": (0, 40, 0),
            "defpoint2": (0, 0, 0), "defpoint3": (length, 0, 0),
            "char_height": 25.2,
        }, 0.3)
        extension = ax.lines[0]
        assert max(extension.get_ydata()) - 40 == pytest.approx(12.6)
        tick = ax.lines[3]
        xs, ys = tick.get_data()
        tick_lengths.append(((xs[1] - xs[0]) ** 2 + (ys[1] - ys[0]) ** 2) ** 0.5)
        assert all(line.get_linewidth() <= 0.3 for line in ax.lines)
    assert tick_lengths == pytest.approx([20.16, 20.16])


def test_reference_markers_are_distinct_from_real_circles(pyplot) -> None:
    _, ax = pyplot.subplots()
    render_module._draw_reference_point(ax, (10, 20), color="black")
    render_module._draw_circle(ax, (50, 20), 4, 64, 0.5, color="black")
    assert ax.lines[0].get_marker() == "+"
    assert ax.lines[0].get_markersize() <= 2
    assert len(ax.lines[1].get_xdata()) > 2


def test_auto_fit_preserves_rectangular_drawing_extents(pyplot) -> None:
    _, ax = pyplot.subplots()
    ax.plot([0, 1000], [0, 100])
    render_module._apply_auto_limits(ax, equal=True, margin=0.04)
    assert ax.get_xlim() == pytest.approx((-40, 1040))
    assert ax.get_ylim() == pytest.approx((-4, 104))
    assert ax.get_aspect() == 1.0


def test_multiline_rotated_text_preserves_background_and_fits(pyplot) -> None:
    from matplotlib.colors import to_rgba

    figure, ax = pyplot.subplots()
    render_module._draw_text(
        ax, (50, 50), "HH\\PHH", 10, 90, color="red",
        background={"facecolor": "yellow", "edgecolor": "none", "alpha": 0.5},
    )
    render_module._apply_auto_limits(ax, equal=True, margin=0.04)
    figure.canvas.draw()
    assert len(ax.patches) == 2
    assert ax.patches[0].get_facecolor() == to_rgba("yellow", 0.5)
    assert ax.patches[1].get_facecolor() == to_rgba("red")
    box = ax.patches[0].get_window_extent()
    text = ax.patches[1].get_window_extent()
    assert box.contains(text.x0, text.y0) and box.contains(text.x1, text.y1)
    assert ax.bbox.contains(box.x0, box.y0) and ax.bbox.contains(box.x1, box.y1)


@pytest.mark.parametrize("stem", ["text_2000", "mtext_2004"])
def test_text_fixture_fits_saved_canvas(pyplot, tmp_path, stem) -> None:
    from PIL import Image

    doc = ezdwg.read(str(ROOT / "test_dwg" / f"{stem}.dwg"))
    ax = doc.plot(types="TEXT MTEXT", show=False)
    ax.figure.canvas.draw()
    assert ax.patches
    for patch in ax.patches:
        bounds = patch.get_window_extent()
        assert ax.bbox.contains(bounds.x0, bounds.y0)
        assert ax.bbox.contains(bounds.x1, bounds.y1)
    output = tmp_path / "drawing.png"
    ax.figure.savefig(output, dpi=150, bbox_inches="tight")
    with Image.open(output) as image:
        assert image.width < 1200 and image.height < 1200


@pytest.mark.parametrize("version", ["AC1027", "AC1032"])
def test_dimension_height_matches_saved_block_text(version) -> None:
    source = ROOT / "test_dwg" / "acadsharp" / f"sample_{version}.dwg"
    doc = ezdwg.read(str(source))
    dimension = next(e for e in doc.modelspace().query("DIMENSION") if e.handle == 0x514)
    owner = dimension.dxf["anonymous_block_handle"]
    text = next(
        row for row in ezdwg.raw.decode_mtext_entities(str(source)) if row[10] == owner
    )
    assert text[6] == pytest.approx(2.5)
    assert dimension.dxf["char_height"] == pytest.approx(text[6])
    assert dimension.dxf["char_height_source"] == "anonymous_block"


def test_block_height_rejects_ambiguous_or_invalid_sizes(monkeypatch) -> None:
    def text_row(owner, height):
        return (1, "H", (), (), (), (0, 0, height, 0, 1), (), None, owner)

    def mtext_row(owner, height):
        return (2, "H", (), (), (), 0, height, 0, 0, (), owner)

    document_module._block_text_height_map.cache_clear()
    monkeypatch.setattr(document_module.raw, "decode_text_entities", lambda path: [
        text_row(10, 25.2), text_row(20, 25.2), text_row(30, 25.2), text_row(40, 0),
    ])
    monkeypatch.setattr(document_module.raw, "decode_mtext_entities", lambda path: [
        mtext_row(10, 25.19999999999999), mtext_row(20, 36), mtext_row(30, float("nan")),
    ])
    assert document_module._block_text_height_map("height_test.dwg") == pytest.approx(
        {10: 25.2}
    )
    ezdwg.clear_decode_caches()
    assert document_module._block_text_height_map.cache_info().currsize == 0


@pytest.mark.parametrize("dimtype", ["LINEAR", "DIAMETER", "RADIUS"])
def test_dimension_text_height_does_not_depend_on_length(monkeypatch, dimtype) -> None:
    heights = []
    monkeypatch.setattr(
        render_module, "_draw_text",
        lambda ax, pos, text, height, rotation, **kw: heights.append(height),
    )
    ax = SimpleNamespace(plot=lambda *args, **kwargs: None)
    for length in (100, 4610):
        dxf = {
            "dimtype": dimtype, "defpoint": (0, 40, 0),
            "defpoint2": (0, 0, 0), "defpoint3": (length, 0, 0),
            "text_midpoint": (length / 2, 40, 0), "text": "<>",
            "actual_measurement": length, "char_height": 25.2,
        }
        render_module._draw_dimension(ax, dxf, 1)
    assert heights == [25.2, 25.2]
    del dxf["char_height"]
    render_module._draw_dimension(ax, dxf, 1)
    assert heights[-1] == 1.0  # Documented fallback, independent of length.
