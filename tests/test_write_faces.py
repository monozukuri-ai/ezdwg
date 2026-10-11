from __future__ import annotations

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export

KINDS = ("SOLID", "TRACE", "3DFACE")
PLANAR = [(1.0, 2.0, 5.0), (7.0, 3.0, 5.0), (2.0, 8.0, 5.0), (6.0, 9.0, 5.0)]
SPATIAL = [(1.0, 2.0, 0.0), (7.0, 3.0, 4.0), (6.0, 9.0, -3.0), (2.0, 8.0, 2.0)]
NORMAL = (0.0, 0.6, 0.8)
DECODERS = {
    "SOLID": ezdwg.raw.decode_solid_entities,
    "TRACE": ezdwg.raw.decode_trace_entities,
    "3DFACE": ezdwg.raw.decode_3dface_entities,
}
RAW_KEYS = {"SOLID": "solids", "TRACE": "traces", "3DFACE": "faces3d"}


def add(layout, kind, points=None, **attributes):
    method = {"SOLID": "add_solid", "TRACE": "add_trace", "3DFACE": "add_3dface"}[kind]
    return getattr(layout, method)(
        (SPATIAL if kind == "3DFACE" else PLANAR) if points is None else points,
        **attributes,
    )


def raw_write(path, kind, points, **attributes):
    tail = (
        (attributes.pop("invisible_edge_flags", 0),)
        if kind == "3DFACE"
        else (
            attributes.pop("thickness", 0),
            attributes.pop("extrusion", (0, 0, 1)),
        )
    )
    ezdwg.raw.write_ac1015_dwg(
        str(path),
        [],
        [],
        [],
        [],
        [],
        [],
        **{RAW_KEYS[kind]: [(100, *points, *tail)]},
        entity_attributes={100: attributes},
    )


@pytest.mark.parametrize("kind", KINDS)
def test_face_new_edit_save_and_raw_geometry(tmp_path, kind):
    drawing = ezdwg.new()
    drawing.layers["面"] = {"color_index": 3}
    drawing.linetypes["DASH"] = {"dashes": [0.5, -0.25]}
    attrs = (
        {"invisible_edge_flags": 9}
        if kind == "3DFACE"
        else {
            "thickness": -2,
            "extrusion": NORMAL,
        }
    )
    face = add(
        drawing.modelspace(),
        kind,
        **attrs,
        layer="面",
        color_index=2,
        linetype="DASH",
        linetype_scale=2,
        lineweight=35,
        invisible=True,
    )
    path = tmp_path / "face.dwg"
    result = drawing.save(path)
    assert result.written_entities == 1 and not result.diagnostics
    (row,) = DECODERS[kind](str(path))
    expected = SPATIAL if kind == "3DFACE" else PLANAR
    assert list(row[1:5]) == expected
    assert row[5:] == ((9,) if kind == "3DFACE" else (-2, NORMAL))
    source = ezdwg.read(str(path))
    (entity,) = source.modelspace().query(kind)
    assert entity.handle == face.handle
    assert entity.dxf["points"] == expected
    assert entity.dxf["color_index"] == 2
    assert entity.dxf["linetype"] == "DASH"
    assert entity.dxf["linetype_scale"] == 2
    assert entity.dxf["lineweight"] == 35 and entity.dxf["invisible"]
    edited = source.edit()
    edited.modelspace().update(face.handle, points=expected[:3])
    path.unlink()
    edited.save(path)
    (row,) = DECODERS[kind](str(path))
    assert list(row[1:5]) == [*expected[:3], expected[2]]
    result = ezdwg.to_dwg(
        str(path), str(tmp_path / "filtered.dwg"), types=kind, strict=True
    )
    assert result.written_entities == 1 and not result.diagnostics


INVALID = [
    ("SOLID", PLANAR[:2], {}),
    ("TRACE", PLANAR + [PLANAR[0]], {}),
    ("SOLID", [(0, 0), *PLANAR[1:]], {}),
    ("TRACE", [(0, 0, 0), *PLANAR[1:]], {}),
    ("SOLID", [(float("nan"), 0, 5), *PLANAR[1:]], {}),
    ("TRACE", PLANAR, {"extrusion": (0, 0, 0)}),
    ("SOLID", PLANAR, {"thickness": float("inf")}),
    ("3DFACE", SPATIAL, {"invisible_edge_flags": 16}),
    ("3DFACE", SPATIAL, {"invisible_edge_flags": -1}),
    ("3DFACE", SPATIAL, {"invisible_edge_flags": 1.5}),
    ("3DFACE", [(0, 0, float("inf")), *SPATIAL[1:]], {}),
    ("3DFACE", SPATIAL, {"thickness": 2}),
    ("3DFACE", SPATIAL, {"extrusion": (0, 1, 0)}),
    ("SOLID", PLANAR, {"elevation": 2}),
]


@pytest.mark.parametrize("kind,points,attrs", INVALID)
def test_invalid_faces_rejected_before_replacing_destination(
    tmp_path, kind, points, attrs
):
    msp = ezdwg.new().modelspace()
    face = add(msp, kind)
    original = dict(face.dxf)
    with pytest.raises((ValueError, TypeError)):
        msp.update(face, points=points, **attrs)
    assert face.dxf == original
    with pytest.raises((ValueError, TypeError)):
        add(msp, kind, points, **attrs)
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    with pytest.raises((ValueError, TypeError, OverflowError)):
        raw_write(path, kind, points, **attrs)
    assert path.read_bytes() == b"keep"


@pytest.mark.parametrize("kind", KINDS)
def test_raw_face_formatting_survives_partial_attribute_dictionary(tmp_path, kind):
    path = tmp_path / "raw.dwg"
    if kind == "3DFACE":
        raw_write(path, kind, SPATIAL, invisible_edge_flags=6, color_index=2)
        assert DECODERS[kind](str(path))[0][5:] == (6,)
    else:
        raw_write(path, kind, PLANAR, thickness=-2, extrusion=NORMAL, color_index=2)
        assert DECODERS[kind](str(path))[0][5:] == (-2, NORMAL)
    assert next(ezdwg.read(str(path)).modelspace().query(kind)).dxf["color_index"] == 2


@pytest.mark.parametrize("kind", KINDS)
def test_direct_face_mutations_strict_and_non_strict(tmp_path, kind):
    doc = ezdwg.new()
    face = add(doc.modelspace(), kind)
    doc.modelspace().add_point((0, 0))
    face.dxf["points"] = [(0, 0, 0)]
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    with pytest.raises(ValueError):
        doc.save(path)
    assert path.read_bytes() == b"keep"
    result = doc.save(path, strict=False)
    assert result.skipped_by_type == {kind: 1}
    assert result.written_entities == 1
    assert any(
        d.handle == face.handle and d.attribute == "geometry"
        for d in result.diagnostics
    )


def assert_external(entity, points, *, flags=None, thickness=0, normal=(0, 0, 1)):
    corners = [*points, points[-1]] if len(points) == 3 else points
    for i, p in enumerate(corners):
        assert tuple(entity.dxf.get(f"vtx{i}")) == pytest.approx(p)
    if entity.dxftype() == "3DFACE":
        assert entity.dxf.invisible_edges == flags
    else:
        assert entity.dxf.thickness == pytest.approx(thickness)
        assert tuple(entity.dxf.extrusion) == pytest.approx(normal)


def test_faces_oda_read_resave_all_flags_and_block_selection(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    for kind in ("SOLID", "TRACE"):
        add(doc.modelspace(), kind, thickness=-2, extrusion=NORMAL, color_index=2)
        add(doc.modelspace(), kind, PLANAR[:3], extrusion=(0, 0, -1))
    for flags in range(16):
        points = SPATIAL[:3] if flags % 2 else SPATIAL
        add(doc.modelspace(), "3DFACE", points, invisible_edge_flags=flags)
    part = doc.blocks.new("Part", base_point=(1, 2, 3))
    for kind in KINDS:
        add(part, kind)
    nested = doc.blocks.new("Nested")
    nested.add_blockref("Part", (4, 5), scale=(-2, 3, 1), rotation=30)
    doc.modelspace().add_blockref("Nested", (100, 200))
    path = tmp_path / "faces.dwg"
    doc.save(path)
    direct = convert(oda_command, path, tmp_path / "direct", "DXF")
    resaved = convert(oda_command, path, tmp_path / "resaved", "DWG")
    again = convert(oda_command, resaved, tmp_path / "again", "DXF")
    for dxf in (direct, again):
        external = ezdxf.readfile(dxf)
        audit = external.audit()
        assert not audit.errors and not audit.fixes
        msp = external.modelspace()
        assert len(msp) == 21
        for kind in ("SOLID", "TRACE"):
            a, b = msp.query(kind)
            assert_external(a, PLANAR, thickness=-2, normal=NORMAL)
            assert a.dxf.color == 2
            assert_external(b, PLANAR[:3], normal=(0, 0, -1))
        for flags, face in enumerate(msp.query("3DFACE")):
            assert_external(face, SPATIAL[:3] if flags % 2 else SPATIAL, flags=flags)
        assert len(external.blocks["Part"]) == 3
        for kind in KINDS:
            assert_external(
                external.blocks["Part"].query(kind).first,
                SPATIAL if kind == "3DFACE" else PLANAR,
                flags=0,
            )
    output = tmp_path / "filtered.dwg"
    result = ezdwg.to_dwg(str(resaved), str(output), types="INSERT", strict=True)
    assert result.written_entities == 5
    edited = ezdwg.read(str(output)).edit()
    assert len(edited.blocks["Part"]) == 3
    assert len(edited.modelspace()) == 1


@pytest.mark.parametrize("version", ["ACAD2000", "ACAD2004", "ACAD2013"])
def test_independent_faces_edit_and_native_export(oda_command, tmp_path, version):
    ezdxf = pytest.importorskip("ezdxf")
    source = ezdxf.new(
        {"ACAD2000": "R2000", "ACAD2004": "R2004", "ACAD2013": "R2013"}[version]
    )
    source.modelspace().add_solid(
        PLANAR, dxfattribs={"thickness": 2, "extrusion": NORMAL}
    )
    source.modelspace().add_trace(
        PLANAR[:3], dxfattribs={"thickness": -2, "extrusion": (0, 0, -1)}
    )
    source.modelspace().add_3dface(SPATIAL, dxfattribs={"invisible_edges": 10})
    path = tmp_path / "independent.dxf"
    source.saveas(path)
    dwg = convert(oda_command, path, tmp_path / "input", "DWG", version)
    doc = ezdwg.read(str(dwg)).edit()
    assert len(doc.modelspace()) == 3
    for kind in KINDS:
        (entity,) = doc.modelspace().query(kind)
        doc.modelspace().update(entity, color_index=4)
        if kind == "3DFACE":
            doc.modelspace().update(entity, invisible_edge_flags=5)
        elif kind == "SOLID":
            doc.modelspace().update(entity, thickness=3)
    output = tmp_path / "native.dwg"
    doc.save(output)
    external = ezdxf.readfile(convert(oda_command, output, tmp_path / "output", "DXF"))
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    msp = external.modelspace()
    assert len(msp) == 3
    assert_external(msp.query("SOLID").first, PLANAR, thickness=3, normal=NORMAL)
    assert_external(
        msp.query("TRACE").first, PLANAR[:3], thickness=-2, normal=(0, 0, -1)
    )
    assert_external(msp.query("3DFACE").first, SPATIAL, flags=5)
    assert all(e.dxf.color == 4 for e in msp)
