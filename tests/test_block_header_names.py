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
    rows = ezdwg.raw.decode_insert_entities(str(SAMPLES / "acadsharp" / "sample_AC1032.dwg"))
    resolved = [name for *_rest, name in rows if name is not None]
    assert len(resolved) == len(rows)
    assert "my-dynamic-block" in resolved
    assert "my_block_v2" in resolved
    assert "*Model_Space" in resolved


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
