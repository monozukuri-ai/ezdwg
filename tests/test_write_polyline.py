from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export

XY = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]
XYZ = [(0.0, 0.0, 1.0), (2.0, 3.0, 4.0), (5.0, 6.0, -2.0)]
WIDTHS = [(0.1, 0.2), (0.3, 0.4), (0.5, 0.6)]
BULGES = [0.0, 0.5, -0.25]


def drawing():
    doc = ezdwg.new()
    doc.layers["経路"] = {"color_index": 3}
    doc.linetypes["DASH"] = {"dashes": [0.5, -0.25]}
    msp = doc.modelspace()
    a = msp.add_polyline2d(
        XY,
        closed=True,
        flags=128,
        bulges=BULGES,
        widths=WIDTHS,
        width_start=0.7,
        width_end=0.8,
        elevation=4,
        thickness=2,
        extrusion=(0, 1, 0),
        color_index=2,
        layer="経路",
        linetype="DASH",
    )
    b = msp.add_polyline3d(XYZ, closed=True, layer="経路", lineweight=35)
    msp.add_point((9, 9))
    block = doc.blocks.new("Part")
    c = block.add_polyline2d(XY, bulges=BULGES)
    block.add_polyline3d(XYZ)
    msp.add_blockref("Part", (10, 20))
    return doc, a.handle, b.handle, c.handle


def test_legacy_polylines_native_roundtrip_edit_and_filter(tmp_path):
    doc, a, b, c = drawing()
    path = tmp_path / "legacy.dwg"
    result = doc.save(path)
    assert result.written_entities == 6 and not result.diagnostics
    metadata = {r["handle"]: r for r in ezdwg.raw.decode_write_polylines(str(path))}
    assert len(metadata) == 4
    assert all(r["complete"] and not r["losses"] for r in metadata.values())
    assert metadata[a]["flags"] == 129
    assert metadata[b]["flags"] == 9
    assert metadata[a]["vertices"] == [
        (p + (0.0,), *w, bulge) for p, w, bulge in zip(XY, WIDTHS, BULGES)
    ]
    assert [v[0] for v in metadata[b]["vertices"]] == XYZ
    source = ezdwg.read(str(path))
    entities = {
        e.handle: e for e in source.modelspace().query("POLYLINE_2D POLYLINE_3D")
    }
    assert set(entities) == {a, b}
    assert entities[a].dxf["bulges"] == BULGES
    assert entities[a].dxf["widths"] == WIDTHS  # last vertex is not a duplicate closure
    assert entities[a].dxf["elevation"] == 4
    assert entities[a].dxf["extrusion"] == (0, 1, 0)
    assert entities[a].dxf["color_index"] == 2
    assert entities[b].dxf["vertex_handles"] == metadata[b]["vertex_handles"]
    assert entities[b].dxf["seqend_handle"] == metadata[b]["seqend_handle"]
    edited = source.edit()
    assert edited.modelspace().get(b).dxf["points"] == XYZ
    edited.modelspace().update(
        a, points=[*XY, (0.0, 3.0)], closed=False, width_start=1.2
    )
    edited.blocks["Part"].delete(c)
    path.unlink()
    edited.save(path)
    out = ezdwg.read(str(path)).edit()
    assert out.modelspace().get(a).dxf["points"] == [*XY, (0.0, 3.0)]
    assert out.modelspace().get(a).dxf["width_start"] == 1.2
    assert not out.modelspace().get(a).dxf["closed"]
    assert len(out.blocks["Part"]) == 1
    filtered = tmp_path / "filtered.dwg"
    result = ezdwg.to_dwg(
        source=out, output_path=str(filtered), types="POLYLINE_3D", strict=True
    )
    assert result.written_entities == 1
    assert len(ezdwg.raw.decode_polyline_3d_with_vertices(str(filtered))) == 1


@pytest.mark.parametrize(
    "is_3d, attributes",
    [
        (False, {"points": []}),
        (True, {"points": [(0, 0, 0)]}),
        (False, {"points": [(0, 0, 0), (1, 1, 0)]}),
        (True, {"points": [(0, 0), (1, 1)]}),
        (False, {"flags": 4}),
        (True, {"flags": 16}),
        (False, {"bulges": [0, 0, 0, 1]}),
        (False, {"widths": [(-1, 2)]}),
        (False, {"width_start": -1}),
        (False, {"points": [(0, 0), (float("nan"), 1)]}),
        (True, {"points": [(0, 0, 0), (1, float("inf"), 1)]}),
    ],
)
def test_raw_and_editable_invalid_polylines_leave_destination(
    tmp_path, is_3d, attributes
):
    data = {"points": XYZ if is_3d else XY, **attributes}
    doc = ezdwg.new()
    with pytest.raises(ValueError):
        doc.modelspace().add_entity("POLYLINE_3D" if is_3d else "POLYLINE_2D", data)
    out = tmp_path / "keep.dwg"
    out.write_bytes(b"keep")
    with pytest.raises((ValueError, TypeError)):
        ezdwg.raw.write_ac1015_dwg(
            str(out),
            [],
            [],
            [],
            [],
            [],
            [],
            **{"polylines3d" if is_3d else "polylines2d": [data]},
        )
    assert out.read_bytes() == b"keep"


def test_invalid_direct_mutation_is_skipped_as_one_polyline(tmp_path):
    doc = ezdwg.new()
    poly = doc.modelspace().add_polyline2d(XY)
    poly.dxf["bulges"] = [0, 1, 2, 3]
    doc.modelspace().add_point((1, 2))
    out = tmp_path / "keep.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError, match="POLYLINE_2D"):
        doc.save(out)
    assert out.read_bytes() == b"keep"
    result = doc.save(out, strict=False)
    assert result.skipped_by_type == {"POLYLINE_2D": 1}
    assert result.total_entities == 2 and result.written_entities == 1
    assert not ezdwg.raw.decode_write_polylines(str(out))


def test_native_polyline_oda_read_resave(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc, a, b, _ = drawing()
    path = tmp_path / "polylines.dwg"
    doc.save(path)
    direct = convert(oda_command, path, tmp_path / "dxf", "DXF")
    resaved = convert(oda_command, path, tmp_path / "resaved", "DWG")
    again = convert(oda_command, resaved, tmp_path / "again", "DXF")
    for output in (direct, again):
        external = ezdxf.readfile(output)
        audit = external.audit()
        assert not audit.errors and not audit.fixes
        assert len(external.modelspace()) == 4
        p2, p3 = external.entitydb[f"{a:X}"], external.entitydb[f"{b:X}"]
        assert p2.dxftype() == "POLYLINE" and p2.is_2d_polyline
        assert p3.dxftype() == "POLYLINE" and p3.is_3d_polyline
        assert p2.is_closed and p3.is_closed
        assert p2.dxf.flags == 129
        assert tuple(p2.dxf.elevation) == (0, 0, 4)
        assert tuple(p2.dxf.extrusion) == (0, 1, 0)
        assert p2.dxf.thickness == 2
        assert p2.dxf.default_start_width == 0.7
        assert p2.dxf.default_end_width == 0.8
        assert p2.dxf.color == 2 and p2.dxf.linetype == "DASH"
        assert [tuple(v.dxf.location) for v in p2.vertices] == [p + (4.0,) for p in XY]
        assert [tuple(p) for p in p2.points_in_wcs()] == [(-x, 4.0, y) for x, y in XY]
        assert [v.dxf.bulge for v in p2.vertices] == BULGES
        assert [(v.dxf.start_width, v.dxf.end_width) for v in p2.vertices] == WIDTHS
        assert [tuple(v.dxf.location) for v in p3.vertices] == XYZ
        assert len(external.blocks["Part"].query("POLYLINE")) == 2
    imported = ezdwg.read(str(resaved)).edit()
    assert imported.modelspace().get(a).dxf["bulges"] == BULGES


@pytest.mark.parametrize("version", ["ACAD2000", "ACAD2004", "ACAD2013"])
def test_independent_polyline_input_edit_and_export(oda_command, tmp_path, version):
    ezdxf = pytest.importorskip("ezdxf")
    external = ezdxf.new("R2013")
    p2 = external.modelspace().add_polyline2d(
        XY,
        close=True,
        dxfattribs={"elevation": (0, 0, 4), "extrusion": (0, 1, 0), "thickness": 2},
    )
    for vertex, bulge, width in zip(p2.vertices, BULGES, WIDTHS):
        vertex.dxf.bulge = bulge
        vertex.dxf.start_width, vertex.dxf.end_width = width
    external.modelspace().add_polyline3d(XYZ, close=True)
    # Preserve topology order when numeric handles are sparse and non-monotonic.
    for vertex, handle in zip(p2.vertices, ["1000", "3000", "2000"]):
        assert external.entitydb.reset_handle(vertex, handle)
    src = tmp_path / "independent.dxf"
    external.saveas(src)
    path = convert(oda_command, src, tmp_path / "input", "DWG", version)
    source_doc = ezdwg.read(str(path))
    source_two = next(source_doc.modelspace().query("POLYLINE_2D"))
    assert source_two.dxf["vertex_handles"] == [0x1000, 0x3000, 0x2000]
    assert source_two.dxf["points"] == [p + (4.,) for p in XY]
    assert source_two.dxf["bulges"] == BULGES
    doc = source_doc.edit()
    two = list(doc.modelspace().query("POLYLINE_2D"))
    assert len(two) == 1
    assert two[0].dxf["points"] == XY
    assert two[0].dxf["bulges"] == BULGES
    assert len(list(doc.modelspace().query("POLYLINE_3D"))) == 1
    doc.modelspace().update(two[0], elevation=7, closed=False)
    out = tmp_path / "edited.dwg"
    doc.save(out)
    result = ezdxf.readfile(convert(oda_command, out, tmp_path / "output", "DXF"))
    audit = result.audit()
    assert not audit.errors and not audit.fixes
    polys = list(result.modelspace().query("POLYLINE"))
    assert len(polys) == 2
    poly = next(p for p in polys if p.is_2d_polyline)
    assert poly.dxf.elevation.z == 7 and not poly.is_closed
    assert [v.dxf.bulge for v in poly.vertices] == BULGES
    assert [(v.dxf.start_width, v.dxf.end_width) for v in poly.vertices] == WIDTHS
    assert [
        tuple(v.dxf.location)
        for v in next(p for p in polys if p.is_3d_polyline).vertices
    ] == XYZ


@pytest.mark.parametrize("loss", ["color", "xdata", "vertex_id", "fit"])
def test_independent_vertex_losses_fail_strict(oda_command, tmp_path, loss):
    ezdxf = pytest.importorskip("ezdxf")
    external = ezdxf.new("R2013")
    poly = external.modelspace().add_polyline2d(XY)
    if loss == "color":
        poly.vertices[1].dxf.color = 2
    elif loss == "xdata":
        external.appids.new("CUSTOM")
        poly.vertices[1].set_xdata("CUSTOM", [(1000, "keep")])
    elif loss == "vertex_id":
        poly.vertices[1].dxf.vertex_identifier = 77
    else:
        poly.dxf.flags |= 4
        poly.dxf.smooth_type = 6
        poly.vertices[1].dxf.flags = 16
    src = tmp_path / "loss.dxf"
    external.saveas(src)
    dwg = convert(oda_command, src, tmp_path / "input", "DWG", "ACAD2013")
    out = tmp_path / "keep.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError):
        ezdwg.to_dwg(str(dwg), str(out), strict=True)
    assert out.read_bytes() == b"keep"
    with pytest.raises(ValueError):
        ezdwg.read(str(dwg)).edit()
    result = ezdwg.to_dwg(str(dwg), str(out), strict=False)
    assert result.diagnostics
    if loss == "fit":
        assert result.skipped_by_type == {"POLYLINE_2D": 1}


def test_existing_3d_polyline_fixture(tmp_path):
    source = (
        Path(__file__).resolve().parents[1] / "test_dwg/acadsharp/sample_AC1015.dwg"
    )
    out = tmp_path / "fixture.dwg"
    result = ezdwg.to_dwg(str(source), str(out), types="POLYLINE_3D", strict=True)
    assert result.written_entities == 1
    assert len(ezdwg.raw.decode_write_polylines(str(out))) == 1


@pytest.mark.parametrize("missing", [False, True])
def test_undecoded_or_inconsistent_polyline_references_fail_closed(
    tmp_path, monkeypatch, missing
):
    doc = ezdwg.new()
    doc.modelspace().add_polyline2d(XY)
    path = tmp_path / "source.dwg"
    doc.save(path)
    original = ezdwg.raw.decode_write_polylines

    def broken(path):
        rows = original(path)
        if missing:
            return []
        rows[0]["complete"] = False
        return rows

    monkeypatch.setattr(ezdwg.raw, "decode_write_polylines", broken)
    output = tmp_path / "out.dwg"
    output.write_bytes(b"keep")
    with pytest.raises(ValueError):
        ezdwg.to_dwg(str(path), str(output), strict=True)
    assert output.read_bytes() == b"keep"
    with pytest.raises(ValueError):
        ezdwg.read(str(path)).edit()
    result = ezdwg.to_dwg(str(path), str(output), strict=False)
    assert result.skipped_by_type == {"POLYLINE_2D": 1}
    assert result.written_entities == 0
