from __future__ import annotations

import sys
import tempfile
import webbrowser
from pathlib import Path

import pytest

import ezdwg.cli as cli_module


SAMPLE = Path(__file__).resolve().parents[1] / "examples" / "data" / "line_2000.dwg"


@pytest.fixture
def pyplot():
    matplotlib = pytest.importorskip("matplotlib")
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    yield plt
    plt.close("all")


@pytest.mark.parametrize(
    ("extension", "signature"),
    [("png", b"\x89PNG\r\n\x1a\n"), ("svg", b"<?xml"), ("pdf", b"%PDF-")],
)
def test_plot_saves_without_showing(
    extension, signature, pyplot, monkeypatch, tmp_path, capsys
) -> None:
    def unexpected_show():
        pytest.fail("file output must not open a window")

    monkeypatch.setattr(pyplot, "show", unexpected_show)
    monkeypatch.setattr(webbrowser, "open", lambda _uri: pytest.fail("must not open a browser"))
    output = tmp_path / f"drawing.{extension}"
    code = cli_module.main([
        "plot", str(SAMPLE), "-o", str(output), "--dpi", "200",
        "--types", "LINE", "--title", "CLI drawing",
    ])

    captured = capsys.readouterr()
    assert code == 0, captured.err
    assert output.read_bytes().startswith(signature)
    assert output.stat().st_size > 1000
    assert f"output: {output}" in captured.out
    assert not pyplot.get_fignums()
    if extension == "png":
        from PIL import Image

        with Image.open(output) as image:
            assert image.info["dpi"] == pytest.approx((200, 200), abs=0.01)
    elif extension == "svg":
        assert "CLI drawing" in output.read_text()


@pytest.mark.parametrize(("types", "line_count"), [("LINE", 1), ("ARC", 0)])
def test_plot_displays_filtered_drawing(types, line_count, pyplot, monkeypatch) -> None:
    from matplotlib.backend_bases import FigureManagerBase
    from matplotlib.backends.backend_agg import FigureCanvasAgg

    class WindowManager(FigureManagerBase):
        def show(self):
            pass

    monkeypatch.setattr(FigureCanvasAgg, "manager_class", WindowManager)
    monkeypatch.setattr(webbrowser, "open", lambda _uri: pytest.fail("must use the window"))
    shown = []

    def show():
        ax = pyplot.gca()
        shown.append(len(ax.lines))
        assert ax.get_title() == "Drawing preview"
        assert not ax.axison
        if types == "LINE":
            assert list(ax.lines[0].get_xdata()) == [50.0, 100.0]
            assert list(ax.lines[0].get_ydata()) == [50.0, 100.0]

    monkeypatch.setattr(pyplot, "show", show)
    code = cli_module.main([
        "plot", str(SAMPLE), "--types", types, "--title", "Drawing preview",
    ])

    assert code == 0
    assert shown == [line_count]
    assert not pyplot.get_fignums()


@pytest.mark.filterwarnings("error:FigureCanvasAgg is non-interactive")
def test_plot_agg_opens_persistent_svg_preview(pyplot, monkeypatch, tmp_path, capsys) -> None:
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    monkeypatch.setattr(pyplot, "show", lambda: pytest.fail("must not show an Agg canvas"))
    opened = []

    def open_preview(uri):
        previews = list(tmp_path.glob("ezdwg-plot-*.svg"))
        assert len(previews) == 1
        assert uri == previews[0].as_uri()
        assert "Browser preview" in previews[0].read_text()
        opened.append(previews[0])
        return True

    monkeypatch.setattr(webbrowser, "open", open_preview)
    code = cli_module.main(["plot", str(SAMPLE), "--title", "Browser preview"])

    captured = capsys.readouterr()
    assert code == 0, captured.err
    assert len(opened) == 1
    assert opened[0].is_file()  # Still available after figures are closed.
    assert f"preview: {opened[0].as_uri()}" in captured.out
    assert not pyplot.get_fignums()


@pytest.mark.parametrize("browser_error", [None, webbrowser.Error, OSError])
def test_plot_preserves_preview_if_browser_unavailable(
    browser_error, pyplot, monkeypatch, tmp_path, capsys
) -> None:
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))

    def unavailable_browser(uri):
        if browser_error is not None:
            raise browser_error("browser unavailable")
        return False

    monkeypatch.setattr(webbrowser, "open", unavailable_browser)
    assert cli_module.main(["plot", str(SAMPLE)]) == 2
    captured = capsys.readouterr()
    preview, = tmp_path.glob("ezdwg-plot-*.svg")
    assert preview.read_bytes().startswith(b"<?xml")
    assert "could not open a browser automatically" in captured.err
    assert preview.as_uri() in captured.err
    assert not pyplot.get_fignums()


def test_plot_removes_incomplete_preview(pyplot, monkeypatch, tmp_path, capsys) -> None:
    from matplotlib.figure import Figure

    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    monkeypatch.setattr(webbrowser, "open", lambda _uri: pytest.fail("preview is incomplete"))

    def failed_save(*args, **kwargs):
        raise OSError("cannot save preview")

    monkeypatch.setattr(Figure, "savefig", failed_save)
    assert cli_module.main(["plot", str(SAMPLE)]) == 2
    assert "cannot save preview" in capsys.readouterr().err
    assert not list(tmp_path.glob("ezdwg-plot-*"))
    assert not pyplot.get_fignums()


@pytest.mark.parametrize("dpi", ["0", "-1", "invalid"])
def test_plot_rejects_invalid_dpi(dpi, capsys) -> None:
    with pytest.raises(SystemExit) as exc:
        cli_module.main(["plot", str(SAMPLE), "--dpi", dpi])
    assert exc.value.code == 2
    assert "must be a positive integer" in capsys.readouterr().err


def test_plot_reports_missing_file(tmp_path, capsys) -> None:
    assert cli_module.main(["plot", str(tmp_path / "missing.dwg")]) == 2
    assert "file not found" in capsys.readouterr().err


def test_plot_reports_missing_matplotlib(monkeypatch, tmp_path, capsys) -> None:
    monkeypatch.setitem(sys.modules, "matplotlib", None)
    output = tmp_path / "drawing.png"

    assert cli_module.main(["plot", str(SAMPLE), "-o", str(output)]) == 2
    assert 'pip install "ezdwg[plot]"' in capsys.readouterr().err
    assert not output.exists()


def test_plot_help_does_not_require_matplotlib(monkeypatch, capsys) -> None:
    monkeypatch.setitem(sys.modules, "matplotlib", None)
    with pytest.raises(SystemExit) as exc:
        cli_module.main(["plot", "--help"])
    assert exc.value.code == 0
    assert "--output" in capsys.readouterr().out


def test_plot_reports_invalid_dwg(pyplot, tmp_path, capsys) -> None:
    source = tmp_path / "invalid.dwg"
    source.write_bytes(b"not a DWG file")
    output = tmp_path / "drawing.png"

    assert cli_module.main(["plot", str(source), "-o", str(output)]) == 2
    assert "failed to plot DWG" in capsys.readouterr().err
    assert not output.exists()
    assert not pyplot.get_fignums()


@pytest.mark.parametrize("output_name", ["missing/drawing.png", "drawing.invalid"])
def test_plot_reports_save_failure(output_name, pyplot, tmp_path, capsys) -> None:
    output = tmp_path / output_name
    assert cli_module.main(["plot", str(SAMPLE), "--output", str(output)]) == 2
    captured = capsys.readouterr()
    assert "failed to plot DWG" in captured.err
    assert "output:" not in captured.out
    assert not pyplot.get_fignums()
