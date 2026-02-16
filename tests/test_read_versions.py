from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
import ezdwg.document as document_module


ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize(
    ("relative_path", "expected_version"),
    [
        ("test_dwg/line_R14.dwg", "AC1014"),
        ("test_dwg/line_2000.dwg", "AC1015"),
        ("test_dwg/line_2004.dwg", "AC1018"),
        ("test_dwg/line_2007.dwg", "AC1021"),
        ("test_dwg/line_2010.dwg", "AC1024"),
        ("test_dwg/line_2013.dwg", "AC1027"),
    ],
)
def test_detect_version_from_samples(relative_path: str, expected_version: str) -> None:
    path = ROOT / relative_path
    assert path.exists(), f"missing sample: {path}"
    assert ezdwg.raw.detect_version(str(path)) == expected_version


@pytest.mark.parametrize(
    ("relative_path", "expected_version"),
    [
        ("test_dwg/line_R14.dwg", "AC1014"),
        ("test_dwg/line_2000.dwg", "AC1015"),
        ("test_dwg/line_2004.dwg", "AC1018"),
        ("test_dwg/line_2007.dwg", "AC1021"),
        ("test_dwg/line_2010.dwg", "AC1024"),
        ("test_dwg/line_2013.dwg", "AC1027"),
    ],
)
def test_read_native_versions(relative_path: str, expected_version: str) -> None:
    path = ROOT / relative_path
    assert path.exists(), f"missing sample: {path}"

    doc = ezdwg.read(str(path))

    assert doc.version == expected_version
    assert doc.decode_version == expected_version
    assert doc.decode_path == str(path)


@pytest.mark.parametrize("source_version", ["AC1014", "AC1021", "AC1024", "AC1027", "AC1032"])
def test_read_native_versions_do_not_require_conversion(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    source_version: str,
) -> None:
    source_path = tmp_path / "source_native.dwg"
    source_path.write_bytes(b"source")

    def fake_detect_version(path: str) -> str:
        if path == str(source_path):
            return source_version
        raise AssertionError(f"unexpected detect_version path: {path}")

    monkeypatch.setattr(document_module.raw, "detect_version", fake_detect_version)

    doc = document_module.read(str(source_path))

    assert doc.version == source_version
    assert doc.decode_version == source_version
    assert doc.decode_path == str(source_path)


def test_read_rejects_unknown_version(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    source_path = tmp_path / "unknown.dwg"
    source_path.write_bytes(b"unknown")

    monkeypatch.setattr(document_module.raw, "detect_version", lambda _: "AC9999")

    with pytest.raises(ValueError, match="unsupported DWG version: AC9999"):
        document_module.read(str(source_path))


def test_detect_version_accepts_ac1032_header(tmp_path: Path) -> None:
    path = tmp_path / "ac1032_header.dwg"
    path.write_bytes(b"AC1032dummy")

    assert ezdwg.raw.detect_version(str(path)) == "AC1032"


@pytest.mark.parametrize(
    ("relative_path", "expected_version"),
    [
        ("test_dwg/acadsharp/sample_AC1032.dwg", "AC1032"),
        ("test_dwg/acadsharp/BLOCKPOINTPARAMETER.dwg", "AC1032"),
        ("test_dwg/acadsharp/sample_AC1027.dwg", "AC1027"),
    ],
)
def test_detect_version_from_acadsharp_samples(
    relative_path: str,
    expected_version: str,
) -> None:
    path = ROOT / relative_path
    assert path.exists(), f"missing sample: {path}"
    assert ezdwg.raw.detect_version(str(path)) == expected_version


def test_read_ac1032_sample_smoke() -> None:
    path = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert path.exists(), f"missing sample: {path}"

    doc = ezdwg.read(str(path))

    assert doc.version == "AC1032"
    assert doc.decode_version == "AC1032"
    assert doc.decode_path == str(path)

    rows = ezdwg.raw.list_object_headers_with_type(str(path), limit=20)
    assert len(rows) == 20


def test_read_ac1014_sample_smoke() -> None:
    path = ROOT / "test_dwg/line_R14.dwg"
    assert path.exists(), f"missing sample: {path}"

    doc = ezdwg.read(str(path))

    assert doc.version == "AC1014"
    assert doc.decode_version == "AC1014"
    assert doc.decode_path == str(path)

    rows = ezdwg.raw.list_object_headers_with_type(str(path), limit=20)
    assert len(rows) == 20


def test_read_object_records_by_handle_roundtrip() -> None:
    path = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert path.exists(), f"missing sample: {path}"

    headers = ezdwg.raw.list_object_headers_with_type(str(path), limit=5)
    handles = [int(row[0]) for row in headers[:3]]
    rows = ezdwg.raw.read_object_records_by_handle(str(path), handles)

    assert [int(row[0]) for row in rows] == handles
    assert all(len(bytes(row[4])) > 0 for row in rows)


def test_decode_object_handle_stream_refs_smoke() -> None:
    path = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert path.exists(), f"missing sample: {path}"

    headers = ezdwg.raw.list_object_headers_with_type(str(path))
    handles = [int(row[0]) for row in headers if int(row[3]) in {0x222, 0x223}][:2]
    assert handles

    rows = ezdwg.raw.decode_object_handle_stream_refs(str(path), handles)
    assert [int(row[0]) for row in rows] == handles

    known_handles = {handle for handle, _ in ezdwg.raw.list_object_map_entries(str(path))}
    for _handle, refs in rows:
        assert all(int(ref) in known_handles for ref in refs)
    assert any(len(refs) > 0 for _handle, refs in rows)


def test_decode_acis_candidate_infos_smoke() -> None:
    path = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert path.exists(), f"missing sample: {path}"

    headers = ezdwg.raw.list_object_headers_with_type(str(path))
    handles = [int(row[0]) for row in headers if int(row[3]) in {0x222, 0x223}][:2]
    assert handles

    rows = ezdwg.raw.decode_acis_candidate_infos(str(path), handles)
    assert [int(row[0]) for row in rows] == handles

    known_handles = {handle for handle, _ in ezdwg.raw.list_object_map_entries(str(path))}
    for _handle, type_code, data_size, role_hint, refs, confidence in rows:
        assert type_code in {0x214, 0x221, 0x222, 0x223, 0x224, 0x225} or type_code > 0
        assert data_size >= 0
        assert isinstance(role_hint, str) and role_hint != ""
        assert all(int(ref) in known_handles for ref in refs)
        assert 0 <= int(confidence) <= 100
    assert any(str(row[3]).startswith("acis-") for row in rows)


def test_decode_acis_candidate_infos_expected_chain_sample() -> None:
    path = ROOT / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert path.exists(), f"missing sample: {path}"

    rows = ezdwg.raw.decode_acis_candidate_infos(str(path), [3430, 3431, 3432])
    row_map = {
        int(handle): (int(type_code), str(role_hint), list(refs), int(confidence))
        for handle, type_code, _size, role_hint, refs, confidence in rows
    }

    assert 3430 in row_map and 3431 in row_map and 3432 in row_map
    assert row_map[3430][1] in {"acis-header", "acis-text-header"}
    assert row_map[3431][1] == "acis-link-table"
    assert row_map[3432][1].startswith("acis-payload")
    assert 3429 in row_map[3430][2]
    assert 3430 in row_map[3431][2]
    assert 3431 in row_map[3432][2]
    assert all(0 <= row_map[handle][3] <= 100 for handle in (3430, 3431, 3432))
