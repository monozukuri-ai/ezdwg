from __future__ import annotations

import shutil
from pathlib import Path

import pytest

import ezdwg
from ezdwg import _style_tables, raw

ROOT = Path(__file__).resolve().parents[1]
AC1015 = ROOT / "test_dwg/acadsharp/sample_AC1015.dwg"
IMPERIAL = ROOT / "examples/data/mechanical_example-imperial.dwg"
MTEXT = ROOT / "examples/data/mtext_2000.dwg"


def test_linetype_rows_join_dashes_and_elements():
    rows = {r["name"]: r for r in _style_tables.linetype_rows(str(AC1015)).values()}
    gas = rows["GAS_LINE"]
    assert gas["dashes"] == pytest.approx([0.5, -0.2, -0.25]) and gas["complex"] is True
    assert gas["elements"][1][8] == "GAS"
    assert rows["ACAD_ISO02W100"]["complex"] is False


def test_linetype_rows_missing_from_write_tables(monkeypatch):
    path = str(AC1015)
    tables = raw.decode_write_tables(path)
    victim = next(r for r in tables if r["kind"] == "LTYPE" and r["complex"])
    monkeypatch.setattr(
        raw, "decode_write_tables", lambda _p: [r for r in tables if r is not victim]
    )
    rows = _style_tables.linetype_rows(path)
    assert rows[victim["handle"]]["complex"] is None
    assert rows[victim["handle"]]["extended_data"] is False
    assert rows[victim["handle"]]["dashes"] is not None


def test_linetype_rows_missing_from_linetypes(monkeypatch):
    path = str(AC1015)
    tables = raw.decode_write_tables(path)
    victim = next(r for r in tables if r["kind"] == "LTYPE" and r["complex"])
    all_rows = raw.decode_linetypes(path)
    monkeypatch.setattr(
        raw, "decode_linetypes", lambda _p: [r for r in all_rows if r[0] != victim["handle"]]
    )
    row = _style_tables.linetype_rows(path)[victim["handle"]]
    assert row["name"] == victim["name"] and row["complex"] is True
    assert row["description"] is None and row["pattern_length"] is None
    assert row["dashes"] is None


def test_text_style_rows_are_the_write_table_rows():
    rows = _style_tables.text_style_rows(str(IMPERIAL))
    assert {r["name"] for r in rows.values()} >= {"Standard", "Notes"}
    assert "font_xdata" not in next(iter(rows.values()))
    assert next(r for r in rows.values() if r["name"] == "Notes")["font"].lower() == "arial.ttf"


def test_text_style_font_xdata():
    path = str(IMPERIAL)
    standard = next(
        h for h, r in _style_tables.text_style_rows(path).items() if r["name"] == "Standard"
    )
    assert _style_tables.text_style_font_xdata(path)[standard] == ("Arial", 34)


@pytest.mark.parametrize(
    "block",
    [
        (1, "AcadAnnotative", [(1000, "AnnotativeData")]),
        (1, "ACAD", [(1071, 34)]),
    ],
)
def test_text_style_font_xdata_skips_blocks_without_acad_family(monkeypatch, block):
    path = str(IMPERIAL)
    handle = next(iter(_style_tables.text_style_rows(path)))
    monkeypatch.setattr(raw, "decode_object_eed", lambda _path: [(handle, [block])])
    assert _style_tables.text_style_font_xdata(path) == {}


def test_replaced_file_gives_rows_and_diagnostics_of_a_fresh_path(tmp_path):
    def diagnostics(p):
        result = ezdwg.to_dwg(ezdwg.read(str(p)), str(tmp_path / "out.dwg"))
        return sorted((d.handle, d.dxftype, d.attribute, d.message) for d in result.diagnostics)

    reused = tmp_path / "reused.dwg"
    shutil.copy(MTEXT, reused)
    diagnostics(reused)
    _style_tables.linetype_rows(str(reused))
    shutil.copy(AC1015, reused)
    ezdwg.clear_decode_caches()

    fresh = tmp_path / "fresh.dwg"
    shutil.copy(AC1015, fresh)
    assert diagnostics(reused) == diagnostics(fresh)
    assert _style_tables.linetype_rows(str(reused)) == _style_tables.linetype_rows(str(fresh))
    assert _style_tables.text_style_rows(str(reused)) == _style_tables.text_style_rows(str(fresh))


@pytest.mark.parametrize(
    "value, expected",
    [
        (2.5, 2.5),
        (2, 2),
        (0, 1.0),
        (-1, 1.0),
        (float("nan"), 1.0),
        (float("inf"), 1.0),
        (None, 1.0),
        ("x", 1.0),
    ],
)
def test_clean_linetype_scale(value, expected):
    result = _style_tables.clean_linetype_scale(value)
    assert result == expected and type(result) is type(expected)
