from __future__ import annotations

import os
from pathlib import Path
import shutil

import pytest

import ezdwg
import ezdwg.document as document_module
from ezdwg import raw


SAMPLES = Path(__file__).resolve().parents[1] / "test_dwg"


def test_high_level_catalog_helpers_do_not_request_full_headers(monkeypatch) -> None:
    path = str(SAMPLES / "line_2000.dwg")
    ezdwg.clear_decode_caches()
    headers = raw.list_object_headers_with_type(path)
    expected = tuple(row[0] for row in headers if row[4].strip().upper() == "LINE")
    calls = []

    def full_headers_forbidden(*_args, **_kwargs):
        calls.append(True)
        raise AssertionError("compact helpers must not request all headers")

    monkeypatch.setattr(raw, "list_object_headers_with_type", full_headers_forbidden)
    assert "LINE" in document_module._present_supported_types(path)
    assert document_module._entity_handles_by_type_name(path, " line ") == expected
    assert calls == []


@pytest.mark.parametrize(
    "sample",
    [
        "line_R14.dwg",
        "line_2000.dwg",
        "line_2004.dwg",
        "line_2007.dwg",
        "line_2010.dwg",
        "line_2013.dwg",
        "acadsharp/sample_AC1032.dwg",
    ],
)
def test_typed_catalog_preserves_headers_records_and_limits(sample: str) -> None:
    path = str(SAMPLES / sample)
    ezdwg.clear_decode_caches()
    # Start with a limited typed entity read on a cold catalog.
    limited_lines = raw.decode_line_entities(path, limit=1)
    headers = raw.list_object_headers_with_type(path)
    expected = [row for row in headers if row[3] in {0x13, 0x12}]
    assert expected
    assert raw.list_object_headers_by_type(path, [0x13, 0x12, 0x13]) == expected
    assert raw.list_object_headers_by_type(path, [0x13, 0x12], limit=1) == expected[:1]
    assert raw.list_object_headers(path) == [row[:4] for row in headers]
    assert raw.list_object_headers_with_type(path, limit=1) == headers[:1]

    records = raw.read_object_records_by_type(path, [0x13, 0x12])
    assert [row[:4] for row in records] == [row[:4] for row in expected]
    assert records == raw.read_object_records_by_handle(path, [row[0] for row in records])
    assert limited_lines == raw.decode_line_entities(path)[:1]
    assert limited_lines == raw.decode_line_entities(path, limit=1)


@pytest.mark.parametrize(
    "sample",
    [
        "line_R14.dwg",
        "line_2000.dwg",
        "line_2004.dwg",
        "line_2007.dwg",
        "line_2010.dwg",
        "line_2013.dwg",
        "acadsharp/sample_AC1032.dwg",
    ],
)
def test_compact_catalog_matches_full_header_views(sample: str) -> None:
    path = str(SAMPLES / sample)
    ezdwg.clear_decode_caches()
    names = raw.list_object_type_names(path)  # initialize from the cold catalog
    headers = raw.list_object_headers_with_type(path)
    assert names == list(dict.fromkeys(row[4] for row in headers))
    for name in names:
        expected = [row[0] for row in headers if row[4].strip().upper() == name.strip().upper()]
        assert raw.list_object_handles_by_type_name(path, name) == expected
        assert raw.list_object_handles_by_type_name(path, f" {name.lower()} ", limit=1) == expected[:1]
        assert raw.list_object_handles_by_type_name(path, name, limit=0) == []
    assert raw.list_object_handles_by_type_name(path, "no_such_type") == []
    assert raw.list_object_handles_by_type_name(path, " ") == []


def test_catalog_invalidation_updates_typed_geometry(tmp_path: Path) -> None:
    path = tmp_path / "drawing.dwg"
    shutil.copyfile(SAMPLES / "line_2000.dwg", path)
    assert raw.decode_line_entities(str(path))
    assert "LINE" in raw.list_object_type_names(str(path))
    assert raw.list_object_handles_by_type_name(str(path), "LINE")
    assert not raw.decode_circle_entities(str(path))
    shutil.copyfile(SAMPLES / "circle_2007.dwg", path)
    assert not raw.decode_line_entities(str(path))
    assert "LINE" not in raw.list_object_type_names(str(path))
    assert raw.list_object_handles_by_type_name(str(path), "LINE") == []
    assert raw.list_object_handles_by_type_name(str(path), "CIRCLE")
    circles = raw.decode_circle_entities(str(path))
    assert circles
    raw.clear_decode_cache()
    assert raw.decode_circle_entities(str(path)) == circles


def test_same_size_replacement_invalidates_headers_and_geometry(tmp_path: Path) -> None:
    path = tmp_path / "drawing.dwg"
    drawing = ezdwg.new("AC1015")
    line = drawing.modelspace().add_line((1.5, 2.5, 3.5), (4.5, 5.5, 6.5))
    drawing.save(path)
    original = raw.decode_line_entities(str(path))
    before = path.stat()
    drawing.modelspace().update(line, start=(1.75, 2.5, 3.5))
    drawing.save(path)
    assert path.stat().st_size == before.st_size
    stamp = before.st_mtime_ns + 1_000_000_000
    os.utime(path, ns=(stamp, stamp))
    updated = raw.decode_line_entities(str(path))
    assert updated != original
    assert updated[0][1] == 1.75
    ezdwg.clear_decode_caches()
    assert raw.decode_line_entities(str(path)) == updated


def test_limited_strict_read_does_not_raise_a_later_cached_header_error(
    tmp_path: Path,
) -> None:
    path = tmp_path / "damaged.dwg"
    drawing = ezdwg.new("AC1015")
    model = drawing.modelspace()
    model.add_line((0, 0), (1, 1))
    model.add_line((2, 2), (3, 3))
    drawing.save(path)
    lines = raw.decode_line_entities(str(path))
    headers = raw.list_object_headers_by_type(str(path), [0x13])
    records = raw.read_object_records_by_type(str(path), [0x13])
    assert len(headers) == 2
    # AC1015 object offsets are file offsets. A zero MS size invalidates the
    # second record's framing while retaining the valid object map and first line.
    data = bytearray(path.read_bytes())
    offset = headers[1][1]
    data[offset : offset + 2] = b"\0\0"
    path.write_bytes(data)
    ezdwg.clear_decode_caches()

    for _ in range(2):  # both cold and shared catalog paths
        assert raw.decode_line_entities(str(path), limit=1) == lines[:1]
        assert raw.list_object_headers_by_type(str(path), [0x13], limit=1) == headers[:1]
        assert raw.decode_line_entities(str(path)) == lines[:1]
        assert raw.read_object_records_by_type(str(path), [0x13], limit=1) == records[:1]
        assert raw.list_object_handles_by_type_name(str(path), "LINE") == [lines[0][0]]
        assert raw.list_object_type_names(str(path)) == list(
            dict.fromkeys(row[4] for row in raw.list_object_headers_with_type(str(path)))
        )
        with pytest.raises(ValueError, match="object record size is zero"):
            raw.read_object_records_by_type(str(path), [0x13])
        # A strict lookup for an absent type must still encounter that error.
        with pytest.raises(ValueError, match="object record size is zero"):
            raw.read_object_records_by_type(str(path), [0x12])
