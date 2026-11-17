"""The acadsharp samples are one drawing saved in each DWG version, with one INSERT clipped by XCLIP, so the
clip boundary read from it must not depend on the version it was saved in."""

from __future__ import annotations

from pathlib import Path

import pytest

from ezdwg import _core

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg/acadsharp"
VERSIONS = ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"]


def sample(version: str) -> str:
    return str(SAMPLES / f"sample_{version}.dwg")


def rounded(value):
    if isinstance(value, float):
        return round(value, 9)
    if isinstance(value, (list, tuple)):
        return type(value)(rounded(item) for item in value)
    return value


@pytest.mark.parametrize("version", VERSIONS)
def test_a_spatial_filter_is_read_alike_whatever_the_version(version: str) -> None:
    """Points, plane, display flag, clipping distances and both transforms of the XCLIP boundary."""
    [reference] = _core.decode_spatial_filters(sample("AC1032"))
    _handle, _owner, points, extrusion, _origin, displayed, front, back, inverse, boundary = reference
    assert len(points) == 2 and extrusion == (0.0, 0.0, 1.0) and displayed
    assert (front, back) == (None, None)
    assert len(inverse) == 12 and len(boundary) == 12
    assert [rounded(row) for row in _core.decode_spatial_filters(sample(version))] == [rounded(reference)]


@pytest.mark.parametrize("version", VERSIONS[1:])
def test_a_spatial_filter_leads_back_to_the_insert_it_clips(version: str) -> None:
    """The filter's owner is the dictionary holding it as SPATIAL, whose owner is the ACAD_FILTER extension
    dictionary of an INSERT the INSERT decoder reads. R14 dictionaries are not read."""
    path = sample(version)
    [spatial_filter] = _core.decode_spatial_filters(path)
    dictionaries = {row[0]: row for row in _core.decode_dictionary_owners(path)}
    _handle, filter_owner, entries = dictionaries[spatial_filter[1]]
    assert entries == [("SPATIAL", spatial_filter[0])]
    _handle, insert, entries = dictionaries[filter_owner]
    assert entries == [("ACAD_FILTER", spatial_filter[1])]
    assert insert in {row[0] for row in _core.decode_insert_entities(path)}
