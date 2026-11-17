"""The acadsharp samples are one drawing saved in each DWG version, so what a decoder reads of it must not depend
on the version it was saved in."""

from __future__ import annotations

from pathlib import Path

import pytest

from ezdwg import _core

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg/acadsharp"


def sample(version: str) -> str:
    return str(SAMPLES / f"sample_{version}.dwg")


def rounded(value):
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, (list, tuple)):
        return type(value)(rounded(part) for part in value)
    return value


@pytest.mark.parametrize("version", ["AC1014", "AC1015", "AC1021", "AC1024", "AC1027", "AC1032"])
def test_tables_are_placed_alike_and_draw_a_table_block(version: str) -> None:
    rows = sorted(_core.decode_table_placements(sample(version)), key=lambda row: rounded(row[1]))
    reference = sorted(_core.decode_table_placements(sample("AC1032")), key=lambda row: rounded(row[1]))
    assert [rounded(row[1:5]) for row in rows] == [rounded(row[1:5]) for row in reference]
    names = {handle: name for handle, name in _core.decode_block_header_names(sample(version))}
    assert all(names.get(row[5], "").upper().startswith("*T") for row in rows)


@pytest.mark.parametrize("version", ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027"])
def test_hatch_boundaries_read_as_stored_alike(version: str) -> None:
    def boundaries(name: str) -> list:
        return sorted(str(rounded(paths)) for _, paths in _core.decode_hatch_boundaries(sample(name)))

    assert boundaries(version) == boundaries("AC1032")
    kinds = {path[0] for _, paths in _core.decode_hatch_boundaries(sample("AC1032")) for path in paths}
    assert kinds == {"polyline", "edges"}
