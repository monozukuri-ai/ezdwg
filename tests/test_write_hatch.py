"""Native AC1015 HATCH writing: solid and pattern fills, polyline and edge paths."""

from __future__ import annotations

import math

import pytest

import ezdwg
from ezdwg._hatch import PATTERNS, transform_pattern_lines
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export

RECT = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]


def hatches(path):
    return list(ezdwg.read(str(path)).modelspace().query("HATCH"))


def test_solid_hatch_roundtrip(tmp_path):
    doc = ezdwg.new()
    doc.layers["塗"] = {"color_index": 4}
    msp = doc.modelspace()
    hatch = msp.add_hatch([RECT], layer="塗", color_index=1, elevation=2.0, hatch_style=1)
    assert hatch.dxf["solid_fill"] and hatch.dxf["pattern_name"] == "SOLID"
    out = tmp_path / "solid.dwg"
    result = doc.save(out)
    assert result.written_entities == 1 and not result.diagnostics
    (back,) = hatches(out)
    assert back.dxf["solid_fill"] and not back.dxf["associative"]
    assert back.dxf["pattern_name"] == "SOLID"
    assert back.dxf["elevation"] == 2.0 and back.dxf["color_index"] == 1
    (path,) = back.dxf["paths"]
    assert path["closed"]
    assert [p[:2] for p in path["points"]] == RECT + [RECT[0]]
    (raw,) = ezdwg.raw.decode_hatch_entities(str(out))
    assert raw[1] == "SOLID" and raw[2] is True


def test_pattern_hatch_keeps_transformed_definition_and_edges(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    edges = [
        {"type": "line", "start": (0, 0), "end": (4, 0)},
        {"type": "arc", "center": (4, 2), "radius": 2, "start_angle": -90, "end_angle": 90},
        {"type": "line", "start": (4, 4), "end": (0, 4)},
        {"type": "line", "start": (0, 4), "end": (0, 0)},
    ]
    hatch = msp.add_hatch(
        [{"edges": edges}],
        pattern="ANSI31",
        pattern_angle=30,
        pattern_scale=25.4,
    )
    assert not hatch.dxf["solid_fill"] and hatch.dxf["pattern_name"] == "ANSI31"
    (line,) = hatch.dxf["pattern_lines"]
    assert line["angle"] == pytest.approx(75.0)
    assert line["offset"] == pytest.approx(
        (-3.175 * math.sin(math.radians(75)), 3.175 * math.cos(math.radians(75)))
    )
    out = tmp_path / "pattern.dwg"
    doc.save(out)
    (back,) = hatches(out)
    assert back.dxf["pattern_name"] == "ANSI31" and not back.dxf["solid_fill"]
    assert back.dxf["pattern_angle"] == pytest.approx(30.0)
    assert back.dxf["pattern_scale"] == pytest.approx(25.4)
    (stored,) = back.dxf["pattern_lines"]
    assert stored["angle"] == pytest.approx(75.0)
    assert stored["offset"] == pytest.approx(line["offset"])
    (path,) = back.dxf["paths"]
    assert path["closed"] and len(path["points"]) > 10  # the arc edge is tessellated
    assert path["points"][0][:2] == (0.0, 0.0) and path["points"][-1][:2] == (0.0, 0.0)


def test_custom_pattern_rows_and_bulged_polyline_paths(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    msp.add_hatch(
        [
            {"points": [(0, 0), (10, 0), (10, 10)], "bulges": [0.0, 1.0, 0.0], "closed": True},
            {"points": [(3, 3), (5, 3), (5, 5), (3, 5)], "closed": True, "flags": 2 | 16},
        ],
        pattern=[(0.0, 0.0, 0.0, 0.0, 0.5, [1.0, -0.5])],
        pattern_double=True,
    )
    out = tmp_path / "custom.dwg"
    doc.save(out)
    (back,) = hatches(out)
    assert back.dxf["pattern_type"] == 0 if "pattern_type" in back.dxf else True
    assert back.dxf["pattern_double"] is True
    assert len(back.dxf["paths"]) == 2
    assert len(back.dxf["paths"][0]["points"]) > 4  # bulge tessellated by the reader
    assert back.dxf["pattern_lines"][0]["dashes"] == pytest.approx([1.0, -0.5])


def test_transform_pattern_lines_matches_acad_storage():
    (line,) = transform_pattern_lines("ANSI31", 0.0, 25.4)
    assert line["angle"] == 45.0
    assert line["offset"] == pytest.approx((-2.2450640303, 2.2450640303))
    assert set(PATTERNS) >= {"ANSI31", "ANSI37", "DOTS", "NET"}
    with pytest.raises(ValueError):
        transform_pattern_lines("NOPE")


@pytest.mark.parametrize(
    "paths,kwargs,match",
    [
        ([], {}, "at least one"),
        ([[(0, 0)]], {}, "at least 2"),
        ([{"points": RECT, "bulges": [0.0]}], {}, "bulge count"),
        ([{"edges": []}], {}, "at least one edge"),
        ([{"edges": [{"type": "arc", "center": (0, 0), "radius": 0}]}], {}, "radius"),
        ([RECT], {"pattern": "ANSI31", "pattern_scale": 0}, "scale"),
        ([RECT], {"hatch_style": 3}, "hatch_style"),
        ([{"points": RECT, "flags": 7}], {}, "pixel_size"),
    ],
)
def test_invalid_hatches_are_rejected(paths, kwargs, match):
    with pytest.raises(ValueError, match=match):
        ezdwg.new().modelspace().add_hatch(paths, **kwargs)


def test_hatch_in_block_and_import_edit_resave(tmp_path):
    doc = ezdwg.new()
    block = doc.blocks.new("Part")
    block.add_hatch([RECT], color_index=2)
    doc.modelspace().add_blockref("Part", (20, 0))
    doc.modelspace().add_hatch([RECT], pattern="NET", pattern_scale=25.4)
    source = tmp_path / "source.dwg"
    assert doc.save(source).written_entities == 3
    edit = ezdwg.read(str(source)).edit()
    assert {e.dxftype for e in edit.modelspace()} == {"INSERT", "HATCH"}
    assert {e.dxftype for e in edit.blocks["Part"]} == {"HATCH"}
    hatch = next(edit.modelspace().query("HATCH"))
    assert hatch.dxf["pattern_name"] == "NET" and len(hatch.dxf["pattern_lines"]) == 2
    edit.modelspace().update(hatch, color_index=5)
    out = tmp_path / "resaved.dwg"
    assert edit.save(out).written_entities == 3
    (back,) = [e for e in hatches(out) if e.dxf["pattern_name"] == "NET"]
    assert back.dxf["color_index"] == 5


def test_hatch_external_read(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    msp = doc.modelspace()
    msp.add_hatch([RECT], color_index=1)
    msp.add_hatch([[(20, 0), (30, 0), (25, 8)]], pattern="ANSI31", pattern_scale=25.4)
    source = tmp_path / "hatch.dwg"
    doc.save(source)
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf = ezdxf.readfile(convert(oda_command, resaved, tmp_path / "dxf", "DXF"))
    found = list(dxf.modelspace().query("HATCH"))
    assert len(found) == 2
    solid = [h for h in found if h.dxf.solid_fill][0]
    pattern = [h for h in found if not h.dxf.solid_fill][0]
    assert solid.dxf.pattern_name == "SOLID" and len(solid.paths) == 1
    assert pattern.dxf.pattern_name == "ANSI31"
    assert pattern.pattern is not None and len(pattern.pattern.lines) == 1
    assert pattern.pattern.lines[0].angle == pytest.approx(45.0)
    assert len(hatches(resaved)) == 2
