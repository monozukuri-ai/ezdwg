from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
from ezdwg.layers import Layer, LayerTable, LayerPropertyNotImplemented


ROOT = Path(__file__).resolve().parents[1]
AC1027_SAMPLE = ROOT / "test_dwg/acadsharp/sample_AC1027.dwg"
AC1032_SAMPLE = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"

# Ground truth independently verified against a from-source LibreDWG build
# (dwg_api.h / dwg.h, not ezdwg) -- see 1a_status_and_evidence.md and
# 1a_resolved.md for how this fixture and methodology were established.
# (frozen, off, frozen_in_new, locked, plotflag, lineweight_index)
GROUND_TRUTH = {
    16: (0, 0, 0, 0, 1, 31),
    629: (0, 0, 0, 0, 1, 31),
    632: (0, 1, 0, 0, 1, 31),
    633: (1, 0, 0, 0, 1, 31),
    634: (0, 0, 0, 1, 1, 31),
    635: (0, 0, 0, 0, 0, 31),
    636: (0, 0, 0, 0, 1, 31),
    637: (0, 0, 0, 0, 1, 31),
    641: (0, 0, 0, 0, 1, 9),
    642: (0, 0, 0, 0, 1, 31),
    643: (0, 0, 1, 0, 1, 31),
    644: (0, 0, 0, 0, 1, 31),
    645: (0, 0, 0, 0, 1, 31),
    646: (0, 0, 0, 0, 1, 31),
    1234: (0, 0, 0, 0, 0, 31),
    1235: (0, 0, 0, 0, 1, 31),
    1889: (0, 0, 0, 0, 1, 11),
    2293: (0, 0, 0, 0, 1, 31),
    2548: (0, 0, 0, 0, 1, 31),
}


@pytest.mark.parametrize("sample", [AC1027_SAMPLE, AC1032_SAMPLE])
def test_layers_match_ground_truth(sample: Path) -> None:
    assert sample.exists(), f"missing sample: {sample}"
    doc = ezdwg.read(str(sample))
    assert len(doc.layers) == len(GROUND_TRUTH)
    for handle, (frozen, off, frozen_in_new, locked, plotflag, lineweight_index) in GROUND_TRUTH.items():
        layer = doc.layers.by_handle(handle)
        assert layer.frozen == bool(frozen), layer.name
        assert layer.on == (not bool(off)), layer.name  # the on/off inversion specifically
        assert layer.frozen_in_new_viewports == bool(frozen_in_new), layer.name
        assert layer.locked == bool(locked), layer.name
        assert layer.plot == bool(plotflag), layer.name
        assert layer.lineweight_index == lineweight_index, layer.name


def test_layers_cached_on_document() -> None:
    doc = ezdwg.read(str(AC1027_SAMPLE))
    assert doc.layers is doc.layers


def test_layer_table_collection_protocol() -> None:
    doc = ezdwg.read(str(AC1027_SAMPLE))
    assert "Layer_Freeze" in doc.layers
    assert "NoSuchLayer" not in doc.layers
    assert doc.layers.get("NoSuchLayer") is None
    assert doc.layers.get("Layer_Freeze") is not None
    names_in_order = [layer.name for layer in doc.layers]
    assert names_in_order[0] == "0"  # layer "0" is always first, per DWG convention
    assert len(names_in_order) == len(doc.layers)


def test_layer_is_immutable() -> None:
    doc = ezdwg.read(str(AC1027_SAMPLE))
    layer = doc.layers["0"]
    with pytest.raises(Exception):
        layer.frozen = True  # type: ignore[misc]


def test_transparency_standard_and_description_fields() -> None:
    """transparency / standard / description are plain Optional fields from EED
    (AcCmTransparency / AcAecLayerStandard). Missing XDATA => None."""
    doc = ezdwg.read(str(AC1027_SAMPLE))
    layer = doc.layers["0"]
    assert layer.transparency is None or isinstance(layer.transparency, float)
    assert layer.standard is None or isinstance(layer.standard, str)
    assert layer.description is None or isinstance(layer.description, str)
    assert isinstance(layer.lineweight, int)
    assert layer.linetype is None or isinstance(layer.linetype, str)


def test_lineweight_standard_table_known_values() -> None:
    """Cross-checked against the original findings doc's known-good layers,
    same fixture used for the flags ground truth."""
    doc = ezdwg.read(str(AC1027_SAMPLE))
    assert doc.layers["Layer_lw_035"].lineweight == 35
    assert doc.layers["Layer_lw_050"].lineweight == 50
    assert doc.layers["0"].lineweight == ezdwg.lineweight.DEFAULT  # index 31, unset
    assert doc.layers.lineweight_flagged_handles == []


def test_lineweight_custom_table() -> None:
    custom = {9: 12345}
    doc = ezdwg.read(str(AC1027_SAMPLE), lineweight_table=custom)
    assert doc.layers["Layer_lw_035"].lineweight_index == 9
    assert doc.layers["Layer_lw_035"].lineweight == 12345  # overridden
    assert doc.layers["Layer_lw_050"].lineweight == 50  # unaffected, falls through to standard


def test_lineweight_out_of_range_wraps_and_flags(caplog) -> None:
    from ezdwg.lineweight import _lookup

    with caplog.at_level("ERROR", logger="ezdwg.lineweight"):
        value, out_of_range = _lookup(35, None)
    assert out_of_range is True
    assert value == ezdwg.lineweight.STANDARD_TABLE[35 % 32]
    assert any("out of the valid 0-31 range" in r.message for r in caplog.records)


def test_lineweight_custom_table_validation_warns(caplog) -> None:
    with caplog.at_level("WARNING", logger="ezdwg.lineweight"):
        ezdwg.lineweight.validate_custom_table({40: 100})
    assert any("outside the valid 0-31 range" in r.message for r in caplog.records)


def test_rgb_derived_from_true_color() -> None:
    doc = ezdwg.read(str(AC1027_SAMPLE))
    layer = doc.layers["0"]
    if layer.true_color is None:
        assert layer.rgb is None
    else:
        assert layer.rgb == (
            (layer.true_color >> 16) & 0xFF,
            (layer.true_color >> 8) & 0xFF,
            layer.true_color & 0xFF,
        )


def test_to_dict_and_to_records_shape() -> None:
    doc = ezdwg.read(str(AC1027_SAMPLE))
    layer = doc.layers["0"]
    d = layer.to_dict()
    # Core identity + appearance keys must be present. Extra decoded fields
    # (linetype, transparency, description, handles, eed, …) may also appear.
    required_keys = {
        "handle", "name", "color", "true_color", "rgb", "on", "frozen",
        "locked", "frozen_in_new_viewports", "plot", "lineweight_index",
        "lineweight_index_out_of_range", "lineweight",
        "transparency", "standard", "description",
    }
    assert required_keys.issubset(set(d.keys()))
    assert "transparency" in d and "standard" in d and "description" in d

    records = doc.layers.to_records()
    assert len(records) == len(doc.layers)
    assert all(required_keys.issubset(set(r.keys())) for r in records)


@pytest.mark.parametrize(
    "sample_file",
    [
        "test_dwg/insert_2004.dwg",
        "test_dwg/line_2000.dwg",
        "examples/data/arc_2000.dwg",
        "test_dwg/mtext_2000.dwg",
        "test_dwg/text_2000.dwg",
        "examples/data/polyline2d_line_2000.dwg",
    ],
)
def test_layers_no_regression_on_simple_fixtures(sample_file: str) -> None:
    """These simpler fixtures were the ones used while fixing the
    underlying flag0 decoding bug -- kept here as a regression check that
    the high-level wrapper doesn't crash or lose layers on them.

    Deliberately NOT asserting on layer.name being non-empty here: several
    of these R2000 fixtures return an empty name from decode_layer_names
    today (a pre-existing, separate bug -- see "Priority 1b" / the
    STRING_STREAM_METADATA_BITS issue in ezdwg_layer_extension_plan.md).
    That's a name-decoding problem, not something this layer-API wrapper
    introduced or is scoped to fix -- asserting past it here would either
    mask a real bug or misattribute it to this code.
    """
    path = ROOT / sample_file
    assert path.exists(), f"missing sample: {path}"
    doc = ezdwg.read(str(path))
    assert len(doc.layers) >= 1
    for layer in doc.layers:
        assert isinstance(layer, Layer)
        assert layer.handle > 0
        assert isinstance(layer.on, bool)
