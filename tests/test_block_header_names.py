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
