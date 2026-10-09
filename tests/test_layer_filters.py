"""Layer filter discovery tests against in-repo DWG fixtures.

Public acadsharp samples currently ship with empty filter tables; these tests
assert the APIs are callable and return well-typed empty results, and that
Document wiring works.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from ezdwg import raw, read

ROOT = Path(__file__).resolve().parents[1]
AC1032 = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
LINE_2000 = ROOT / "test_dwg/line_2000.dwg"


@pytest.fixture(scope="module")
def sample() -> Path:
    assert AC1032.is_file(), f"missing in-repo fixture: {AC1032}"
    return AC1032


def test_decode_layer_filter_names_typed(sample: Path) -> None:
    names = raw.decode_layer_filter_names(str(sample))
    assert isinstance(names, list)
    assert all(isinstance(n, str) for n in names)


def test_decode_layer_filter_xrecords_typed(sample: Path) -> None:
    rows = raw.decode_layer_filter_xrecords(str(sample))
    assert isinstance(rows, list)


def test_decode_layer_filter_tree_typed(sample: Path) -> None:
    nodes = raw.decode_layer_filter_tree(str(sample))
    assert isinstance(nodes, list)


def test_build_layer_filter_table_empty_ok(sample: Path) -> None:
    from ezdwg.layer_filters import build_layer_filter_table

    table = build_layer_filter_table(str(sample))
    assert table is not None
    assert len(table) == len(raw.decode_layer_filter_names(str(sample)))


def test_build_layer_filter_tree_empty_ok(sample: Path) -> None:
    from ezdwg.layer_filters import build_layer_filter_tree

    tree = build_layer_filter_tree(str(sample))
    assert tree is not None


def test_document_layer_filters_wiring(sample: Path) -> None:
    doc = read(str(sample))
    assert hasattr(doc, "layer_filters")
    assert hasattr(doc, "layer_filter_tree")
    _ = doc.layer_filters
    _ = doc.layer_filter_tree


def test_drawing_without_filters() -> None:
    assert LINE_2000.is_file(), f"missing in-repo fixture: {LINE_2000}"
    assert raw.decode_layer_filter_names(str(LINE_2000)) == []
    assert raw.decode_layer_filter_xrecords(str(LINE_2000)) == []
    assert raw.decode_layer_filter_tree(str(LINE_2000)) == []
