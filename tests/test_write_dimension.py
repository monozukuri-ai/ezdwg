"""Native AC1015 DIMENSION writing with rendered anonymous blocks."""

from __future__ import annotations

import math

import pytest

import ezdwg
from ezdwg._dimension import format_measurement, render_dimension
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export


def dimensions(path):
    return {e.dxf["dimtype"]: e for e in ezdwg.read(str(path)).modelspace().query("DIMENSION")}


def test_linear_dimension_renders_block_and_roundtrips(tmp_path):
    doc = ezdwg.new()
    doc.layers["寸法"] = {"color_index": 1}
    msp = doc.modelspace()
    dim = msp.add_linear_dim((0, 10), (0, 0), (50, 0), layer="寸法", text="<> mm")
    assert dim.dxf["geometry"] == "*D1" and dim.dxf["actual_measurement"] == 50.0
    assert dim.dxf["text_midpoint"] == pytest.approx((25.0, 11.875, 0.0))
    kinds = [e.dxftype for e in doc.blocks["*D1"]]
    assert kinds.count("LINE") == 3 and kinds.count("SOLID") == 2 and kinds.count("TEXT") == 1
    text = next(e for e in doc.blocks["*D1"] if e.dxftype == "TEXT")
    assert text.dxf["text"] == "50 mm" and text.dxf["color_index"] == 0
    out = tmp_path / "linear.dwg"
    result = doc.save(out)
    assert result.written_entities == 7 and not result.diagnostics
    back = dimensions(out)["LINEAR"]
    assert back.dxf["defpoint2"] == (0.0, 0.0, 0.0) and back.dxf["defpoint3"] == (50.0, 0.0, 0.0)
    assert back.dxf["defpoint"] == (0.0, 10.0, 0.0)
    assert back.dxf["text"] == "<> mm" and back.dxf["actual_measurement"] == pytest.approx(50.0)
    assert back.dxf["anonymous_block_name"] == "*D1"
    assert back.dxf["attachment_point"] == 5 and back.dxf["dim_flags"] == 3
    loaded = ezdwg.read(str(out))
    assert loaded.layers["寸法"].color == 1


def test_every_dimension_kind_writes_its_points(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    msp.add_aligned_dim((0, 20), (30, 40), 5)
    msp.add_radius_dim((80, 10), 12, angle=30)
    msp.add_diameter_dim((80, 40), 10, angle=0)
    msp.add_angular_dim((120, 0), (140, 0), (120, 20), 15)
    msp.add_ordinate_dim((150, 10), (150, 25))
    msp.add_dimension(
        "angular2ln",
        defpoint2=(0, 100),
        defpoint3=(40, 100),
        defpoint4=(0, 100),
        defpoint=(0, 140),
        defpoint5=(20, 120),
    )
    out = tmp_path / "kinds.dwg"
    doc.save(out)
    back = dimensions(out)
    assert set(back) == {"ALIGNED", "RADIUS", "DIAMETER", "ANG3PT", "ANG2LN", "ORDINATE"}
    assert back["ALIGNED"].dxf["actual_measurement"] == pytest.approx(math.hypot(30, 20))
    assert back["RADIUS"].dxf["defpoint"] == (80.0, 10.0, 0.0)
    assert back["RADIUS"].dxf["defpoint4"] == pytest.approx((80 + 12 * math.cos(math.radians(30)), 10 + 6.0, 0.0))
    assert back["DIAMETER"].dxf["defpoint"] == pytest.approx((90.0, 40.0, 0.0))
    assert back["DIAMETER"].dxf["defpoint4"] == pytest.approx((70.0, 40.0, 0.0))
    assert back["ANG3PT"].dxf["defpoint4"] == (120.0, 0.0, 0.0)
    assert back["ANG3PT"].dxf["actual_measurement"] == pytest.approx(90.0)
    assert back["ANG2LN"].dxf["defpoint5"] == pytest.approx((20.0, 120.0, 0.0))
    assert back["ORDINATE"].dxf["defpoint2"] == (150.0, 10.0, 0.0)
    assert back["ORDINATE"].dxf["actual_measurement"] == pytest.approx(150.0)
    assert sorted(doc.blocks) == [f"*D{i}" for i in range(1, 7)]


def test_update_rerenders_and_custom_geometry_is_kept(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    dim = msp.add_linear_dim((0, 10), (0, 0), (50, 0))
    msp.update(dim, defpoint3=(80, 0, 0))
    assert dim.dxf["actual_measurement"] == 80.0
    text = next(e for e in doc.blocks["*D1"] if e.dxftype == "TEXT")
    assert text.dxf["text"] == "80"
    custom = doc.blocks.new("*D9")
    custom.add_line((0, 0), (1, 1))
    fixed = msp.add_dimension(
        "linear", defpoint=(0, 5), defpoint2=(0, 0), defpoint3=(20, 0), geometry="*D9", text_midpoint=(10, 6)
    )
    assert fixed.dxf["geometry"] == "*D9" and "actual_measurement" not in fixed.dxf
    assert doc.save(tmp_path / "custom.dwg").written_entities == 9


def test_format_and_render_helpers():
    assert format_measurement(12.5) == "12.5" and format_measurement(12.0) == "12"
    assert format_measurement(0.004) == "0" and format_measurement(1234.567) == "1234.57"
    entities, derived = render_dimension(
        {"dimtype": "radius", "defpoint": (0, 0, 0), "defpoint4": (3, 4, 0)}
    )
    assert derived["actual_measurement"] == pytest.approx(5.0)
    assert any(dxf.get("text") == "R5" for _, dxf in entities)


@pytest.mark.parametrize(
    "kind,kwargs,match",
    [
        ("linear", {"defpoint": (0, 0), "defpoint2": (0, 0)}, "defpoint3"),
        ("radius", {"defpoint": (0, 0), "defpoint4": (1, 0), "leader_length": -1}, "leader_length"),
        ("linear", {"defpoint": (0, 0), "defpoint2": (0, 0), "defpoint3": (1, 0), "attachment_point": 0}, "attachment"),
        ("linear", {"defpoint": (0, 0), "defpoint2": (0, 0), "defpoint3": (1, 0), "dimstyle": "ISO-25"}, "Standard"),
        ("bogus", {"defpoint": (0, 0)}, "unsupported"),
    ],
)
def test_invalid_dimensions_are_rejected(kind, kwargs, match):
    with pytest.raises(ValueError, match=match):
        ezdwg.new().modelspace().add_dimension(kind, **kwargs)


def test_imported_dimension_keeps_its_block_through_edit_and_resave(tmp_path):
    doc = ezdwg.new()
    doc.modelspace().add_linear_dim((0, 10), (0, 0), (50, 0))
    source = tmp_path / "source.dwg"
    doc.save(source)
    edit = ezdwg.read(str(source)).edit()
    dim = next(edit.modelspace().query("DIMENSION"))
    assert dim.dxf["dimtype"] == "linear" and dim.dxf["geometry"] == "*D1"
    assert {e.dxftype for e in edit.blocks["*D1"]} == {"LINE", "SOLID", "TEXT"}
    edit.modelspace().update(dim, color_index=2)
    out = tmp_path / "resaved.dwg"
    assert edit.save(out).written_entities == 7
    back = dimensions(out)["LINEAR"]
    assert back.dxf["color_index"] == 2 and back.dxf["anonymous_block_name"] == "*D1"
    # Plain re-export of a read document carries the dimension and its block.
    result = ezdwg.to_dwg(str(source), str(tmp_path / "reexport.dwg"), strict=True)
    assert result.written_entities == 7
    assert dimensions(tmp_path / "reexport.dwg")["LINEAR"].dxf["actual_measurement"] == pytest.approx(50.0)


def test_dimension_external_read(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    msp = doc.modelspace()
    msp.add_linear_dim((0, 10), (0, 0), (50, 0))
    msp.add_radius_dim((80, 10), 12)
    source = tmp_path / "dims.dwg"
    doc.save(source)
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf = ezdxf.readfile(convert(oda_command, resaved, tmp_path / "dxf", "DXF"))
    dims = list(dxf.modelspace().query("DIMENSION"))
    assert len(dims) == 2
    linear = next(d for d in dims if d.dimtype == 0)
    assert linear.dxf.defpoint2.isclose((0, 0, 0)) and linear.dxf.defpoint3.isclose((50, 0, 0))
    assert linear.dxf.geometry.startswith("*D")
    block = dxf.blocks.get(linear.dxf.geometry)
    assert {e.dxftype() for e in block} >= {"LINE", "SOLID", "TEXT"}
    assert dxf.dimstyles.get("Standard") is not None
    assert len(dimensions(resaved)) == 2
