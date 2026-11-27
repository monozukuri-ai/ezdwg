from __future__ import annotations

import math
from collections import Counter
from pathlib import Path

import pytest

import ezdwg
import ezdwg.document as document_module
from ezdwg import _style_tables, raw

ezdxf = pytest.importorskip("ezdxf")

ROOT = Path(__file__).resolve().parents[1]
AC1015 = ROOT / "test_dwg/acadsharp/sample_AC1015.dwg"
IMPERIAL = ROOT / "examples/data/mechanical_example-imperial.dwg"
LINE = ROOT / "test_dwg/line_2007.dwg"
STANDARD_HANDLE = 0x11


def _convert(tmp_path, source, name="out.dxf", **kw):
    out = tmp_path / name
    ezdwg.to_dxf(str(source), str(out), **kw)
    return ezdxf.readfile(out)


def _text_like(doc):
    """Every TEXT, MTEXT, ATTDEF and ATTRIB of every layout and block."""
    found = []
    for block in doc.blocks:
        for entity in block:
            if entity.dxftype() in {"TEXT", "MTEXT", "ATTDEF"}:
                found.append(entity)
            if entity.dxftype() == "INSERT":
                found.extend(entity.attribs)
    return found


def _rows_with(monkeypatch, path, change):
    rows = _style_tables.text_style_rows(str(path))
    change(rows)
    monkeypatch.setattr(_style_tables, "text_style_rows", lambda _path: rows)


def _row(handle, name, **values):
    row = {
        "handle": handle,
        "name": name,
        "kind": "STYLE",
        "extended_data": False,
        "shape": False,
        "vertical": False,
        "height": 0.0,
        "width": 1.0,
        "oblique": 0.0,
        "generation": 0,
        "last_height": 0.2,
        "font": "arial.ttf",
        "bigfont": "",
    }
    row.update(values)
    return row


def test_style_table_has_fonts(tmp_path):
    doc = _convert(tmp_path, AC1015)
    style = doc.styles.get("custom_text_style")
    assert style.dxf.font.lower() == "consola.ttf"
    assert style.get_extended_font_data() == ("Consolas", False, False)
    assert doc.styles.find_shx("ltypeshp.shx") is not None
    doc = _convert(tmp_path, IMPERIAL, name="imperial.dxf")
    notes = doc.styles.get("Notes")
    assert notes.dxf.font.lower() == "arial.ttf"
    assert notes.dxf.height == pytest.approx(0.125)
    assert doc.styles.get("Standard").get_extended_font_data()[0] == "Arial"
    assert doc.styles.get("Standard").get_xdata("ACAD") == [(1000, "Arial"), (1071, 34)]


def test_style_values_and_flags(tmp_path, monkeypatch):
    def change(rows):
        rows[0x9001] = _row(
            0x9001,
            "Slanted",
            vertical=True,
            height=2.5,
            width=0.8,
            oblique=math.radians(15.0),
            generation=6,
            last_height=3.0,
            bigfont="bigfont.shx",
        )

    _rows_with(monkeypatch, LINE, change)
    style = _convert(tmp_path, LINE).styles.get("Slanted")
    assert style.dxf.flags & 4
    assert style.dxf.height == pytest.approx(2.5)
    assert style.dxf.width == pytest.approx(0.8)
    assert style.dxf.oblique == pytest.approx(15.0)
    assert style.dxf.generation_flags == 6
    assert style.dxf.last_height == pytest.approx(3.0)
    assert style.dxf.bigfont == "bigfont.shx"


def test_text_entities_reference_decoded_style(tmp_path):
    rows = _style_tables.text_style_rows(str(AC1015))
    expected = Counter(
        (e.dxftype, rows[e.dxf["style_handle"]]["name"])
        for e in ezdwg.read(str(AC1015)).entities().query("TEXT MTEXT")
    )
    doc = _convert(tmp_path, AC1015, types="TEXT MTEXT")
    actual = Counter((e.dxftype(), e.dxf.style) for e in doc.modelspace().query("TEXT MTEXT"))
    assert actual == expected and {name for _, name in actual} - {"Standard"}


@pytest.mark.parametrize("preserve_colors", [True, False])
def test_every_text_writer_sets_style(tmp_path, monkeypatch, preserve_colors):
    _rows_with(
        monkeypatch, AC1015, lambda rows: rows[STANDARD_HANDLE].update(name="Renamed")
    )
    doc = _convert(tmp_path, AC1015, preserve_colors=preserve_colors)
    styles: dict[str, Counter] = {}
    for entity in _text_like(doc):
        styles.setdefault(entity.dxftype(), Counter())[entity.dxf.style] += 1
    assert set(styles["MTEXT"]) == {"Renamed"}
    assert set(styles["ATTDEF"]) == {"Renamed"}
    assert set(styles["ATTRIB"]) == {"Renamed"}
    assert styles["TEXT"]["Renamed"] >= 17 and styles["TEXT"]["custom_text_style"] == 12


def test_styles_kept_without_colors(tmp_path):
    with_colors = _convert(tmp_path, AC1015, name="with.dxf")
    without = _convert(tmp_path, AC1015, name="without.dxf", preserve_colors=False)

    def key(doc):
        return sorted((e.dxftype(), e.dxf.style) for e in _text_like(doc))

    assert key(with_colors) == key(without)
    assert sorted(s.dxf.name for s in without.styles) == sorted(
        s.dxf.name for s in with_colors.styles
    )


def test_mtext_styles_not_decoded_again_with_colors(tmp_path, monkeypatch):
    calls = []
    decode = raw.decode_mtext_write_properties

    def counting(path):
        calls.append(path)
        return decode(path)

    monkeypatch.setattr(raw, "decode_mtext_write_properties", counting)
    document_module._entity_style_map.cache_clear()
    _convert(tmp_path, AC1015, types="MTEXT")
    assert len(calls) == 1


@pytest.mark.parametrize("dxf_version", ["R2018", "R12"])
def test_style_rows_edge_cases(tmp_path, monkeypatch, dxf_version):
    rows = {
        0x9001: _row(0x9001, ".VnTime", font=""),
        0x9002: _row(0x9002, "XREF|ROMANS", font="romans.shx"),
        0x9003: _row(0x9003, "DUP", font="first.ttf"),
        0x9004: _row(0x9004, "dup", font="second.ttf"),
        0x9005: _row(0x9005, "", shape=True, font="ltypeshp.shx"),
        0x9006: _row(
            0x9006,
            "Damaged",
            height=float("nan"),
            width=0.0,
            oblique=float("inf"),
            last_height=float("nan"),
        ),
        0x9007: _row(0x9007, "STANDARD", font="romans.shx"),
        0x9008: _row(0x9008, "a<b"),
        0x9009: _row(0x9009, "Junk", generation="x", height="tall"),
        0x900A: _row(0x900A, None),
        0x900B: _row(0x900B, "", shape=True, font=""),
        0x900C: _row(0x900C, "Huge", oblique=1e307),
        0x900D: _row(0x900D, "Back", oblique=math.radians(345.0)),
        0x900E: _row(0x900E, "Straße"),
        0x900F: _row(0x900F, "Chữ Việt"),
    }
    monkeypatch.setattr(_style_tables, "text_style_rows", lambda _path: rows)
    out = tmp_path / "edge.dxf"
    ezdwg.to_dxf(str(LINE), str(out), dxf_version=dxf_version)
    doc = ezdxf.readfile(out)
    names = [s.dxf.name for s in doc.styles]
    assert ".VnTime" in names and "XREF|ROMANS" in names
    assert [n.lower() for n in names].count("dup") == 1
    assert doc.styles.get("dup").dxf.font == "first.ttf"
    assert [n.lower() for n in names].count("standard") == 1
    assert doc.styles.get("Standard").dxf.font == "romans.shx"
    assert doc.styles.find_shx("ltypeshp.shx") is not None
    damaged = doc.styles.get("Damaged")
    assert damaged.dxf.height == 0.0 and damaged.dxf.width == 1.0
    assert damaged.dxf.oblique == 0.0 and math.isfinite(damaged.dxf.last_height)
    assert "a<b" not in names
    assert doc.styles.get("Huge").dxf.oblique == 0.0
    assert doc.styles.get("Back").dxf.oblique == pytest.approx(-15.0)
    # R12 is cp1252: ezdxf writes other characters as \U+XXXX and reads them back escaped.
    vietnamese = "Chữ Việt" if dxf_version != "R12" else "Ch\\U+1eef Vi\\U+1ec7t"
    assert "Straße" in names and vietnamese in names
    tokens = {line.strip().lower() for line in out.read_text(errors="replace").splitlines()}
    assert not tokens & {"nan", "inf", "-inf", "+inf"}


@pytest.mark.parametrize("dxf_version", ["R2018", "R12"])
def test_style_control_characters_are_removed(tmp_path, monkeypatch, dxf_version):
    rows = {
        0x9001: _row(0x9001, "CtlFont", font="ari\nal.ttf"),
        0x9002: _row(0x9002, "CtlBig", font="romans.shx", bigfont="big\nfont.shx"),
        0x9003: _row(0x9003, "", shape=True, font="ctl\nshape.shx"),
        0x9004: _row(0x9004, "", shape=True, font="\r\n"),
        0x9005: _row(0x9005, "CtlNul", font="a\x00b.ttf"),
        0x9006: _row(0x9006, "CtlXdata", extended_data=True),
        0x9007: _row(0x9007, "XNoFamily", extended_data=True),
        0x9008: _row(0x9008, "XBadFlags", extended_data=True),
        0x9009: _row(0x9009, "XBlankFamily", extended_data=True),
    }
    fonts = {
        0x9006: ("Con\nsolas", 34),
        0x9007: (None, 34),
        0x9008: ("Consolas", "x"),
        0x9009: ("\n", 34),
    }
    monkeypatch.setattr(_style_tables, "text_style_rows", lambda _path: rows)
    monkeypatch.setattr(_style_tables, "text_style_font_xdata", lambda _path: fonts)
    out = tmp_path / "ctl.dxf"
    ezdwg.to_dxf(str(LINE), str(out), dxf_version=dxf_version)
    from ezdxf import recover

    doc = ezdxf.readfile(out)
    _recovered, auditor = recover.readfile(out)
    assert not auditor.has_errors
    assert doc.styles.get("CtlFont").dxf.font == "arial.ttf"
    assert doc.styles.get("CtlBig").dxf.bigfont == "bigfont.shx"
    assert doc.styles.get("CtlNul").dxf.font == "ab.ttf"
    assert doc.styles.find_shx("ctlshape.shx") is not None
    # Iterating the table skips shape-file entries, which have no name.
    styles = [e for e in doc.entitydb.values() if e.dxftype() == "STYLE"]
    assert [s.dxf.font for s in styles if s.dxf.flags & 1] == ["ctlshape.shx"]
    assert doc.styles.get("CtlXdata").get_xdata("ACAD") == [(1000, "Consolas"), (1071, 34)]
    for name in ("XNoFamily", "XBadFlags", "XBlankFamily"):
        assert not doc.styles.get(name).has_xdata("ACAD")
    for style in styles:
        values = [style.dxf.name, style.dxf.font, style.dxf.get("bigfont", "")]
        if style.has_xdata("ACAD"):
            values += [v for _code, v in style.get_xdata("ACAD") if isinstance(v, str)]
        assert all(ord(c) >= 32 for value in values for c in value), values


@pytest.mark.parametrize("name", [None, "A\nB", "A\rB", "A\x00B"])
def test_unknown_style_handle_leaves_default(tmp_path, monkeypatch, name):
    custom = next(
        h for h, r in _style_tables.text_style_rows(str(AC1015)).items()
        if r["name"] == "custom_text_style"
    )
    if name is None:
        _rows_with(monkeypatch, AC1015, lambda rows: rows.pop(custom))
    else:
        _rows_with(monkeypatch, AC1015, lambda rows: rows[custom].update(name=name))
    doc = _convert(tmp_path, AC1015, types="TEXT")
    assert not [s for s in doc.styles if any(ord(c) < 32 for c in s.dxf.name)]
    styles = Counter(e.dxf.style for e in doc.modelspace().query("TEXT"))
    assert styles == Counter({"Standard": 29})


def test_undecodable_style_table_keeps_conversion(tmp_path, monkeypatch):
    def broken(_path):
        raise ValueError("broken STYLE table")

    monkeypatch.setattr(raw, "decode_write_tables", broken)
    doc = _convert(tmp_path, AC1015, types="TEXT")
    assert Counter(e.dxf.style for e in doc.modelspace().query("TEXT")) == Counter(
        {"Standard": 29}
    )


def test_styles_r12_saves(tmp_path):
    doc = _convert(tmp_path, AC1015, dxf_version="R12")
    assert doc.styles.get("custom_text_style").dxf.font.lower() == "consola.ttf"


def _tag_values(entry, code):
    return [t.value for t in entry.pattern_tags.tags if t.code == code]


def _all_entities(doc):
    """Every entity of every layout and block, attributes included."""
    found = []
    for block in doc.blocks:
        for entity in block:
            found.append(entity)
            if entity.dxftype() == "INSERT":
                found.extend(entity.attribs)
    return found


def _linetype_key(entity):
    return entity.dxf.get("linetype", "BYLAYER").upper(), entity.dxf.get("ltscale", 1.0)


def test_ltype_table_matches_decoder(tmp_path):
    doc = _convert(tmp_path, AC1015)
    rows = raw.decode_linetypes(str(AC1015))
    assert len(rows) == 8
    assert doc.linetypes.get("Continuous").dxf.description == "Solid line"
    for _h, name, desc, length, dashes in rows:
        entry = doc.linetypes.get(name)
        assert entry.dxf.description == desc
        assert _tag_values(entry, 49) == pytest.approx(dashes)
        if dashes:
            assert _tag_values(entry, 40) == pytest.approx([length])


def test_complex_linetype_round_trip(tmp_path):
    doc = _convert(tmp_path, AC1015)
    gas = doc.linetypes.get("GAS_LINE")
    assert _tag_values(gas, 9) == ["GAS"]
    assert _tag_values(gas, 74) == [0, 10, 0]
    assert doc.entitydb.get(_tag_values(gas, 340)[0]).dxftype() == "STYLE"
    assert _tag_values(gas, 46) == pytest.approx([0.1])
    assert _tag_values(gas, 44) == pytest.approx([-0.1])
    assert _tag_values(gas, 45) == pytest.approx([-0.05])
    tracks = doc.linetypes.get("TRACKS")
    assert _tag_values(tracks, 75) == [130]
    shx = doc.entitydb.get(_tag_values(tracks, 340)[0])
    assert shx.dxf.flags & 1 and shx.dxf.font.lower() == "ltypeshp.shx"
    # DWG stores pi; .lin files and ezdxf's own complex linetype tags use degrees.
    assert _tag_values(doc.linetypes.get("ZIGZAG"), 50) == pytest.approx([0.0, 180.0])
    codes = [t.code for t in gas.pattern_tags.tags]
    assert codes == [72, 73, 40, 49, 74, 49, 74, 75, 340, 46, 50, 44, 45, 9, 49, 74]


@pytest.mark.parametrize("source", [AC1015, IMPERIAL])
def test_layer_linetypes(tmp_path, source):
    doc = _convert(tmp_path, source)
    names_by_handle, layer_ltype = document_module._linetype_tables(str(source))
    layer_names = document_module._layer_names_by_handle(str(source))
    checked = Counter()
    for layer, linetype in layer_ltype.items():
        name = layer_names.get(layer)
        if name in doc.layers:
            expected = names_by_handle[linetype].upper()
            assert doc.layers.get(name).dxf.linetype.upper() == expected
            checked[expected] += 1
    assert len(checked) > 1


def test_entity_linetypes_and_scale(tmp_path):
    entity_map = document_module._entity_linetype_map(str(AC1015))
    source = ezdwg.read(str(AC1015)).entities().query()
    expected = Counter(
        entity_map[e.handle][0] for e in source if entity_map[e.handle][0] != "BYLAYER"
    )
    assert expected["ACAD_ISO02W100"] == 8 and expected["GAS_LINE"] == 1
    doc = _convert(tmp_path, AC1015)
    actual = Counter(_linetype_key(e)[0] for e in doc.modelspace())
    del actual["BYLAYER"]
    assert actual == expected
    assert Counter(e.dxf.get("ltscale") for e in doc.modelspace()) == Counter(
        {None: len(doc.modelspace()) - 1, 2.0: 1}
    )


def test_linetypes_kept_without_colors(tmp_path):
    with_colors = _convert(tmp_path, AC1015, name="with.dxf")
    without = _convert(tmp_path, AC1015, name="without.dxf", preserve_colors=False)

    def key(doc):
        return Counter(_linetype_key(e) for e in _all_entities(doc))

    assert key(with_colors) == key(without)
    assert len(key(without)) > 3

    def layers(doc):
        return {layer.dxf.name: layer.dxf.linetype for layer in doc.layers}

    assert layers(with_colors) == layers(without)
    assert set(layers(without).values()) - {"Continuous"}


def _ltype_row(handle, name, dashes, length=1.0, elements=None, description="d"):
    return {
        "handle": handle,
        "name": name,
        "description": description,
        "pattern_length": length,
        "dashes": dashes,
        "complex": None,
        "extended_data": False,
        "elements": elements,
    }


def _element(length, flags=0, shape=0, rotation=0.0, style=None, text=None):
    return (length, flags, shape, 0.0, 0.0, 1.0, rotation, style, text)


@pytest.mark.parametrize("target", [0x9005, 0x900E, 0x900F, 0x9010])
def test_linetype_rows_edge_cases(tmp_path, monkeypatch, target):
    standard = next(
        h for h, r in _style_tables.text_style_rows(str(LINE)).items() if r["name"] == "Standard"
    )
    text = 'A,B]"' + "é"
    rows = {
        0x9001: _ltype_row(0x9001, "XREF|DASHED", [0.5, -0.25]),
        0x9002: _ltype_row(0x9002, "Dashed", [0.5, -0.25]),
        0x9003: _ltype_row(0x9003, "DASHED", [0.75, -0.25]),
        0x9004: _ltype_row(0x9004, "CONTINUOUS", [], description="other"),
        0x9005: _ltype_row(0x9005, "NANROW", [0.5, float("nan")]),
        0x9006: _ltype_row(0x9006, "INFLENGTH", [0.5], length=float("inf")),
        0x9007: _ltype_row(0x9007, "UNDECODED", None),
        0x9008: _ltype_row(
            0x9008, "TEXTY", [0.5, -0.5], elements=[
                _element(0.5), _element(-0.5, flags=2, style=standard, text=text)
            ]
        ),
        0x9009: _ltype_row(
            0x9009, "NOSTYLE", [0.5, -0.5], elements=[
                _element(0.5), _element(-0.5, flags=4, shape=7, style=0x7777)
            ]
        ),
        0x900A: _ltype_row(
            0x900A, "HUGEROT", [0.5, -0.5], elements=[
                _element(0.5), _element(-0.5, flags=2, rotation=1e308, style=standard, text="X")
            ]
        ),
        0x900B: _ltype_row(0x900B, "SHORTELEMENTS", [0.5, -0.5], elements=[_element(0.5)]),
        0x900D: _ltype_row(
            0x900D, "BIGSHAPE", [0.5], description="two\nlines", elements=[
                _element(0.5, flags=4, shape=70000, style=standard)
            ]
        ),
        0x900C: _ltype_row(0x900C, "", [0.5]),
        0x900E: _ltype_row(0x900E, "NL\nNAME", [0.5]),
        0x900F: _ltype_row(0x900F, "CR\rNAME", [0.5]),
        0x9010: _ltype_row(0x9010, "NUL\x00NAME", [0.5]),
        0x9011: _ltype_row(
            0x9011, "EMPTYTEXT", [0.5, -0.5], elements=[
                _element(0.5), _element(-0.5, flags=2, style=standard, text=None)
            ]
        ),
    }
    monkeypatch.setattr(_style_tables, "linetype_rows", lambda _path: rows)
    entities = document_module._entity_linetype_map(str(LINE))
    names_by_handle, layers = document_module._linetype_tables(str(LINE))
    assert entities and layers
    monkeypatch.setattr(
        document_module,
        "_entity_linetype_map",
        lambda _path: dict.fromkeys(entities, (rows[target]["name"], target, 1.0)),
    )
    monkeypatch.setattr(
        document_module,
        "_linetype_tables",
        lambda _path: (names_by_handle, dict.fromkeys(layers, target)),
    )
    out = tmp_path / "edge.dxf"
    ezdwg.to_dxf(str(LINE), str(out), dxf_version="R2018")
    doc = ezdxf.readfile(out)
    names = [lt.dxf.name.upper() for lt in doc.linetypes]
    assert names.count("DASHED") == 1 and "XREF|DASHED" in names
    assert _tag_values(doc.linetypes.get("DASHED"), 49) == pytest.approx([0.5, -0.25])
    assert names.count("CONTINUOUS") == 1
    assert doc.linetypes.get("Continuous").dxf.description == "other"
    assert not {"NANROW", "INFLENGTH", "UNDECODED", ""} & set(names)
    assert not [n for n in names if any(ord(c) < 32 for c in n)]
    assert all(not e.dxf.hasattr("linetype") for e in doc.modelspace())
    assert {layer.dxf.linetype for layer in doc.layers} == {"Continuous"}
    empty = doc.linetypes.get("EMPTYTEXT")
    assert _tag_values(empty, 9) == [] and _tag_values(empty, 74) == [0, 0]
    assert _tag_values(doc.linetypes.get("TEXTY"), 9) == [text]
    nostyle = doc.linetypes.get("NOSTYLE")
    assert _tag_values(nostyle, 340) == [] and _tag_values(nostyle, 75) == [7]
    assert _tag_values(doc.linetypes.get("HUGEROT"), 9) == []
    assert _tag_values(doc.linetypes.get("SHORTELEMENTS"), 49) == pytest.approx([0.5, -0.5])
    big = doc.linetypes.get("BIGSHAPE")
    assert _tag_values(big, 75) == [] and big.dxf.description == "twolines"
    tokens = {line.strip().lower() for line in out.read_text(encoding="utf-8").splitlines()}
    assert not tokens & {"nan", "inf", "-inf", "+inf"}
    auditor = doc.audit()
    assert not auditor.has_errors


def test_unknown_linetype_handle_and_bad_scale(tmp_path, monkeypatch):
    center = next(
        h for h, r in _style_tables.linetype_rows(str(IMPERIAL)).items() if r["name"] == "CENTER"
    )
    real = document_module._entity_linetype_map(str(IMPERIAL))

    def convert(value, name):
        monkeypatch.setattr(
            document_module, "_entity_linetype_map", lambda _path: dict.fromkeys(real, value)
        )
        return list(_convert(tmp_path, IMPERIAL, name=name, types="LINE").modelspace())

    gone = convert(("GONE", 0x999, 2.0), "gone.dxf")
    assert gone and all(not e.dxf.hasattr("linetype") for e in gone)
    assert all(e.dxf.ltscale == 2.0 for e in gone)
    named = convert(("CENTER", center, float("nan")), "center.dxf")
    assert named and all(e.dxf.linetype == "CENTER" for e in named)
    assert all(not e.dxf.hasattr("ltscale") for e in named)


def test_attributes_get_entity_linetype(tmp_path, monkeypatch):
    real = document_module._entity_linetype_map(str(AC1015))
    iso = next(h for h, name, *_ in raw.decode_linetypes(str(AC1015)) if name == "ACAD_ISO02W100")
    monkeypatch.setattr(
        document_module,
        "_entity_linetype_map",
        lambda _path: dict.fromkeys(real, ("ACAD_ISO02W100", iso, 3.0)),
    )
    doc = _convert(tmp_path, AC1015)
    attribs = [a for e in doc.modelspace().query("INSERT") for a in e.attribs]
    assert attribs
    assert {(a.dxf.get("linetype"), a.dxf.get("ltscale")) for a in attribs} == {
        ("ACAD_ISO02W100", 3.0)
    }


@pytest.mark.parametrize("dxf_version", ["R2018", "R12"])
def test_non_ascii_linetype_name(tmp_path, monkeypatch, dxf_version):
    name = "Đường_ống"
    rows = _style_tables.linetype_rows(str(LINE))
    rows[0x9001] = _ltype_row(0x9001, name, [0.5, -0.25])
    monkeypatch.setattr(_style_tables, "linetype_rows", lambda _path: rows)
    real = document_module._entity_linetype_map(str(LINE))
    value = (name, 0x9001, 1.0)
    monkeypatch.setattr(
        document_module, "_entity_linetype_map", lambda _path: dict.fromkeys(real, value)
    )
    doc = _convert(tmp_path, LINE, dxf_version=dxf_version)
    lines = list(doc.modelspace())
    assert lines and all(e.dxf.linetype in doc.linetypes for e in lines)
    assert _tag_values(doc.linetypes.get(lines[0].dxf.linetype), 49) == pytest.approx([0.5, -0.25])
    if dxf_version == "R2018":
        assert lines[0].dxf.linetype == name
    assert not doc.audit().has_errors


def test_linetypes_r12_saves(tmp_path):
    doc = _convert(tmp_path, AC1015, dxf_version="R12")
    gas = doc.linetypes.get("GAS_LINE")
    assert _tag_values(gas, 9) == []
    assert _tag_values(gas, 49) == pytest.approx([0.5, -0.2, -0.25])
    assert sum(1 for e in doc.modelspace() if e.dxf.get("linetype") == "GAS_LINE") == 1


def test_hatch_pattern_scale_from_sample(tmp_path):
    doc = _convert(tmp_path, AC1015)
    scales = sorted(
        round(h.dxf.pattern_scale, 3)
        for h in doc.modelspace().query("HATCH")
        if not h.dxf.solid_fill
    )
    assert scales == [0.1, 0.1, 0.1, 0.1, 1.0, 1.0]


def test_hatch_definition_is_not_scaled_twice(tmp_path):
    decoded = {
        e.dxf["pattern_name"]: e.dxf["pattern_lines"]
        for e in ezdwg.read(str(AC1015)).modelspace().query("HATCH")
        if not e.dxf.get("solid_fill") and e.dxf.get("pattern_lines")
    }
    assert decoded
    doc = _convert(tmp_path, AC1015)
    checked = 0
    for hatch in doc.modelspace().query("HATCH"):
        lines = decoded.get(hatch.dxf.pattern_name)
        if hatch.dxf.solid_fill or not lines:
            continue
        assert len(hatch.pattern.lines) == len(lines)
        for got, want in zip(hatch.pattern.lines, lines):
            assert got.angle == pytest.approx(want["angle"])
            assert tuple(got.offset) == pytest.approx(want["offset"])
            assert list(got.dash_length_items) == pytest.approx(want["dashes"])
        checked += 1
    assert checked == len(decoded)


@pytest.mark.parametrize("version", ["AC1021", "AC1024", "AC1027", "AC1032"])
def test_hatch_names_from_string_stream(version):
    def names(path):
        return sorted((row[0], row[1]) for row in raw.decode_hatch_entities(str(path)))

    want = names(AC1015)
    assert "ANSI31" in dict(want).values()
    assert names(ROOT / f"test_dwg/acadsharp/sample_{version}.dwg") == want


def test_write_hatch_unnamed_pattern():
    line = {"angle": 30.0, "base": (0.0, 0.0), "offset": (3.0, 4.0), "dashes": []}
    user = _write_hatch_in_new_doc(_hatch_dxf(pattern_name="", pattern_lines=[line]))
    assert (user.dxf.pattern_name, user.dxf.pattern_type) == ("_USER", 0)
    assert tuple(user.pattern.lines[0].offset) == pytest.approx((3.0, 4.0))
    bare = _write_hatch_in_new_doc(_hatch_dxf(pattern_name=""))
    assert (bare.dxf.pattern_name, bare.dxf.pattern_type) == ("ANSI31", 1)


def _hatch_dxf(**values):
    dxf = {
        "paths": [{"points": [(0, 0), (10, 0), (10, 10)], "closed": True}],
        "pattern_name": "MUDST",
    }
    dxf.update(values)
    return dxf


def _write_hatch_in_new_doc(dxf):
    from ezdwg import convert

    doc = ezdxf.new("R2000")
    doc.header["$MEASUREMENT"] = 1
    assert convert._write_hatch(doc.modelspace(), dxf, {})
    return list(doc.modelspace().query("HATCH"))[0]


def test_write_hatch_angle_scale_and_lines():
    line = {"angle": 30.0, "base": (1.0, 2.0), "offset": (3.0, 4.0), "dashes": [5.0, -2.0]}
    hatch = _write_hatch_in_new_doc(
        _hatch_dxf(
            pattern_angle=45.0, pattern_scale=5.0, pattern_double=True, pattern_lines=[line]
        )
    )
    assert hatch.dxf.pattern_angle == 45.0
    assert hatch.dxf.pattern_scale == 5.0
    assert hatch.dxf.pattern_double == 1
    assert len(hatch.pattern.lines) == 1
    got = hatch.pattern.lines[0]
    assert got.angle == pytest.approx(30.0)
    assert tuple(got.base_point) == pytest.approx((1.0, 2.0))
    assert tuple(got.offset) == pytest.approx((3.0, 4.0))
    assert list(got.dash_length_items) == pytest.approx([5.0, -2.0])


def test_write_hatch_without_lines_scales_predefined():
    hatch = _write_hatch_in_new_doc(_hatch_dxf(pattern_scale=5.0))
    assert hatch.dxf.pattern_scale == 5.0
    assert tuple(hatch.pattern.lines[0].offset) == pytest.approx((63.5, 31.75))


def test_write_hatch_bad_numbers():
    base = _write_hatch_in_new_doc(_hatch_dxf())
    bad_line = {"angle": 0.0, "base": (0, 0), "offset": (1, 1), "dashes": [math.nan]}
    hatch = _write_hatch_in_new_doc(
        _hatch_dxf(
            pattern_scale=math.nan,
            pattern_angle=math.inf,
            pattern_lines=[bad_line, {"angle": math.inf, "base": (0, 0), "offset": (1, 1)}],
        )
    )
    assert hatch.dxf.pattern_scale == 1.0
    assert hatch.dxf.pattern_angle == 0.0
    assert [tuple(ln.offset) for ln in hatch.pattern.lines] == [
        tuple(ln.offset) for ln in base.pattern.lines
    ]
    good = {"angle": 30.0, "base": (1.0, 2.0), "offset": (3.0, 4.0), "dashes": [5.0]}
    mixed = _write_hatch_in_new_doc(
        _hatch_dxf(pattern_scale=5.0, pattern_lines=[bad_line, good])
    )
    assert len(mixed.pattern.lines) == 1
    assert tuple(mixed.pattern.lines[0].offset) == pytest.approx((3.0, 4.0))
    assert mixed.pattern.lines[0].angle == pytest.approx(30.0)
    for lines in (None, [], [bad_line]):
        huge = _write_hatch_in_new_doc(_hatch_dxf(pattern_scale=1e308, pattern_lines=lines))
        assert huge.dxf.pattern_scale == 1.0
        assert all(
            math.isfinite(v)
            for ln in huge.pattern.lines
            for v in (ln.angle, *ln.base_point, *ln.offset, *ln.dash_length_items)
        )
    for scale in (0.0, -2.0):
        assert _write_hatch_in_new_doc(_hatch_dxf(pattern_scale=scale)).dxf.pattern_scale == 1.0
