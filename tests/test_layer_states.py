"""Layer states tests against in-repo DWG fixtures.

Primary fixture: ``test_dwg/acadsharp/sample_AC1032.dwg`` (one
``ACAD_VIEWS_view_custom`` state with per-layer entries).
Empty-state smoke: ``test_dwg/line_2000.dwg``.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from ezdwg import raw, read

ROOT = Path(__file__).resolve().parents[1]
AC1032 = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
AC1027 = ROOT / "test_dwg/acadsharp/sample_AC1027.dwg"
LINE_2000 = ROOT / "test_dwg/line_2000.dwg"
# Known state on the acadsharp AC1027/AC1032 samples.
KNOWN_STATE = "ACAD_VIEWS_view_custom"


@pytest.fixture(scope="module")
def sample() -> Path:
    assert AC1032.is_file(), f"missing in-repo fixture: {AC1032}"
    return AC1032


def test_sample_is_ac1032(sample: Path) -> None:
    assert raw.detect_version(str(sample)) == "AC1032"


def test_decode_layer_state_names(sample: Path) -> None:
    names = raw.decode_layer_state_names(str(sample))
    assert isinstance(names, list)
    assert KNOWN_STATE in names
    assert all(isinstance(n, str) and n for n in names)
    assert not any(n.startswith("Unnamed") or n == "" for n in names)


def test_decode_layer_state_xrecords_shape(sample: Path) -> None:
    rows = raw.decode_layer_state_xrecords(str(sample))
    assert len(rows) >= 1
    for row in rows:
        assert len(row) == 7
        name, xh, mask, desc, groups, objids, xraw = row
        assert isinstance(name, str) and name
        assert isinstance(xh, int) and xh > 0
        assert mask is None or isinstance(mask, int)
        assert desc is None or isinstance(desc, str)
        assert isinstance(groups, list) and len(groups) > 0
        assert isinstance(objids, list)
        assert isinstance(xraw, (bytes, bytearray, list))


def test_xrecord_has_per_layer_groups(sample: Path) -> None:
    rows = raw.decode_layer_state_xrecords(str(sample))
    live = [r for r in rows if r[0] == KNOWN_STATE]
    assert live, f"missing state {KNOWN_STATE!r}"
    codes = {g[0] for g in live[0][4]}
    # Header + per-layer property groups used by the AC1032 schema.
    assert 91 in codes or 330 in codes
    assert 90 in codes
    assert 62 in codes
    assert 370 in codes


def test_drawing_without_layer_states() -> None:
    assert LINE_2000.is_file(), f"missing in-repo fixture: {LINE_2000}"
    names = raw.decode_layer_state_names(str(LINE_2000))
    rows = raw.decode_layer_state_xrecords(str(LINE_2000))
    assert names == []
    assert rows == []


def test_names_match_xrecord_rows(sample: Path) -> None:
    names = set(raw.decode_layer_state_names(str(sample)))
    rows = raw.decode_layer_state_xrecords(str(sample))
    row_names = {r[0] for r in rows}
    assert names == row_names


def test_build_layer_state_table(sample: Path) -> None:
    from ezdwg.layer_states import build_layer_state_table

    table = build_layer_state_table(str(sample))
    assert len(table) >= 1
    assert KNOWN_STATE in table.names()
    st = table.get(KNOWN_STATE)
    assert st is not None
    assert int(st.mask) > 0
    assert len(st.entries) >= 1
    assert any(e.color is not None for e in st.entries)
    assert any(e.flags is not None for e in st.entries)


def test_entry_flag_semantics(sample: Path) -> None:
    from ezdwg.layer_states import build_layer_state_table

    st = build_layer_state_table(str(sample)).get(KNOWN_STATE)
    assert st is not None
    # sample_AC1032 exercises off / frozen / frozen-in-new-VP bits.
    assert any(not e.on for e in st.entries)
    assert any(e.frozen for e in st.entries)
    assert any(e.frozen_in_new_viewports for e in st.entries)


def test_document_layer_states_property(sample: Path) -> None:
    doc = read(str(sample))
    table = doc.layer_states
    assert table is not None
    assert len(table) >= 1
    assert KNOWN_STATE in table.names()


def test_diff_against_live_layers(sample: Path) -> None:
    from ezdwg.layer_states import build_layer_state_table

    doc = read(str(sample))
    st = build_layer_state_table(str(sample)).get(KNOWN_STATE)
    assert st is not None
    result = st.diff(doc)
    assert result.summary["matched"] + result.summary["changed"] >= 1
    assert result.summary["missing_in_drawing"] >= 0


def test_plotstyles_table(sample: Path) -> None:
    styles = raw.decode_plotstyles(str(sample))
    assert isinstance(styles, list)
    assert any(name == "Normal" for name, _handle in styles)


def test_ac1027_also_has_view_custom_state() -> None:
    assert AC1027.is_file(), f"missing in-repo fixture: {AC1027}"
    names = raw.decode_layer_state_names(str(AC1027))
    assert KNOWN_STATE in names
    rows = raw.decode_layer_state_xrecords(str(AC1027))
    assert any(r[0] == KNOWN_STATE for r in rows)


def test_current_viewport_header_fields(sample: Path) -> None:
    from ezdwg.layer_states import build_layer_state_table

    st = build_layer_state_table(str(sample)).get(KNOWN_STATE)
    assert st is not None
    assert st.current_viewport is False
    assert st.viewport_code is not None


def test_viewport_override_api_smoke(sample: Path) -> None:
    """Public samples may have zero OVRs; the API must still return a list."""
    rows = raw.decode_layer_vp_overrides(str(sample))
    assert isinstance(rows, list)


def test_viewport_entities_present(sample: Path) -> None:
    entities = raw.decode_viewport_entities(str(sample))
    assert isinstance(entities, list)
    assert len(entities) >= 1
