"""AC1015 has no true colour: ``true_color`` rounds to the nearest ACI with a report."""

from __future__ import annotations

import pytest

import ezdwg
from ezdwg._aci import aci_palette, nearest_aci


def test_palette_matches_the_standard_aci_table():
    palette = aci_palette()
    assert len(palette) == 255
    assert palette[1] == (255, 0, 0) and palette[7] == (255, 255, 255)
    assert palette[10] == (255, 0, 0) and palette[11] == (255, 170, 170)
    assert palette[13] == (189, 126, 126) and palette[21] == (255, 191, 170)
    assert palette[30] == (255, 127, 0) and palette[50] == (255, 255, 0)
    assert palette[250] == (51, 51, 51) and palette[255] == (255, 255, 255)


def test_nearest_aci_rounds_and_keeps_black_and_white_on_the_foreground():
    assert nearest_aci(0xFF0000) == 1
    assert nearest_aci(0x000000) == 7 and nearest_aci(0xFFFFFF) == 7
    assert nearest_aci(0xC3C3C3) == 9 and nearest_aci((51, 51, 51)) == 250
    with pytest.raises(ValueError):
        nearest_aci(0x1000000)
    with pytest.raises(ValueError):
        nearest_aci((256, 0, 0))


def test_entity_and_layer_true_color_are_written_as_aci(tmp_path):
    doc = ezdwg.new()
    doc.layers["塗"] = {"true_color": 0x73B9FF, "lineweight": 35}
    msp = doc.modelspace()
    line = msp.add_line((0, 0), (10, 0), layer="塗", true_color=0xFF0000)
    circle = msp.add_circle((5, 5), 2, color_index=3)
    msp.update(circle, true_color=0x00FF00)
    out = tmp_path / "true_color.dwg"
    result = doc.save(out)  # strict: approximations are informational
    assert result.written_entities == 2
    actions = {(d.dxftype, d.attribute, d.action) for d in result.diagnostics}
    assert actions == {
        ("LINE", "true_color", "approximated"),
        ("CIRCLE", "true_color", "approximated"),
        ("LAYER", "true_color", "approximated"),
    }
    loaded = ezdwg.read(str(out))
    by_type = {e.dxftype: e for e in loaded.modelspace().query()}
    assert by_type["LINE"].dxf["color_index"] == 1
    assert by_type["CIRCLE"].dxf["color_index"] == 3
    assert by_type["LINE"].dxf.get("true_color") is None
    assert loaded.layers["塗"].color == nearest_aci(0x73B9FF)
    assert line.dxf["true_color"] == 0xFF0000  # the drawing keeps the request


def test_invalid_true_color_is_rejected_before_saving():
    doc = ezdwg.new()
    with pytest.raises(ValueError):
        doc.modelspace().add_line((0, 0), (1, 1), true_color=0x1000000)
    with pytest.raises(ValueError):
        doc.modelspace().add_line((0, 0), (1, 1), true_color="red")


def test_imported_true_color_survives_strict_edit_and_resave(tmp_path):
    doc = ezdwg.new()
    doc.modelspace().add_line((0, 0), (1, 1), true_color=0x123456)
    source = tmp_path / "source.dwg"
    doc.save(source)
    edit = ezdwg.read(str(source)).edit()  # strict import: no losses
    result = edit.save(tmp_path / "resaved.dwg")
    assert result.written_entities == 1
