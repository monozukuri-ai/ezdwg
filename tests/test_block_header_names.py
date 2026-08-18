from __future__ import annotations

from pathlib import Path

import ezdwg


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def test_decode_block_header_names_r18_contains_blk1() -> None:
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "insert_2004.dwg"))
    names = {name for _handle, name in rows}
    assert "BLK1" in names


def test_decode_block_header_names_r2018_contains_named_block() -> None:
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    names = {name for _handle, name in rows}
    assert "MyBlock" in names


def test_decode_block_header_names_r2018_extracts_dynamic_block_names() -> None:
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    names = {name for _handle, name in rows}
    assert "my-dynamic-block" in names
    assert "my_block_v2" in names
    assert "My dynamic block description." not in names


def test_decode_block_header_names_r2018_contains_model_space() -> None:
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    names = {name for _handle, name in rows}
    assert "*Model_Space" in names


def test_decode_insert_entities_r2018_resolves_some_block_names() -> None:
    # Ground truth from the ACadSharp DXF twin (acad-ts/samples/sample_AC1032_ascii.dxf):
    # 14 INSERTs referencing MyBlock, my_block_v2 (x2), my_block (x4), _ArchTick (x2),
    # _BoxBlank (x2) and the anonymous representation blocks of the dynamic block
    # ("*U..." in DXF; the DWG stores the bare "*U" and AutoCAD numbers them on load).
    # Dynamic-block instances reference those representation blocks, not
    # "my-dynamic-block" itself (that link lives in AcDbBlockRepBTag xdata).
    rows = ezdwg.raw.decode_insert_entities(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    resolved = [name for *_rest, name in rows if name is not None]
    assert len(rows) == 14
    assert len(resolved) == len(rows)
    counts = {name: resolved.count(name) for name in set(resolved)}
    assert counts["MyBlock"] == 1
    assert counts["my_block_v2"] == 2
    assert counts["my_block"] == 4
    assert counts["_ArchTick"] == 2
    assert counts["_BoxBlank"] == 2
    # anonymous representation blocks are numbered uniquely ("*U8", "*U9", ...)
    anonymous = [name for name in resolved if name.startswith("*U")]
    assert len(anonymous) == 3 and len(set(anonymous)) == 3
    assert "*Model_Space" not in resolved


def test_decode_block_entity_names_r18_contains_block_and_endblk_names() -> None:
    rows = ezdwg.raw.decode_block_entity_names(str(SAMPLES / "insert_2004.dwg"))
    block_names = {name for _handle, type_name, name in rows if type_name == "BLOCK"}
    endblk_names = {name for _handle, type_name, name in rows if type_name == "ENDBLK"}
    assert "BLK1" in block_names
    assert "BLK1" in endblk_names


def test_decode_block_entity_names_r2018_contains_dynamic_names_on_both_sides() -> None:
    rows = ezdwg.raw.decode_block_entity_names(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    block_names = {name for _handle, type_name, name in rows if type_name == "BLOCK"}
    endblk_names = {name for _handle, type_name, name in rows if type_name == "ENDBLK"}
    assert "my-dynamic-block" in block_names
    assert "my-dynamic-block" in endblk_names
    assert "my_block_v2" in block_names
    assert "my_block_v2" in endblk_names


def test_decode_block_entity_name_maps_r2018_contains_dynamic_names_on_both_sides() -> None:
    block_rows, endblk_rows = ezdwg.raw.decode_block_entity_name_maps(
        str(SAMPLES / "acadsharp" / "sample_AC1032.dwg")
    )
    block_names = {name for _handle, name in block_rows}
    endblk_names = {name for _handle, name in endblk_rows}
    assert "my-dynamic-block" in block_names
    assert "my-dynamic-block" in endblk_names


def test_decode_block_header_names_r2007_reads_string_stream_names() -> None:
    # R2007 (AC1021) keeps names in the string stream like R2010+; the RL
    # "size in bits" at the top of the object is the data end.
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "acadsharp" / "sample_AC1021.dwg"))
    names = {name for _handle, name in rows}
    assert {"*Model_Space", "*Paper_Space", "MyBlock", "my_block_v2", "my_block", "_ArchTick"} <= names
    assert not any(name.startswith("{") for name in names)


def test_decode_insert_entities_r2007_resolves_block_names() -> None:
    rows = ezdwg.raw.decode_insert_entities(str(SAMPLES / "acadsharp" / "sample_AC1021.dwg"))
    resolved = [name for *_rest, name in rows if name is not None]
    assert len(rows) == 14
    assert len(resolved) == len(rows)
    counts = {name: resolved.count(name) for name in set(resolved)}
    assert counts["MyBlock"] == 1
    assert counts["my_block_v2"] == 2
    assert counts["my_block"] == 4
    assert counts["_ArchTick"] == 2
    assert counts["_BoxBlank"] == 2
    anonymous = [name for name in resolved if name.startswith("*U")]
    assert len(anonymous) == 3 and len(set(anonymous)) == 3


def test_anonymous_block_names_are_unique_and_numbered() -> None:
    rows = ezdwg.raw.decode_block_header_names(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    names = [name for _handle, name in rows]
    # the DWG stores bare "*D"/"*U"/"*T"; every header must come out numbered
    assert not any(len(name) == 2 and name.startswith("*") for name in names)
    anonymous_headers = {name for name in names if name.startswith("*") and name[1:2].isalpha() and name[2:].isdigit()}
    assert {"*D1", "*U8"} <= anonymous_headers
