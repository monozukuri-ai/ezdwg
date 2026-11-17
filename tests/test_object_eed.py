"""`decode_object_eed` reads the XDATA of every object as the DXF AutoCAD saved from the same drawing holds it."""

from __future__ import annotations

from pathlib import Path

import pytest

ezdxf = pytest.importorskip("ezdxf")

from ezdwg import _core  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
PAIRED = sorted(path for path in (ROOT / "test_dwg").glob("*.dxf") if path.with_suffix(".dwg").is_file())


def dxf_xdata(path: Path) -> dict[tuple[int, str], list[tuple[int, object]]]:
    """XDATA per (handle, application) as the DXF holds it, handles as integers and points as tuples."""
    doc = ezdxf.readfile(path)
    found = {}
    for entity in doc.entitydb.values():
        if not entity.xdata:
            continue
        for app, tags in entity.xdata.data.items():
            found[(int(entity.dxf.handle, 16), app)] = [(tag.code, normalized(tag.code, tag.value)) for tag in tags]
    return found


def normalized(code: int, value: object) -> object:
    if code in (1003, 1005) and isinstance(value, str):
        return int(value, 16)
    if code == 1002:
        return value
    if isinstance(value, float):
        return round(value, 9)
    if not isinstance(value, (str, int, bytes)):
        return tuple(round(float(part), 9) for part in value)
    return value


def dwg_xdata(path: Path) -> dict[tuple[int, str], list[tuple[int, object]]]:
    found = {}
    for handle, blocks in _core.decode_object_eed(str(path)):
        for _app_handle, app, items in blocks:
            found[(handle, app)] = [(1001, app)] + [
                (code, ("{" if value == 0 else "}") if code == 1002 else normalized(code, value))
                for code, value in items
            ]
    return found


@pytest.mark.parametrize("dxf", PAIRED, ids=lambda path: path.stem)
def test_every_saved_xdata_block_is_read_with_its_values(dxf: Path) -> None:
    saved = dxf_xdata(dxf)
    read = dwg_xdata(dxf.with_suffix(".dwg"))
    assert saved, "the sample's DXF holds no XDATA to compare"
    matched = {key: read[key] for key in saved if key in read}
    # A DXF writer may add an ACAD block of its own (dimension style overrides) to an object the DWG stores none on.
    missing = {key for key in saved if key not in read and key[1] != "ACAD"}
    assert not missing
    assert matched == {key: saved[key] for key in matched}


@pytest.mark.parametrize("version", ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"])
def test_a_handle_value_is_an_object_of_the_file(version: str) -> None:
    """Group 1005 holds a handle, which names an object of the same file whatever the file's version."""
    sample = str(ROOT / f"test_dwg/acadsharp/sample_{version}.dwg")
    handles = {row[0] for row in _core.list_object_map_entries(sample)}
    referenced = [
        value
        for _handle, blocks in _core.decode_object_eed(sample)
        for _app_handle, _app, items in blocks
        for code, value in items
        if code == 1005
    ]
    assert referenced
    assert all(value in handles for value in referenced)
