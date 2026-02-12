from __future__ import annotations

import math
from pathlib import Path

import ezdwg
from ezdwg import raw


ROOT = Path(__file__).resolve().parents[1]
SMALL_AC1032 = ROOT / "test_dwg/acadsharp/BLOCKPOINTPARAMETER.dwg"
LARGE_AC1032 = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"


def _assert_finite_rows(rows: list[tuple[int, float, float, float, float, float, float]]) -> None:
    for row in rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)


def _assert_finite_arc_rows(
    rows: list[tuple[int, float, float, float, float, float, float]],
) -> None:
    for row in rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)


def _assert_finite_circle_rows(rows: list[tuple[int, float, float, float, float]]) -> None:
    for row in rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)


def test_ac1032_small_bulk_decode_matches_per_type_decode() -> None:
    assert SMALL_AC1032.exists(), f"missing sample: {SMALL_AC1032}"

    line_rows, arc_rows, circle_rows = raw.decode_line_arc_circle_entities(
        str(SMALL_AC1032), limit=64
    )
    expected_line_rows = raw.decode_line_entities(str(SMALL_AC1032), limit=64)
    expected_arc_rows = raw.decode_arc_entities(str(SMALL_AC1032), limit=64)
    expected_circle_rows = raw.decode_circle_entities(str(SMALL_AC1032), limit=64)

    assert line_rows == expected_line_rows
    assert arc_rows == expected_arc_rows
    assert circle_rows == expected_circle_rows
    assert len(line_rows) >= 1
    assert len(circle_rows) >= 1

    _assert_finite_rows(line_rows)
    _assert_finite_arc_rows(arc_rows)
    _assert_finite_circle_rows(circle_rows)


def test_ac1032_small_high_level_query_counts_match_raw_decode() -> None:
    assert SMALL_AC1032.exists(), f"missing sample: {SMALL_AC1032}"

    doc = ezdwg.read(str(SMALL_AC1032))
    modelspace = doc.modelspace()

    line_count = sum(1 for _ in modelspace.query("LINE"))
    circle_count = sum(1 for _ in modelspace.query("CIRCLE"))

    assert doc.version == "AC1032"
    assert line_count == len(raw.decode_line_entities(str(SMALL_AC1032), limit=1000))
    assert circle_count == len(raw.decode_circle_entities(str(SMALL_AC1032), limit=1000))
    assert line_count >= 1
    assert circle_count >= 1


def test_ac1032_large_bulk_decode_smoke() -> None:
    assert LARGE_AC1032.exists(), f"missing sample: {LARGE_AC1032}"

    line_rows, arc_rows, circle_rows = raw.decode_line_arc_circle_entities(
        str(LARGE_AC1032), limit=128
    )

    assert len(line_rows) > 0
    assert len(arc_rows) > 0
    assert len(circle_rows) > 0

    _assert_finite_rows(line_rows)
    _assert_finite_arc_rows(arc_rows)
    _assert_finite_circle_rows(circle_rows)
