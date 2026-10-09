from dataclasses import replace
from pathlib import Path
import os

import pytest

import ezdwg
from ezdwg.layer_states import (
    LayerStateMasks,
    build_viewport_override_table,
    _split_state_groups,
)


ROOT = Path(__file__).resolve().parents[1]
SAMPLE = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"


def saved_entry(document, name, mask, **changes):
    state = document.layer_states.states[0]
    entry = next(e for e in state.entries if e.name == name)
    return replace(state, mask=mask, entries=(replace(entry, **changes),))


def test_saved_lineweight_uses_dxf_units_and_restores_enum():
    doc = ezdwg.read(str(SAMPLE))
    state = saved_entry(doc, "Layer_lw_035", LayerStateMasks.LINE_WEIGHT)
    assert state.entries[0].lineweight_index == 9
    live = doc.layers["Layer_lw_035"]
    doc.layers.replace_layer(replace(live, lineweight_index=11, lineweight=50))
    assert state.diff(doc).summary["changed"] == 1
    assert state.apply(doc).summary["changed"] == 0
    assert doc.layers[live.name].lineweight == 35
    assert doc.layers[live.name].lineweight_index == 9


def test_saved_true_color_matches_live_fixture():
    doc = ezdwg.read(str(SAMPLE))
    state = saved_entry(doc, "Layer_true_color", LayerStateMasks.COLOR)
    assert state.entries[0].true_color == doc.layers["Layer_true_color"].true_color
    assert state.entries[0].true_color is not None


def test_apply_aci_clears_true_color():
    doc = ezdwg.read(str(SAMPLE))
    state = saved_entry(
        doc, "Layer_true_color", LayerStateMasks.COLOR, color=2, true_color=None
    )
    assert "true_color" in state.diff(doc).entries[0].changes
    assert state.apply(doc).summary["changed"] == 0
    layer = doc.layers["Layer_true_color"]
    assert layer.color == 2
    assert layer.true_color is None
    assert layer.rgb is None


def test_apply_linetype_updates_name_and_handle():
    doc = ezdwg.read(str(SAMPLE))
    source = doc.layers["Layer_Lt_dash"]
    state = saved_entry(
        doc,
        "0",
        LayerStateMasks.LINE_TYPE,
        linetype=source.linetype,
        linetype_handle=source.ltype_handle,
    )
    assert state.apply(doc).summary["changed"] == 0
    assert doc.layers["0"].linetype == source.linetype
    assert doc.layers["0"].ltype_handle == source.ltype_handle


def test_apply_keeps_document_custom_lineweight_table():
    doc = ezdwg.read(str(SAMPLE), lineweight_table={9: 12345})
    state = saved_entry(doc, "Layer_lw_035", LayerStateMasks.LINE_WEIGHT)
    state.apply(doc)
    assert doc.layers["Layer_lw_035"].lineweight == 12345


def test_viewport_apply_keeps_fractional_transparency_and_global_layer():
    doc = ezdwg.read(str(SAMPLE))
    state = saved_entry(doc, "0", LayerStateMasks.TRANSPARENCY, transparency=70.2)
    original = doc.layers["0"]
    assert state.apply(doc, viewport=123).summary["changed"] == 0
    assert (
        doc.viewport_overrides.get_property(original.handle, 123, "transparency")
        == 70.2
    )
    assert doc.layers["0"] is original


def test_viewport_override_values_are_normalized(monkeypatch):
    monkeypatch.setattr(ezdwg.raw, "decode_viewport_details", lambda _: [])
    monkeypatch.setattr(
        ezdwg.raw,
        "decode_layer_vp_overrides",
        lambda _: [
            (16, "0", "color", 123, 0xC2000000 - 2**32),
            (17, "Other", "color", 123, 0xC3000002 - 2**32),
            (16, "0", "lineweight", 123, 35),
            (16, "0", "transparency", 123, 0x0200007F),
        ],
    )
    table = build_viewport_override_table("unused.dwg")
    assert table.get_property(16, 123, "true_color") == 0
    assert table.get_property(17, 123, "color") == 2
    assert table.properties[(17, 123)]["true_color"] is None
    assert table.get_property(16, 123, "lineweight") == 35
    assert table.get_property(16, 123, "transparency") == pytest.approx(50.2)


def test_missing_layer_strict_apply_is_atomic():
    doc = ezdwg.read(str(SAMPLE))
    state = saved_entry(doc, "0", LayerStateMasks.COLOR, color=2)
    missing = replace(state.entries[0], name="missing", layer_handle=999999)
    state = replace(state, entries=state.entries + (missing,))
    original = doc.layers["0"]
    with pytest.raises(KeyError, match="missing"):
        state.apply(doc, skip_missing=False)
    assert doc.layers["0"] is original


def test_handle_and_name_in_same_state_block():
    groups = [(91, 32), (330, 16), (8, "0"), (62, 2), (330, 17), (62, 3)]
    header, blocks = _split_state_groups(groups)
    assert header == [(91, 32)]
    assert blocks == [groups[1:4], groups[4:]]


def test_unnamed_layer_retains_legacy_fallback(monkeypatch):
    from ezdwg.layers import build_layer_table

    monkeypatch.setattr(ezdwg.raw, "decode_layer_names", lambda _: [(16, "")])
    table = build_layer_table(str(SAMPLE))
    assert table.by_handle(16).name == "LAYER_10"
    assert table()["LAYER_10"]["handle"] == 16


def test_populated_property_filter_and_nested_tree(monkeypatch):
    from ezdwg.layer_filters import build_layer_filter_table, build_layer_filter_tree

    monkeypatch.setattr(
        ezdwg.raw,
        "decode_layer_filter_xrecords",
        lambda _: [
            ("Walls", 100, None, 0, [(1, "Walls"), (1, "AR*")]),
        ],
    )
    table = build_layer_filter_table("unused.dwg")
    assert table.get("Walls").matches_name("ar-wall")
    assert not table.get("Walls").matches_name("Other")
    monkeypatch.setattr(
        ezdwg.raw,
        "decode_layer_filter_tree",
        lambda _: [
            ("Root", 101, None, "root", 0, 1, []),
            ("Walls", 102, 'NAME == "AR*"', "child", 1, 0, []),
        ],
    )
    tree = build_layer_filter_tree("unused.dwg")
    assert tree.root_count == 1
    assert len(tree) == 2
    assert tree.roots[0].children[0].name == "Walls"
    assert tree.get("Walls").matches_name("AR-WALL")


def test_native_cache_distinguishes_relative_paths_after_chdir(tmp_path, monkeypatch):
    sources = [
        ROOT / "examples/data/line_2000.dwg",
        ROOT / "examples/data/mechanical_example-imperial.dwg",
    ]
    data = [path.read_bytes() for path in sources]
    size = max(map(len, data))
    paths = []
    for i, payload in enumerate(data):
        directory = tmp_path / str(i)
        directory.mkdir()
        path = directory / "drawing.dwg"
        path.write_bytes(payload.ljust(size, b"\0"))
        os.utime(path, ns=(1_800_000_000_000_000_000,) * 2)
        paths.append(path)
    expected = [ezdwg.raw.decode_layer_names(str(path)) for path in paths]
    assert expected[0] != expected[1]
    ezdwg.clear_decode_caches()
    for path, names in zip(paths, expected):
        monkeypatch.chdir(path.parent)
        assert ezdwg.raw.decode_layer_names("drawing.dwg") == names
    ezdwg.clear_decode_caches()


def test_native_cache_invalidates_changed_file_and_clear(tmp_path):
    first = ROOT / "examples/data/line_2000.dwg"
    second = ROOT / "examples/data/mechanical_example-imperial.dwg"
    path = tmp_path / "drawing.dwg"
    path.write_bytes(first.read_bytes())
    os.utime(path, ns=(1_800_000_000_000_000_000,) * 2)
    assert ezdwg.raw.decode_layer_names(str(path)) == ezdwg.raw.decode_layer_names(
        str(first)
    )
    path.write_bytes(second.read_bytes())
    os.utime(path, ns=(1_800_000_001_000_000_000,) * 2)
    expected = ezdwg.raw.decode_layer_names(str(second))
    assert ezdwg.raw.decode_layer_names(str(path)) == expected
    ezdwg.clear_decode_caches()
    assert ezdwg.raw.decode_layer_names(str(path)) == expected
