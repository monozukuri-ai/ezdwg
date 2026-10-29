"""Native AC1032 RGB/Unicode writing and independent ODA interoperability."""

from collections import Counter

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command

TEXT = "日本語 한국어 Русский 中文 العربية 😀"
LONG_TEXT = TEXT.replace("😀", "字") * 400
LAYER = "図面_도면_Чертёж"


def drawing():
    doc = ezdwg.new("AC1032")
    doc.header["insunits"] = 4
    doc.layers[LAYER] = {"true_color": 0x73B9FF, "lineweight": 35}
    doc.text_styles["字体_글꼴"] = {"font": "txt"}
    doc.linetypes["破線_штрих"] = {"description": TEXT, "dashes": [1, -0.5]}
    msp = doc.modelspace()
    msp.add_line(
        (0, 0, 3),
        (10, 20, 6),
        layer=LAYER,
        true_color=0x123456,
        linetype="破線_штрих",
        linetype_scale=2,
        lineweight=35,
    )
    msp.add_circle((5, 6, 7), 2, true_color=0x000000)
    msp.add_arc((3, 4), 2, 30, 150, true_color=0xFFFFFF)
    msp.add_point((7, 8, 9), true_color=0xABCDEF)
    msp.add_ray((1, 2), (1, 0, 0), true_color=0x010203)
    msp.add_xline((2, 3), (0, 1, 0), true_color=0xFEDCBA)
    msp.add_ellipse((4, 5, 6), (3, 4, 0), 0.35, true_color=0x654321)
    msp.add_lwpolyline(
        [(0, 0), (2, 0), (2, 1)], closed=True, bulges=[0, 0.5, 0], true_color=0x13579B
    )
    msp.add_text(
        TEXT, (1, 2), height=2, style="字体_글꼴", layer=LAYER, true_color=0x2468AC
    )
    msp.add_mtext(
        TEXT + "\\P二行目",
        (3, 4),
        char_height=2,
        rect_width=20,
        layer=LAYER,
        true_color=0xA1B2C3,
    )
    return doc


def test_rgb_and_mixed_unicode_roundtrip_and_edit(tmp_path):
    source = tmp_path / "unicode.dwg"
    result = drawing().save(source)
    assert source.read_bytes()[:6] == b"AC1032"
    assert result.written_entities == 10
    assert not result.diagnostics
    doc = ezdwg.read(str(source))
    assert doc.header_variables()["insunits"] == 4
    assert next(doc.modelspace().query("TEXT")).dxf["text"] == TEXT
    assert next(doc.modelspace().query("MTEXT")).dxf["raw_text"] == TEXT + "\\P二行目"
    assert next(doc.modelspace().query("LINE")).dxf["true_color"] == 0x123456
    assert doc.layers.get(LAYER).true_color == 0x73B9FF
    assert {e.dxftype: e.dxf["true_color"] for e in doc.modelspace().query()} == {
        e.dxftype: e.dxf["true_color"] for e in drawing().modelspace()
    }
    edited = doc.edit(version="AC1032")
    assert not edited.diagnostics
    saved = tmp_path / "edited.dwg"
    assert not edited.save(saved).diagnostics
    reread = ezdwg.read(str(saved))
    assert next(reread.modelspace().query("TEXT")).dxf["text"] == TEXT
    assert next(reread.modelspace().query("LINE")).dxf["true_color"] == 0x123456
    assert reread.layers.get(LAYER).true_color == 0x73B9FF


def test_direct_document_export_keeps_colors_and_text(tmp_path):
    source, target = tmp_path / "source.dwg", tmp_path / "target.dwg"
    drawing().save(source)
    result = ezdwg.to_dwg(str(source), str(target), version="AC1032", strict=True)
    assert not result.diagnostics
    text = next(ezdwg.read(str(target)).modelspace().query("TEXT"))
    assert text.dxf["text"] == TEXT
    assert text.dxf["true_color"] == 0x2468AC


@pytest.mark.parametrize("count", [0, 1, 600])
def test_native_ac1032_external_read_and_resave(oda_command, tmp_path, count):
    ezdxf = pytest.importorskip("ezdxf")
    doc = drawing() if count == 1 else ezdwg.new("AC1032")
    if count == 600:
        for i in range(count):
            doc.modelspace().add_line((i, 0), (i, 10), true_color=0x123456 + i)
        # Crosses both the extended string-size trailer and data-page boundaries.
        doc.modelspace().add_mtext(LONG_TEXT, char_height=2, rect_width=100)
    source = tmp_path / "native.dwg"
    assert not doc.save(source).diagnostics
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG", "ACAD2018")
    external = ezdxf.readfile(
        convert(oda_command, resaved, tmp_path / "dxf", "DXF", "ACAD2018")
    )
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    msp = external.modelspace()
    if count == 1:
        assert Counter(e.dxftype() for e in msp) == Counter(
            e.dxftype for e in doc.modelspace()
        )
        assert msp.query("TEXT").first.dxf.text == TEXT
        assert msp.query("MTEXT").first.text == TEXT + "\\P二行目"
        assert msp.query("LINE").first.dxf.true_color == 0x123456
        assert tuple(msp.query("LINE").first.dxf.end) == (10, 20, 6)
        assert msp.query("LINE").first.dxf.linetype == "破線_штрих"
        assert external.layers.get(LAYER).dxf.true_color == 0x73B9FF
        assert external.header["$INSUNITS"] == 4
        assert {e.dxftype(): e.dxf.true_color for e in msp} == {
            e.dxftype: e.dxf["true_color"] for e in doc.modelspace()
        }
    elif count == 600:
        assert len(msp.query("LINE")) == 600
        assert msp.query("MTEXT").first.text == LONG_TEXT
        assert sorted(e.dxf.true_color for e in msp.query("LINE")) == list(
            range(0x123456, 0x123456 + 600)
        )
    else:
        assert len(msp) == 0


def test_unicode_static_blocks_with_attributes(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new("AC1032")
    block = doc.blocks.new("部品_деталь", description=TEXT)
    block.add_line((0, 0), (2, 3), true_color=0x123456)
    block.add_attdef("ID", TEXT, (0, 1), height=2, prompt="識別_식별")
    parent = doc.blocks.new("組立_조립")
    parent.add_blockref(block.name, (1, 2))
    root = doc.modelspace().add_blockref(parent.name, (10, 20), true_color=0xFFFFFF)
    doc.modelspace().add_attrib(
        root, "ID", TEXT, (10, 21), height=2, true_color=0x010203
    )
    source = tmp_path / "blocks.dwg"
    assert not doc.save(source).diagnostics
    assert ezdwg.raw.decode_attdef_entities(str(source))[0][1] == TEXT
    external = ezdxf.readfile(
        convert(oda_command, source, tmp_path / "dxf", "DXF", "ACAD2018")
    )
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    assert external.blocks.get(block.name).block.dxf.description == TEXT
    attr = external.modelspace().query("INSERT").first.attribs[0]
    assert attr.dxf.text == TEXT
    assert attr.dxf.true_color == 0x010203


def test_unsupported_ac1032_entity_rejects_before_replacement(tmp_path):
    doc = ezdwg.new("AC1032")
    doc.modelspace().add_line((0, 0), (1, 1))
    source = tmp_path / "safe.dwg"
    doc.save(source)
    before = source.read_bytes()
    doc.modelspace().add_spline([(0, 0, 0), (1, 2, 0), (3, 4, 0), (5, 0, 0)])
    with pytest.raises(ValueError, match="SPLINE"):
        doc.save(source)
    assert source.read_bytes() == before
    result = doc.save(source, strict=False)
    assert result.skipped_by_type == {"SPLINE": 1}
    assert result.written_entities == 1


def test_unicode_length_limit_is_utf16_units(tmp_path):
    doc = ezdwg.new("AC1032")
    doc.modelspace().add_text("😀" * 32768)
    target = tmp_path / "long.dwg"
    with pytest.raises(ValueError, match="utf-16"):
        doc.save(target)
    assert not target.exists()


def test_ac1032_paper_space_rejects_before_replacement(tmp_path):
    doc = ezdwg.new("AC1032")
    target = tmp_path / "safe.dwg"
    doc.save(target)
    before = target.read_bytes()
    doc.paperspace().add_text(TEXT)
    with pytest.raises(ValueError, match="model space and blocks only"):
        doc.save(target)
    assert target.read_bytes() == before


def test_ac1032_cli_export(tmp_path, capsys):
    from ezdwg import cli

    source, output = tmp_path / "source.dwg", tmp_path / "out.dwg"
    drawing().save(source)
    assert (
        cli._run_write(str(source), str(output), dwg_version="AC1032", strict=True) == 0
    )
    assert "target_version: AC1032" in capsys.readouterr().out
    assert next(ezdwg.read(str(output)).modelspace().query("TEXT")).dxf["text"] == TEXT


def test_independent_ac1032_input_preserves_rgb_and_unicode(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    reference = ezdxf.new("R2018")
    reference.layers.new(LAYER, dxfattribs={"true_color": 0x73B9FF})
    reference.modelspace().add_line(
        (1, 2, 3),
        (4, 5, 6),
        dxfattribs={"layer": LAYER, "true_color": 0x123456},
    )
    reference.modelspace().add_text(
        TEXT, dxfattribs={"layer": LAYER, "true_color": 0x2468AC, "height": 2}
    )
    dxf = tmp_path / "independent.dxf"
    reference.saveas(dxf)
    source = convert(oda_command, dxf, tmp_path / "input", "DWG", "ACAD2018")
    output = tmp_path / "native.dwg"
    result = ezdwg.to_dwg(str(source), str(output), version="AC1032", strict=True)
    assert not result.diagnostics
    external = ezdxf.readfile(
        convert(oda_command, output, tmp_path / "output", "DXF", "ACAD2018")
    )
    text = external.modelspace().query("TEXT").first
    assert text.dxf.text == TEXT
    assert text.dxf.true_color == 0x2468AC
    assert external.modelspace().query("LINE").first.dxf.true_color == 0x123456
    assert external.layers.get(LAYER).dxf.true_color == 0x73B9FF
