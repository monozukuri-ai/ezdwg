"""Write-only drawings: the same file as a Drawing, without keeping entities."""

from __future__ import annotations

import pytest

import ezdwg
from tests.test_write_batches import mixed_drawing


def mixed_streaming() -> ezdwg.StreamingDrawing:
    """``mixed_drawing`` rebuilt through the streaming API, tables arriving late."""
    drawing = ezdwg.new_streaming()
    drawing.header["codepage"] = 38  # cp932: the Japanese names below stay as they are
    drawing.layers["部品"] = {"color_index": 2}
    block = drawing.blocks.new("部品A", base_point=(1, 2, 3), description="Nested component")
    block.add_line((1, 2, 3), (5, 2, 3), color_index=0, layer="0")
    block.add_attdef("PART", "既定値", (1, 3, 3), height=1.5, prompt="部品番号", attribute_flags=8)
    block.add_attdef("FIXED", "定数", (1, 4, 3), height=1.5, attribute_flags=2)
    parent = drawing.blocks.new("Assembly", base_point=(2, 0, 0))
    nested = parent.add_blockref("部品A", (3, 4, 5), scale=(-2, 3, 1), rotation=35)
    parent.add_attrib(nested, "PART", "内側", (3, 5, 5), height=2, color_index=0)
    msp = drawing.modelspace()
    msp.add_blockref("Assembly", (10, 20, 30), rotation=90, scale=(2, 2, 2), layer="部品", color_index=3)
    other = msp.add_blockref("部品A", (20, 0, 0), extrusion=(0, 1, 0))
    msp.add_attrib(other, "PART", "外側", (20, 1, 0), height=2, extrusion=(0, 1, 0), attribute_flags=1)
    drawing.blocks.new("Empty")
    # Tables after the first entities: layers, linetypes, styles and image
    # definitions are written last, so they may arrive at any time.
    drawing.layers["Fill"] = {"color_index": 1}
    drawing.linetypes["DASHED"] = {"description": "- -", "dashes": [1.0, -0.5]}
    drawing.text_styles["Narrow"] = {"font": "txt.shx", "width": 0.8}
    drawing.image_defs["scan"] = {"file_path": "scan.png", "size": (200, 100), "pixel_size": (0.1, 0.1)}
    for i in range(7):
        x = float(i)
        msp.add_text(f"T{i}", (x, 0), height=1.5, style="Narrow")
        msp.add_line((x, 0), (x + 1, 1), layer="Fill", linetype="DASHED")
        msp.add_hatch([{"points": [(x, 2), (x + 1, 2), (x + 1, 3)], "closed": True}], layer="Fill")
        msp.add_lwpolyline([(x, 4), (x + 1, 4), (x + 1, 5)], closed=i % 2 == 0, lineweight=35)
        msp.add_circle((x, 6), 0.4, color_index=3)
        msp.add_arc((x, 8), 0.4, 0, 90)
        msp.add_point((x, 10))
        msp.add_ellipse((x, 12), (0.5, 0, 0), 0.5, color_index=4)
        msp.add_solid([(x, 14, 0), (x + 1, 14, 0), (x + 1, 15, 0), (x, 15, 0)])
        msp.add_mtext(f"M{i}\\PLine", (x, 16), char_height=1, rect_width=5)
    msp.add_ray((0, 20), (1, 0, 0))
    msp.add_xline((0, 21), (0, 1, 0))
    msp.add_3dface([(0, 22, 0), (1, 22, 0), (1, 23, 1), (0, 23, 1)])
    msp.add_trace([(0, 24, 0), (1, 24, 0), (1, 25, 0), (0, 25, 0)])
    msp.add_spline([(0, 26, 0), (1, 27, 0), (2, 26, 0), (3, 27, 0)], 3)
    msp.add_polyline2d([(0, 28), (1, 29), (2, 28)])
    msp.add_polyline3d([(0, 30, 0), (1, 31, 1), (2, 30, 2)])
    msp.add_linear_dim((0, 32), (5, 32), (2.5, 34))
    msp.add_image("scan", (0, 36), width=20)
    psp = drawing.paperspace()
    psp.add_line((0, 0), (420, 0))
    psp.add_viewport((210, 148.5), 400, 280, view_center=(5, 15), view_height=40)
    # The sheet may be set after its entities: the layout is written last.
    drawing.set_paper(420, 297, margins=(5, 5, 5, 5))
    return drawing


def _summary(path) -> dict:
    doc = ezdwg.read(str(path))
    kinds = sorted(e.dxftype for e in doc.modelspace().query("*"))
    paper = sorted(e.dxftype for e in doc.paperspace().query("*"))
    layers = sorted(layer.name for layer in doc.layers)
    return {"kinds": kinds, "paper": paper, "layers": layers}


def test_streaming_drawing_matches_the_editable_drawing(tmp_path):
    reference = mixed_drawing()
    reference.set_paper(420, 297, margins=(5, 5, 5, 5))
    kept = tmp_path / "kept.dwg"
    expected = reference.save(kept)
    assert expected.skipped_entities == 0

    streamed = tmp_path / "streamed.dwg"
    drawing = mixed_streaming()
    assert len(drawing.modelspace()) > 70 and not list(drawing.modelspace().query("*"))
    result = drawing.save(streamed)
    assert result.skipped_entities == 0
    assert result.written_entities == expected.written_entities
    assert _summary(streamed) == _summary(kept)
    assert len(ezdwg.raw.decode_attrib_entities(str(streamed))) == 2
    assert {d["name"] for d in ezdwg.raw.decode_write_blocks(str(streamed))} >= {"部品A", "Assembly", "Empty"}
    with pytest.raises(ValueError, match="already been saved"):
        drawing.modelspace().add_line((0, 0), (1, 1))


def test_streaming_drawing_is_write_only(tmp_path):
    drawing = ezdwg.new_streaming(max_entities=3)
    msp = drawing.modelspace()
    line = msp.add_line((0, 0), (1, 1))
    with pytest.raises(TypeError, match="cannot update"):
        msp.update(line, end=(2, 2))
    with pytest.raises(TypeError, match="cannot delete"):
        msp.delete(line)
    with pytest.raises(KeyError):
        msp.get(line.handle)
    msp.add_line((0, 0), (1, 2))
    msp.add_line((0, 0), (1, 3))
    with pytest.raises(ValueError, match="max_entities"):
        msp.add_line((0, 0), (1, 4))


def test_streaming_strict_failure_writes_nothing(tmp_path):
    drawing = ezdwg.new_streaming()
    drawing.header["codepage"] = 30  # cp1252 cannot store the text below
    drawing.modelspace().add_text("日本語", (0, 0), height=1)
    out = tmp_path / "strict.dwg"
    with pytest.raises(ValueError, match="string cannot be stored"):
        drawing.save(out)
    assert not out.exists()
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))


def test_streaming_rejects_unknown_block_and_code_page_change(tmp_path):
    drawing = ezdwg.new_streaming()
    with pytest.raises(ValueError, match="undefined block"):
        drawing.modelspace().add_blockref("Missing", (0, 0))
    drawing.header["codepage"] = 38
    drawing.modelspace().add_text("図", (0, 0), height=1)
    drawing.header["codepage"] = 30
    with pytest.raises(ValueError, match="code page is fixed"):
        drawing.save(tmp_path / "late.dwg")
    # Without a requested code page it is chosen from the table names.
    drawing = ezdwg.new_streaming()
    drawing.layers["図面"] = {"color_index": 3}
    drawing.modelspace().add_line((0, 0), (1, 1), layer="図面")
    out = tmp_path / "auto.dwg"
    assert not drawing.save(out).diagnostics
    assert "図面" in {layer.name for layer in ezdwg.read(str(out)).layers}
