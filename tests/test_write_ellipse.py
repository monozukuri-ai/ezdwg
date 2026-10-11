from __future__ import annotations

import math
from pathlib import Path

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export

SAMPLES = Path(__file__).resolve().parents[1] / "test_dwg"
# Center, semi-major axis, normal, minor/major ratio, start/end parameters.
SHAPES = [
    ((4, 5, 6), (3, 4, 0), (0, 0, 1), 0.35, 0.0, math.tau),
    ((-2, 8, 1), (2, 3, -3), (0, 1, 1), 0.2, 0.3, 4.9),
    ((1, 2, 3), (3, 0, 4), (0, -1, 0), 1.0, 5.5, 0.5),
    ((9, 0, 2), (-5, 0, 0), (0, 0, -1), 0.7, -1.2, 2.7),
    ((0, 0, 0), (100, 0, 0), (0, 0, 1), 1e-6, 0.0, math.tau),
]


def add_shape(layout, shape):
    center, axis, normal, ratio, start, end = shape
    return layout.add_ellipse(center, axis, ratio, start, end, extrusion=normal)


def assert_raw_shape(row, shape):
    for actual, expected in zip(row[1:], shape):
        assert actual == pytest.approx(expected)


def test_new_edit_save_ellipse_geometry_and_formatting(tmp_path):
    doc = ezdwg.new()
    doc.layers["楕円"] = {"color_index": 3}
    doc.linetypes["DASH"] = {"dashes": [0.5, -0.25]}
    msp = doc.modelspace()
    ellipses = [add_shape(msp, s) for s in SHAPES]
    msp.update(
        ellipses[0],
        layer="楕円",
        color_index=2,
        lineweight=35,
        linetype="DASH",
        linetype_scale=2.0,
        invisible=True,
    )
    path = tmp_path / "ellipses.dwg"
    result = doc.save(path)
    assert result.written_entities == len(SHAPES)
    assert not result.diagnostics
    rows = ezdwg.raw.decode_ellipse_entities(str(path))
    assert len(rows) == len(SHAPES)
    for row, shape in zip(rows, SHAPES):
        assert_raw_shape(row, shape)
    source = ezdwg.read(str(path))
    entity = next(source.modelspace().query("ELLIPSE"))
    assert entity.dxf["linetype"] == "DASH"
    assert entity.dxf["color_index"] == 2
    assert entity.dxf["lineweight"] == 35
    assert entity.dxf["linetype_scale"] == 2.0
    assert entity.dxf["invisible"]
    edited = source.edit()
    edited.modelspace().update(
        ellipses[0].handle, axis_ratio=0.6, start_angle=0.2, end_angle=2.1
    )
    edited.modelspace().delete(ellipses[-1].handle)
    added = edited.modelspace().add_ellipse((0, 0))
    assert added.dxf["end_angle"] == math.tau
    path.unlink()  # The editable snapshot is independent of the source file.
    edited.save(path)
    out = ezdwg.read(str(path)).edit()
    assert out.modelspace().get(ellipses[0].handle).dxf["axis_ratio"] == 0.6
    assert len(out.modelspace()) == len(SHAPES)


@pytest.mark.parametrize("version", ["R14", "2004", "2007", "2010", "2013"])
def test_existing_ellipse_fixture_strict_native_export(tmp_path, version):
    source = SAMPLES / f"ellipse_{version}.dwg"
    expected = ezdwg.raw.decode_ellipse_entities(str(source))
    assert len(expected) == 1
    output = tmp_path / "ellipse.dwg"
    result = ezdwg.to_dwg(str(source), str(output), types="ELLIPSE", strict=True)
    assert result.written_entities == 1 and not result.diagnostics
    assert_raw_shape(ezdwg.raw.decode_ellipse_entities(str(output))[0], expected[0][1:])
    copy = ezdwg.read(str(source)).edit()
    assert len(list(copy.modelspace().query("ELLIPSE"))) == 1


INVALID = [
    {"center": (float("nan"), 0, 0)},
    {"major_axis": (0, 0, 0)},
    {"major_axis": (1e-7, 0, 0)},
    {"major_axis": (1, 0, 1)},
    {"major_axis": (1e308, 1e308, 1e308)},
    {"extrusion": (0, 0, 0)},
    {"axis_ratio": 0},
    {"axis_ratio": -0.5},
    {"axis_ratio": 1e-7},
    {"axis_ratio": 1.01},
    {"axis_ratio": float("inf")},
    {"end_angle": float("nan")},
    {"end_angle": 0},
    {"end_angle": 1e-7},
    {"end_angle": 2 * math.tau},
]


@pytest.mark.parametrize("invalid", INVALID)
def test_invalid_ellipse_rejected_by_editable_and_raw_writers(tmp_path, invalid):
    data = {
        "center": (0, 0, 0),
        "major_axis": (2, 0, 0),
        "extrusion": (0, 0, 1),
        "axis_ratio": 0.5,
        "start_angle": 0.0,
        "end_angle": math.tau,
    }
    data.update(invalid)
    with pytest.raises(ValueError):
        ezdwg.new().modelspace().add_entity("ELLIPSE", data)
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    row = (
        10,
        *(
            data[k]
            for k in (
                "center",
                "major_axis",
                "extrusion",
                "axis_ratio",
                "start_angle",
                "end_angle",
            )
        ),
    )
    with pytest.raises(ValueError):
        ezdwg.raw.write_ac1015_dwg(str(path), [], [], [], [], [], [], ellipses=[row])
    assert path.read_bytes() == b"keep"


def test_invalid_mutation_skip_and_unsupported_thickness(tmp_path):
    doc = ezdwg.new()
    ellipse = doc.modelspace().add_ellipse((0, 0), (2, 0, 0), 0.5)
    doc.modelspace().add_point((5, 6))
    path = tmp_path / "out.dwg"
    path.write_bytes(b"keep")
    with pytest.raises(ValueError):
        doc.modelspace().update(ellipse, axis_ratio=2)
    assert ellipse.dxf["axis_ratio"] == 0.5
    ellipse.dxf["axis_ratio"] = 2
    with pytest.raises(ValueError, match="ELLIPSE"):
        doc.save(path)
    assert path.read_bytes() == b"keep"
    result = doc.save(path, strict=False)
    assert result.skipped_by_type == {"ELLIPSE": 1}
    assert result.written_entities == 1
    assert list(ezdwg.read(str(path)).modelspace().query("ELLIPSE")) == []
    with pytest.raises(ValueError, match="thickness"):
        doc.modelspace().add_ellipse((0, 0), thickness=2)
    with pytest.raises(ValueError, match="thickness"):
        ezdwg.raw.write_ac1015_dwg(
            str(path),
            [],
            [],
            [],
            [],
            [],
            [],
            ellipses=[(10, *SHAPES[0])],
            entity_attributes={10: {"thickness": 2}},
        )


def assert_external_shape(entity, shape):
    center, axis, normal, ratio, start, end = shape
    assert entity.dxftype() == "ELLIPSE"
    assert tuple(entity.dxf.center) == pytest.approx(center)
    assert tuple(entity.dxf.major_axis) == pytest.approx(axis)
    normal = tuple(v / math.hypot(*normal) for v in normal)
    actual_normal = entity.dxf.extrusion.normalize()
    assert tuple(actual_normal) == pytest.approx(normal)
    assert entity.dxf.ratio == pytest.approx(ratio)
    assert math.sin(entity.dxf.start_param) == pytest.approx(math.sin(start), abs=1e-12)
    assert math.cos(entity.dxf.start_param) == pytest.approx(math.cos(start), abs=1e-12)
    assert math.sin(entity.dxf.end_param) == pytest.approx(math.sin(end), abs=1e-12)
    assert math.cos(entity.dxf.end_param) == pytest.approx(math.cos(end), abs=1e-12)
    minor = (
        normal[1] * axis[2] - normal[2] * axis[1],
        normal[2] * axis[0] - normal[0] * axis[2],
        normal[0] * axis[1] - normal[1] * axis[0],
    )
    params = [start, (start + end) / 2, end]
    for p, actual in zip(params, entity.vertices(params)):
        expected = tuple(
            c + a * math.cos(p) + b * ratio * math.sin(p)
            for c, a, b in zip(center, axis, minor)
        )
        assert tuple(actual) == pytest.approx(expected)


def test_raw_ellipse_normal_survives_partial_attribute_dictionary(tmp_path):
    path = tmp_path / "ellipse.dwg"
    ezdwg.raw.write_ac1015_dwg(
        str(path),
        [],
        [],
        [],
        [],
        [],
        [],
        ellipses=[(100, *SHAPES[1])],
        entity_attributes={100: {"color_index": 2}},
    )
    (row,) = ezdwg.raw.decode_ellipse_entities(str(path))
    assert_raw_shape(row, SHAPES[1])


def test_ellipse_oda_read_resave_and_nested_blocks(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    drawing = ezdwg.new()
    for shape in SHAPES:
        add_shape(drawing.modelspace(), shape)
    block = drawing.blocks.new("Part", base_point=(1, 2, 3))
    add_shape(block, SHAPES[1])
    assembly = drawing.blocks.new("Assembly")
    assembly.add_blockref("Part", (10, 20), scale=(-2, 3, 1), rotation=30)
    drawing.modelspace().add_blockref("Assembly", (100, 200))
    source = tmp_path / "ellipses.dwg"
    drawing.save(source)
    dxf = convert(oda_command, source, tmp_path / "dxf", "DXF")
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf_again = convert(oda_command, resaved, tmp_path / "dxf-again", "DXF")
    for path in (dxf, dxf_again):
        external = ezdxf.readfile(path)
        audit = external.audit()
        assert not audit.errors and not audit.fixes
        entities = list(external.modelspace().query("ELLIPSE"))
        assert len(entities) == len(SHAPES)
        for entity, shape in zip(entities, SHAPES):
            assert_external_shape(entity, shape)
        assert_external_shape(external.blocks["Part"].query("ELLIPSE").first, SHAPES[1])
        assert len(external.modelspace().query("INSERT")) == 1
    out = tmp_path / "filter.dwg"
    result = ezdwg.to_dwg(
        ezdwg.read(str(resaved)), str(out), types="INSERT", strict=True
    )
    assert result.written_entities == 3  # root, nested reference, block ellipse
    imported = ezdwg.read(str(out)).edit()
    assert len(list(imported.blocks["Part"].query("ELLIPSE"))) == 1
    assert not list(imported.modelspace().query("ELLIPSE"))


@pytest.mark.parametrize("version", ["ACAD2000", "ACAD2013"])
def test_independent_ellipse_fixture_edit_and_export(oda_command, tmp_path, version):
    ezdxf = pytest.importorskip("ezdxf")
    source = ezdxf.new("R2000" if version == "ACAD2000" else "R2013")
    for center, axis, normal, ratio, start, end in SHAPES:
        source.modelspace().add_ellipse(
            center,
            axis,
            ratio,
            start,
            end,
            dxfattribs={"extrusion": normal, "color": 2},
        )
    dxf = tmp_path / "independent.dxf"
    source.saveas(dxf)
    dwg = convert(oda_command, dxf, tmp_path / "input", "DWG", version)
    drawing = ezdwg.read(str(dwg)).edit()
    assert len(drawing.modelspace()) == len(SHAPES)
    first = next(drawing.modelspace().query("ELLIPSE"))
    drawing.modelspace().update(first, axis_ratio=0.6)
    output = tmp_path / "native.dwg"
    drawing.save(output)
    external = ezdxf.readfile(convert(oda_command, output, tmp_path / "output", "DXF"))
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    expected = [(*SHAPES[0][:3], 0.6, *SHAPES[0][4:]), *SHAPES[1:]]
    actual = list(external.modelspace().query("ELLIPSE"))
    assert len(actual) == len(expected)
    for entity, shape in zip(actual, expected):
        assert_external_shape(entity, shape)
        assert entity.dxf.color == 2
