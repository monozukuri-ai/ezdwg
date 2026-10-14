from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
import ezdwg.convert as convert_module
from ezdwg.entity import Entity


def make_styled_drawing(path: Path) -> None:
    ezdwg.raw.write_ac1015_dwg(
        str(path),
        [(10, 0.0, 0.0, 1.0, 2.0, 3.0, 1.0), (11, 1.0, 1.0, 0.0, 3.0, 2.0, 0.0)],
        [],
        [],
        [(12, 0, [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)], [0.0, 0.5, 0.0], [], 0.2)],
        [(13, "日本語 ABC", (1.0, 2.0, 3.0), 2.0, 0.25)],
        [(14, "注記\\P二行目", (2.0, 3.0, 0.0), (1.0, 0.0, 0.0), 20.0, 2.0, 5, 1)],
        entity_attributes={
            10: {
                "layer": "寸法",
                "color_index": 256,
                "linetype": "DASHED",
                "linetype_scale": 2.0,
                "lineweight": 50,
                "thickness": 2.0,
                "extrusion": (0.0, 1.0, 0.0),
            },
            11: {
                "layer": "寸法",
                "color_index": 0,
                "linetype": "BYBLOCK",
                "lineweight": -2,
                "invisible": True,
            },
            12: {
                "layer": "寸法",
                "color_index": 3,
                "elevation": 7.0,
                "thickness": 0.5,
                "extrusion": (0.0, 1.0, 0.0),
            },
            13: {
                "style": "日本語",
                "layer": "寸法",
                "color_index": 256,
                "align_point": (5.0, 6.0, 3.0),
                "halign": 2,
                "valign": 3,
                "width": 0.8,
                "oblique": 10.0,
                "text_generation_flag": 2,
            },
            14: {
                "style": "日本語",
                "layer": "寸法",
                "line_spacing_style": 2,
                "line_spacing_factor": 1.5,
            },
        },
        layers=[
            {
                "name": "寸法",
                "color_index": 2,
                "linetype": "DASHED",
                "lineweight": 35,
                "off": True,
                "frozen": True,
                "locked": True,
                "plot": False,
                "frozen_in_new_viewports": True,
            }
        ],
        linetypes=[{"name": "DASHED", "description": "破線", "dashes": [1.0, -0.5]}],
        text_styles=[
            {
                "name": "日本語",
                "vertical": True,
                "font": "txt.shx",
                "bigfont": "",
                "width": 1.2,
            }
        ],
        metadata={
            "codepage": 38,
            "insunits": 4,
            "lunits": 2,
            "luprec": 3,
            "aunits": 0,
            "auprec": 2,
            "ltscale": 3.0,
        },
    )


def test_attributes_and_japanese_survive_high_level_rewrite(tmp_path):
    source, out = tmp_path / "source.dwg", tmp_path / "out.dwg"
    make_styled_drawing(source)
    source_doc = ezdwg.read(str(source))
    result = ezdwg.to_dwg(source_doc, str(out), strict=True)
    assert not result.diagnostics
    assert result.written_entities == 5
    doc = ezdwg.read(str(out))
    rows = {e.handle: e.dxf for e in doc.modelspace().query()}
    assert rows[10]["color_index"] == 256
    assert rows[11]["color_index"] == 0
    assert rows[10]["linetype"] == "DASHED"
    assert rows[11]["linetype"] == "BYBLOCK"
    assert rows[10]["linetype_scale"] == 2.0
    assert rows[10]["lineweight"] == 50
    assert rows[11]["lineweight"] == -2
    assert rows[11]["invisible"]
    assert rows[10]["thickness"] == 2.0
    assert rows[10]["extrusion"] == (0.0, 1.0, 0.0)
    assert rows[12]["elevation"] == 7.0
    assert rows[12]["extrusion"] == (0.0, 1.0, 0.0)
    assert not rows[12]["closed"]
    assert rows[13]["text"] == "日本語 ABC"
    assert rows[13]["align_point"] == (5.0, 6.0, 3.0)
    assert rows[13]["halign"] == 2
    assert rows[13]["valign"] == 3
    assert rows[13]["width"] == 0.8
    assert rows[13]["oblique"] == pytest.approx(10.0)
    assert rows[14]["raw_text"] == "注記\\P二行目"
    assert rows[14]["line_spacing_style"] == 2
    assert rows[14]["line_spacing_factor"] == 1.5
    layer = doc.layers["寸法"]
    assert layer.color == 2
    assert layer.frozen and layer.locked and not layer.on and not layer.plot
    assert layer.frozen_in_new_viewports
    assert layer.lineweight == 35
    assert layer.linetype == "DASHED"
    assert doc.linetypes()["DASHED"]["dashes"] == [1.0, -0.5]
    assert doc.insunits == 4
    assert doc.header_variables()["ltscale"] == 3.0


@pytest.mark.parametrize(
    "kind,data",
    [
        ("LINE", {"start": (float("nan"), 0.0, 0.0), "end": (1.0, 0.0, 0.0)}),
        ("CIRCLE", {"center": (0.0, 0.0, 0.0), "radius": -1.0}),
        ("RAY", {"start": (0.0, 0.0, 0.0), "unit_vector": (0.0, 0.0, 0.0)}),
        ("LWPOLYLINE", {"points": [], "flags": 0}),
        ("TEXT", {"text": "bad", "insert": (0.0, 0.0, 0.0), "height": 0.0}),
    ],
)
def test_invalid_geometry_is_reported_before_write(monkeypatch, tmp_path, kind, data):
    source = tmp_path / "source.dwg"
    ezdwg.raw.write_ac1015_line_dwg(str(source), [(10, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0)])
    monkeypatch.setattr(
        convert_module,
        "_resolve_dwg_export_entities",
        lambda *args: [Entity(kind, 10, data)],
    )
    out = tmp_path / "out.dwg"
    out.write_bytes(b"preserve existing destination")
    with pytest.raises(ValueError, match="failed to write"):
        ezdwg.to_dwg(str(source), str(out), strict=True)
    assert out.read_bytes() == b"preserve existing destination"
    result = ezdwg.to_dwg(str(source), str(out))
    assert result.skipped_by_type == {kind: 1}
    assert any(d.action == "skipped" for d in result.diagnostics)
    assert not list(ezdwg.read(str(out)).modelspace().query())


def test_attribute_loss_rejects_in_strict_and_reports_in_normal(monkeypatch, tmp_path):
    source = tmp_path / "source.dwg"
    ezdwg.raw.write_ac1015_line_dwg(str(source), [(10, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0)])
    doc = ezdwg.read(str(source))
    entity = next(doc.modelspace().query())
    entity.dxf["transparency"] = 0.5
    entity.dxf["true_color"] = 0xFF0000
    monkeypatch.setattr(
        convert_module, "_resolve_dwg_export_entities", lambda *args: [entity]
    )
    out = tmp_path / "out.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError, match="transparency"):
        ezdwg.to_dwg(doc, str(out), strict=True)
    assert out.read_bytes() == b"keep"
    result = ezdwg.to_dwg(doc, str(out))
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert any(
        d.attribute == "transparency" and d.action == "dropped"
        for d in result.diagnostics
    )
    # AC1015 keeps the nearest ACI of a true colour and reports the rounding.
    assert any(
        d.attribute == "true_color" and d.action == "approximated"
        for d in result.diagnostics
    )
    (written,) = ezdwg.read(str(out)).modelspace().query("LINE")
    assert written.dxf["color_index"] == 1
    del entity.dxf["transparency"]
    assert ezdwg.to_dwg(doc, str(out), strict=True).written_entities == 1


@pytest.mark.parametrize(
    "rows,kwargs",
    [
        ([(10, float("inf"), 0.0, 0.0, 1.0, 0.0, 0.0)], {}),
        (
            [(10, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            {"entity_attributes": {10: {"color_index": 257}}},
        ),
        (
            [(10, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            {"entity_attributes": {10: {"lineweight": 123}}},
        ),
        (
            [(10, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            {"entity_attributes": {10: {"true_color": 0x123456}}},
        ),
    ],
)
def test_raw_validation_also_preserves_destination(tmp_path, rows, kwargs):
    out = tmp_path / "out.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError):
        ezdwg.raw.write_ac1015_dwg(str(out), rows, [], [], [], [], [], **kwargs)
    assert out.read_bytes() == b"keep"


@pytest.mark.parametrize("text", ["emoji \U0001f600", "bad\0text"])
def test_unrepresentable_text_is_not_silently_replaced(monkeypatch, tmp_path, text):
    source = tmp_path / "source.dwg"
    make_styled_drawing(source)
    doc = ezdwg.read(str(source))
    entity = next(doc.modelspace().query("TEXT"))
    entity.dxf["text"] = text
    monkeypatch.setattr(
        convert_module, "_resolve_dwg_export_entities", lambda *args: [entity]
    )
    out = tmp_path / "out.dwg"
    with pytest.raises(ValueError, match="string cannot be stored"):
        ezdwg.to_dwg(doc, str(out), strict=True)
    assert not out.exists()
    result = ezdwg.to_dwg(doc, str(out))
    assert any(
        d.attribute == "text" and d.action == "replaced" for d in result.diagnostics
    )


def test_unknown_raw_attribute_is_rejected(tmp_path):
    with pytest.raises(ValueError, match="unsupported writer attribute"):
        ezdwg.raw.write_ac1015_dwg(
            str(tmp_path / "x.dwg"),
            [(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)],
            [],
            [],
            [],
            [],
            [],
            entity_attributes={1: {"layer_typo": "x"}},
        )
