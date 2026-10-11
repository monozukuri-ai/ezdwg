from __future__ import annotations

import math

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export


def block_drawing():
    drawing = ezdwg.new()
    drawing.layers["部品"] = {"color_index": 2}
    block = drawing.blocks.new(
        "部品A", base_point=(1, 2, 3), description="Nested component"
    )
    block.add_line((1, 2, 3), (5, 2, 3), color_index=0, layer="0")
    block.add_attdef(
        "PART", "既定値", (1, 3, 3), height=1.5, prompt="部品番号", attribute_flags=8
    )
    block.add_attdef("FIXED", "定数", (1, 4, 3), height=1.5, attribute_flags=2)
    parent = drawing.blocks.new("Assembly", base_point=(2, 0, 0))
    nested = parent.add_blockref("部品A", (3, 4, 5), scale=(-2, 3, 1), rotation=35)
    parent.add_attrib(nested, "PART", "内側", (3, 5, 5), height=2, color_index=0)
    root = drawing.modelspace().add_blockref(
        "Assembly",
        (10, 20, 30),
        rotation=90,
        scale=(2, 2, 2),
        layer="部品",
        color_index=3,
    )
    other = drawing.modelspace().add_blockref("部品A", (20, 0, 0), extrusion=(0, 1, 0))
    drawing.modelspace().add_attrib(
        other,
        "PART",
        "外側",
        (20, 1, 0),
        height=2,
        extrusion=(0, 1, 0),
        attribute_flags=1,
    )
    drawing.blocks.new("Empty")
    return drawing, root.handle, nested.handle, other.handle


def test_block_nested_attributes_roundtrip(tmp_path):
    drawing, _root, nested, other = block_drawing()
    path = tmp_path / "blocks.dwg"
    result = drawing.save(path)
    assert result.written_entities == 8
    assert not result.diagnostics
    source = ezdwg.read(str(path))
    assert len(list(source.modelspace().query("INSERT"))) == 2
    assert not list(source.modelspace().query("LINE ATTDEF"))
    assert len(list(source.modelspace().query("ATTRIB"))) == 1
    assert len(ezdwg.raw.decode_attrib_entities(str(path))) == 2
    assert len(ezdwg.raw.decode_attdef_entities(str(path))) == 2
    blocks = {r["name"]: r for r in ezdwg.raw.decode_write_blocks(str(path))}
    assert blocks["部品A"]["base_point"] == (1, 2, 3)
    assert blocks["Assembly"]["base_point"] == (2, 0, 0)
    assert blocks["部品A"]["description"] == "Nested component"
    edit = source.edit()
    assert set(edit.blocks) == {"部品A", "Assembly", "Empty"}
    assert edit.blocks["assembly"].get(nested).dxf["xscale"] == -2
    assert edit.modelspace().get(other).dxf["extrusion"] == (0, 1, 0)
    assert edit.modelspace().attribs(other)[0].dxf["text"] == "外側"
    assert edit.blocks["Assembly"].attribs(nested)[0].dxf["text"] == "内側"
    edit.modelspace().update(other, rotation=45)
    edit.modelspace().update(edit.modelspace().attribs(other)[0], text="変更")
    path.unlink()
    edit.save(path)
    again = ezdwg.read(str(path)).edit()
    assert again.modelspace().get(other).dxf["rotation"] == pytest.approx(45)
    assert again.modelspace().attribs(other)[0].dxf["text"] == "変更"


@pytest.mark.parametrize(
    "case",
    [
        "undefined",
        "cycle",
        "duplicate_tag",
        "invalid_scale",
        "duplicate_handle",
        "orphan",
    ],
)
def test_invalid_block_graph_preserves_destination(tmp_path, case):
    doc = ezdwg.new()
    block = doc.blocks.new("A")
    parent = doc.modelspace().add_blockref("A")
    if case == "undefined":
        parent.dxf["name"] = "Missing"
    elif case == "cycle":
        other = doc.blocks.new("B")
        block.add_blockref("B")
        other.add_blockref("A")
    elif case == "duplicate_tag":
        doc.modelspace().add_attrib(parent, "TAG", "one")
        doc.modelspace().add_attrib(parent, "tag", "two")
    elif case == "invalid_scale":
        parent.dxf["xscale"] = 0
    elif case == "duplicate_handle":
        with pytest.raises(ValueError, match="duplicate"):
            block.add_entity("POINT", {"location": (0, 0)}, handle=parent.handle)
        return
    else:
        with pytest.raises(ValueError, match="attached"):
            block.add_entity(
                "ATTRIB", {"tag": "TAG", "text": "x", "insert": (0, 0), "height": 1}
            )
        return
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"original")
    with pytest.raises(ValueError):
        doc.save(path)
    assert path.read_bytes() == b"original"
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))


def test_block_handles_are_global_and_delete_cascades_attributes(tmp_path):
    doc, root, _nested, other = block_drawing()
    handles = [e.handle for layout in doc._layouts() for e in layout]
    assert len(handles) == len(set(handles))
    with pytest.raises(ValueError, match="referenced"):
        doc.blocks.delete("部品A")
    msp = doc.modelspace()
    attr = msp.attribs(other)[0]
    msp.delete(attr)
    assert not msp.attribs(other)
    msp.add_attrib(other, "PART", "again")
    msp.delete(other)
    assert other not in doc._attributes
    with pytest.raises(ValueError, match="does not belong"):
        msp.update(attr, text="detached")
    msp.delete(root)
    doc.blocks.delete("Assembly")
    doc.blocks.delete("部品A")
    assert set(doc.blocks) == {"Empty"}
    assert doc.save(tmp_path / "empty.dwg").written_entities == 0


def test_export_selection_includes_transitive_dependencies(tmp_path):
    doc, _root, nested, _other = block_drawing()
    path = tmp_path / "source.dwg"
    doc.save(path)
    out = tmp_path / "out.dwg"
    result = ezdwg.to_dwg(str(path), str(out), types="INSERT", strict=True)
    assert result.written_entities == 8
    copy = ezdwg.read(str(out)).edit()
    assert set(copy.blocks) == {"部品A", "Assembly"}  # only referenced blocks exported
    assert len(list(copy.modelspace())) == 2
    assert copy.blocks["Assembly"].attribs(nested)[0].dxf["text"] == "内側"


def test_invalid_insert_non_strict_skips_owned_attributes(tmp_path):
    drawing = ezdwg.new()
    drawing.blocks.new("Part").add_point((0, 0))
    msp = drawing.modelspace()
    insert = msp.add_blockref("Part")
    msp.add_attrib(insert, "TAG", "value")
    insert.dxf["xscale"] = 0
    result = drawing.save(tmp_path / "skipped.dwg", strict=False)
    assert result.skipped_by_type == {"ATTRIB": 1, "INSERT": 1}
    assert result.written_entities == 1  # retained block definition geometry
    assert not list(ezdwg.read(result.output_path).modelspace().query())


@pytest.mark.parametrize(
    "case", ["orphan", "unknown_owner", "duplicate_handle", "duplicate_block", "cycle"]
)
def test_raw_block_contract_rejects_bad_graph_before_replace(tmp_path, case):
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    kwargs = {"blocks": [{"name": "Part"}], "inserts": [{"handle": 10, "name": "Part"}]}
    if case in {"orphan", "unknown_owner", "duplicate_handle"}:
        handle = 10 if case == "duplicate_handle" else 11
        kwargs["attribs"] = [{"handle": handle, "tag": "TAG", "text": "x"}]
        if case != "orphan":
            kwargs["entity_owners"] = {handle: 99 if case == "unknown_owner" else 10}
    elif case == "duplicate_block":
        kwargs["blocks"].append({"name": "part"})
    else:
        kwargs["entity_owners"] = {10: "Part"}
    with pytest.raises(ValueError):
        ezdwg.raw.write_ac1015_dwg(str(path), [], [], [], [], [], [], **kwargs)
    assert path.read_bytes() == b"keep"


def test_anonymous_block_and_block_layout_export(tmp_path):
    doc = ezdwg.new()
    block = doc.blocks.new("*U1")
    point = block.add_point((1, 2))
    doc.modelspace().add_circle((0, 0), 2)
    doc.modelspace().add_blockref("*u1")
    path = tmp_path / "anonymous.dwg"
    doc.save(path)
    assert "*U1" in ezdwg.read(str(path)).edit().blocks
    out = tmp_path / "block-only.dwg"
    result = block.export_dwg(out)
    assert result.written_entities == 1
    entities = list(ezdwg.read(str(out)).modelspace().query())
    assert [(e.dxftype, e.handle) for e in entities] == [("POINT", point.handle)]


def test_nested_blocks_external_read_resave_and_import(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    from ezdxf.lldxf.encoding import decode_mif_to_unicode

    drawing, root, nested, other = block_drawing()
    source = tmp_path / "blocks.dwg"
    drawing.save(source)
    dxf = convert(oda_command, source, tmp_path / "dxf", "DXF")
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf_again = convert(oda_command, resaved, tmp_path / "dxf-again", "DXF")
    for path in (dxf, dxf_again):
        doc = ezdxf.readfile(path)
        audit = doc.audit()
        assert not audit.errors, [(e.code, e.message) for e in audit.errors]
        assert not audit.fixes, [(e.code, e.message) for e in audit.fixes]
        assert len(doc.modelspace()) == 2
        parts = {decode_mif_to_unicode(b.name): b for b in doc.blocks}
        assert tuple(parts["部品A"].base_point) == (1, 2, 3)
        assert len(parts["部品A"]) == 3
        assert len(parts["Assembly"]) == 1
        insert = doc.entitydb[f"{nested:X}"]
        assert (insert.dxf.xscale, insert.dxf.yscale, insert.dxf.zscale) == (-2, 3, 1)
        assert insert.dxf.rotation == pytest.approx(35)
        assert decode_mif_to_unicode(insert.attribs[0].dxf.text) == "内側"
        other_insert = doc.entitydb[f"{other:X}"]
        assert tuple(other_insert.dxf.extrusion) == (0, 1, 0)
        assert decode_mif_to_unicode(other_insert.attribs[0].dxf.text) == "外側"
        assert other_insert.attribs[0].dxf.flags == 1
        assert tuple(doc.entitydb[f"{root:X}"].dxf.insert) == (10, 20, 30)
        root_transform = doc.entitydb[f"{root:X}"].matrix44()
        nested_transform = insert.matrix44()
        assert tuple(
            root_transform.transform(nested_transform.transform((1, 2, 3)))
        ) == pytest.approx((2, 22, 40))
        assert tuple(
            root_transform.transform(nested_transform.transform((5, 2, 3)))
        ) == pytest.approx(
            (
                2 + 16 * math.sin(math.radians(35)),
                22 - 16 * math.cos(math.radians(35)),
                40,
            )
        )
    imported = ezdwg.read(str(resaved)).edit()
    assert imported.blocks["部品A"].base_point == (1, 2, 3)
    assert imported.modelspace().attribs(other)[0].dxf["text"] == "外側"


@pytest.mark.parametrize("source_version", ["ACAD2000", "ACAD2013"])
def test_independent_block_fixture_import_and_native_export(
    oda_command, tmp_path, source_version
):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdxf.new("R2000" if source_version == "ACAD2000" else "R2013")
    child = doc.blocks.new("Component", base_point=(4, 5, 6))
    child.add_circle((4, 5, 6), 2)
    child.add_attdef(
        "ID", (5, 6, 6), text="default", height=1, dxfattribs={"prompt": "Identifier"}
    )
    parent = doc.blocks.new("Assembly", base_point=(1, 2, 0))
    nested = parent.add_blockref(
        "Component",
        (10, 20, 30),
        dxfattribs={"xscale": -2, "yscale": 3, "rotation": 25},
    )
    nested.add_attrib("ID", "NESTED", (11, 21, 30), dxfattribs={"height": 1.5})
    doc.modelspace().add_blockref(
        "Assembly", (100, 200, 0), dxfattribs={"rotation": 45}
    )
    path = tmp_path / "external.dxf"
    doc.saveas(path)
    dwg = convert(oda_command, path, tmp_path / "input", "DWG", source_version)
    drawing = ezdwg.read(str(dwg)).edit()
    assert drawing.blocks["Component"].base_point == (4, 5, 6)
    blockref = next(drawing.blocks["Assembly"].query("INSERT"))
    attr = drawing.blocks["Assembly"].attribs(blockref)[0]
    assert attr.dxf["text"] == "NESTED"
    drawing.blocks["Assembly"].update(attr, text="EDITED")
    output = tmp_path / "native.dwg"
    drawing.save(output)
    exported = ezdxf.readfile(convert(oda_command, output, tmp_path / "output", "DXF"))
    audit = exported.audit()
    assert not audit.errors and not audit.fixes
    assert tuple(exported.blocks["Component"].base_point) == (4, 5, 6)
    assert (
        exported.blocks["Assembly"].query("INSERT").first.attribs[0].dxf.text
        == "EDITED"
    )


def test_strict_checks_unsupported_geometry_inside_referenced_block(
    oda_command, tmp_path
):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdxf.new("R2000")
    block = doc.blocks.new("Part")
    block.add_line((0, 0), (1, 1))
    block.add_leader([(0, 0, 0), (1, 2, 0), (3, 1, 0)])
    doc.modelspace().add_blockref("Part", (10, 20))
    source = tmp_path / "source.dxf"
    doc.saveas(source)
    dwg = convert(oda_command, source, tmp_path / "input", "DWG")
    out = tmp_path / "out.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError, match="LEADER"):
        ezdwg.to_dwg(str(dwg), str(out), types="INSERT", strict=True)
    assert out.read_bytes() == b"keep"
    result = ezdwg.to_dwg(str(dwg), str(out), types="INSERT")
    assert result.skipped_by_type == {"LEADER": 1}
    assert result.written_entities == 2
    saved = ezdwg.read(str(out)).edit()
    assert len(saved.modelspace()) == 1
    assert len(saved.blocks["Part"]) == 1


def test_many_block_references_and_sparse_attribute_chain(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    path = tmp_path / "many.dwg"
    attrs = [
        {"handle": h, "tag": tag, "text": tag, "insert": (float(i), 0.0, 0.0)}
        for i, (h, tag) in enumerate([(100, "A"), (300, "B"), (200, "C")])
    ]
    ezdwg.raw.write_ac1015_dwg(
        str(path),
        [(1, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0)],
        [],
        [],
        [],
        [],
        [],
        blocks=[{"name": "Part"}],
        inserts=[{"handle": 10, "name": "Part"}]
        + [
            {"handle": 1000 + i, "name": "Part", "insert": (float(i), 1.0, 0.0)}
            for i in range(300)
        ],
        attribs=attrs,
        entity_owners={1: "Part", 100: 10, 300: 10, 200: 10},
    )
    doc = ezdxf.readfile(convert(oda_command, path, tmp_path / "dxf", "DXF"))
    audit = doc.audit()
    assert not audit.errors and not audit.fixes
    assert len(doc.modelspace()) == 301
    assert [a.dxf.tag for a in doc.entitydb["A"].attribs] == ["A", "B", "C"]
    assert len(doc.blocks["Part"]) == 1
    drawing = ezdwg.read(str(path)).edit()
    assert [a.dxf["tag"] for a in drawing.modelspace().attribs(10)] == ["A", "B", "C"]
    saved = tmp_path / "edited.dwg"
    drawing.save(saved)
    edited = ezdxf.readfile(convert(oda_command, saved, tmp_path / "edited-dxf", "DXF"))
    assert [a.dxf.tag for a in edited.entitydb["A"].attribs] == ["A", "B", "C"]


def test_missing_owned_attribute_is_diagnosed(tmp_path, monkeypatch):
    from ezdwg import convert as conversion

    drawing, _, _, other = block_drawing()
    source = tmp_path / "source.dwg"
    missing = drawing.modelspace().attribs(other)[0].handle
    drawing.save(source)
    resolve = conversion._resolve_dwg_export_entities

    def omit_attribute(layout, types):
        return [e for e in resolve(layout, types) if e.handle != missing]

    monkeypatch.setattr(conversion, "_resolve_dwg_export_entities", omit_attribute)
    doc = ezdwg.read(str(source))
    with pytest.raises(ValueError, match=r"INSERT.attributes"):
        doc.edit()
    partial = doc.edit(strict=False)
    assert any(
        d.handle == other and d.attribute == "attributes" for d in partial.diagnostics
    )
    out = tmp_path / "out.dwg"
    out.write_bytes(b"keep")
    with pytest.raises(ValueError, match="attributes"):
        ezdwg.to_dwg(doc, str(out), strict=True)
    assert out.read_bytes() == b"keep"
