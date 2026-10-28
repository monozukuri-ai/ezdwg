"""The R2000 writer picks one legacy code page per drawing (header $DWGCODEPAGE).

Before 0.16.4 the choice was cp1252 or cp932 only, so Chinese, Korean, Cyrillic ... text
was replaced with ``?``. Now the first of ``WRITE_CODEPAGES`` that stores every string wins,
``header["codepage"]`` can force one, and the reader decodes the result with the same table.
"""

from __future__ import annotations

import pytest

import ezdwg
from ezdwg import WRITE_CODEPAGES, select_codepage


def _file_codepage(path) -> int:
    """$DWGCODEPAGE as stored in the R2000 file header (RS at offset 0x13)."""
    data = path.read_bytes()
    assert data[:6] == b"AC1015"
    return int.from_bytes(data[0x13:0x15], "little")


@pytest.mark.parametrize(
    ("text", "codepage"),
    [
        ("ASCII only", 30),
        ("Übergröße café", 30),
        ("日本語 ABC", 38),
        ("电子图板 机械", 39),  # simplified-only ideographs are not in cp932
        ("도면 한국어", 40),
        ("図面 寸法", 38),  # kanji without kana: GBK could store it too, Japanese wins
        ("電子圖板 繁體", 38),  # traditional forms = Japanese kyujitai, cp932 stores them
        ("Чертёж", 29),  # cp932 has basic Cyrillic, but a Cyrillic-only drawing keeps 1251
        ("Σχέδιο", 32),  # accented Greek is not in cp932
        ("φ100", 38),  # plain Greek letters are symbols in Japanese drawings
        ("ภาพวาด", 37),
        ("שרטוט", 34),
        ("رسم", 35),
    ],
)
def test_select_codepage_prefers_the_first_full_match(text, codepage):
    codec, code = select_codepage([text, "layer-1"])
    assert code == codepage
    assert text.encode(codec).decode(codec) == text


def test_select_codepage_stores_everything_when_one_page_can():
    # GBK stores kana and every unified ideograph, so mixed Japanese + Simplified Chinese
    # text is kept intact as ANSI_936 rather than replaced under cp932
    assert select_codepage(["日本語", "図面", "寸法", "电子图板"]) == ("gbk", 39)


def test_kana_and_hangul_pin_the_code_page_of_the_drawing():
    # A Japanese drawing keeps ANSI_932 even when a few strings cannot be stored (they become
    # "?" with a diagnostic) rather than silently switching the whole file to GBK
    assert select_codepage(["ﾊﾞﾙﾌﾟ", "図面", "寸法", "电子图板"]) == ("cp932", 38)
    assert select_codepage(["図面ゲート café"]) == ("cp932", 38)
    assert select_codepage(["电子图板", "机械", "图纸", "ﾊﾞﾙﾌﾟ"]) == ("cp932", 38)
    assert select_codepage(["도면", "電子圖板"]) == ("cp949", 40)


def test_select_codepage_falls_back_to_the_fewest_replacements():
    # Without kana/hangul no single page fits here: the one with the fewest
    # unrepresentable strings wins (ties keep the preference order)
    assert select_codepage(["Чертёж", "Σχέδιο", "рисунок"]) == ("cp1251", 29)
    assert select_codepage(["电子图板", "机械", "图纸", "ภาพวาด"]) == ("gbk", 39)


def test_select_codepage_honours_a_requested_code_page():
    assert select_codepage(["ASCII"], 38) == ("cp932", 38)
    assert select_codepage(["ASCII"], 39) == ("gbk", 39)
    # Unknown requests fall back to the automatic choice
    assert select_codepage(["ASCII"], 999) == ("cp1252", 30)


def test_table_codecs_exist_and_indices_are_unique():
    codes = [code for _, code in WRITE_CODEPAGES]
    assert len(codes) == len(set(codes))
    for codec, _ in WRITE_CODEPAGES:
        assert "x".encode(codec) == b"x"


@pytest.mark.parametrize(
    ("text", "layer", "codepage"),
    [
        ("电子图板 机械制图", "图层1", 39),
        ("도면 한국어 주석", "레이어", 40),
        ("Чертёж деталь", "Слой", 29),
    ],
)
def test_non_japanese_text_roundtrips_through_the_matching_code_page(tmp_path, text, layer, codepage):
    drawing = ezdwg.new()
    drawing.layers[layer] = {"color_index": 2}
    msp = drawing.modelspace()
    msp.add_text(text, insert=(0.0, 0.0), height=2.5, layer=layer)
    msp.add_line((0.0, 0.0), (10.0, 0.0), layer=layer)
    out = tmp_path / "out.dwg"
    result = drawing.save(out)
    assert not [d for d in result.diagnostics if d.attribute == "text" and d.action == "replaced"]
    assert _file_codepage(out) == codepage
    doc = ezdwg.read(str(out))
    texts = [e.dxf["text"] for e in doc.modelspace().query(["TEXT"])]
    assert texts == [text]
    assert layer in doc.layers()


def test_requested_code_page_in_header_is_used(tmp_path):
    drawing = ezdwg.new()
    drawing.header["codepage"] = 39
    drawing.modelspace().add_text("ASCII", insert=(0.0, 0.0), height=2.5)
    out = tmp_path / "out.dwg"
    drawing.save(out)
    assert _file_codepage(out) == 39


def test_mixed_scripts_report_replacements_instead_of_failing(tmp_path):
    drawing = ezdwg.new()
    msp = drawing.modelspace()
    for t in ("ﾊﾟﾙﾌﾞ", "図面", "电子图板"):
        msp.add_text(t, insert=(0.0, 0.0), height=2.5)
    out = tmp_path / "out.dwg"
    result = drawing.save(out, strict=False)
    assert _file_codepage(out) == 38
    replaced = [d for d in result.diagnostics if d.action == "replaced"]
    assert len(replaced) == 1


def test_text_starting_like_a_utf16_byte_order_mark_is_stored_as_is(tmp_path):
    # "ÿþ" is FF FE in cp1252. The writer compared its bytes through a BOM-sniffing decode
    # and rejected such strings ("text is not representable in DWG codepage 30"); DGN texts
    # decoded with a code page start that way (three production drawings, 2026-10-22)
    drawing = ezdwg.new()
    msp = drawing.modelspace()
    msp.add_text("ÿþ café", insert=(0.0, 0.0), height=2.5)
    msp.add_text("ï»¿ note", insert=(0.0, 5.0), height=2.5)
    out = tmp_path / "out.dwg"
    result = drawing.save(out)
    assert not [d for d in result.diagnostics if d.action == "replaced"]
    assert _file_codepage(out) == 30
    doc = ezdwg.read(str(out))
    texts = [e.dxf["text"] for e in doc.modelspace().query(["TEXT"])]
    assert texts == ["ÿþ café", "ï»¿ note"]


def test_characters_only_pythons_codec_maps_are_replaced_instead_of_rejected(tmp_path):
    # Python's cp932 maps the undefined byte A0 to U+F8F0 (a DXF read as cp932 carries it);
    # the writer's Shift_JIS table has no such character and used to fail the whole file
    drawing = ezdwg.new()
    msp = drawing.modelspace()
    msp.add_text("ﾊﾟﾙﾌﾞ L\uf8f0990", insert=(0.0, 0.0), height=2.5)  # kana pins cp932
    out = tmp_path / "out.dwg"
    result = drawing.save(out, strict=False)
    assert _file_codepage(out) == 38
    assert len([d for d in result.diagnostics if d.action == "replaced"]) == 1
    doc = ezdwg.read(str(out))
    assert [e.dxf["text"] for e in doc.modelspace().query(["TEXT"])] == ["ﾊﾟﾙﾌﾞ L?990"]


def test_representability_helpers_agree_with_the_writer():
    assert ezdwg.is_representable(30, "ÿþ café")
    assert not ezdwg.is_representable(38, "")
    assert ezdwg.replace_unrepresentable(38, "L990") == ("L?990", 1)
    assert ezdwg.replace_unrepresentable(38, "L990", "") == ("L990", 1)
    assert ezdwg.replace_unrepresentable(30, "plain") == ("plain", 0)
    with pytest.raises(ValueError):
        ezdwg.is_representable(999, "x")
