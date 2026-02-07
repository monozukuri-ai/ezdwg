from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
import ezdwg.document as document_module


ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize(
    ("relative_path", "expected_version"),
    [
        ("dwg_samples/line_2000.dwg", "AC1015"),
        ("dwg_samples/line_2004.dwg", "AC1018"),
        ("dwg_samples/line_2007.dwg", "AC1021"),
        ("dwg_samples/line_2010.dwg", "AC1024"),
        ("dwg_samples/line_2013.dwg", "AC1027"),
    ],
)
def test_read_native_versions(relative_path: str, expected_version: str) -> None:
    path = ROOT / relative_path
    assert path.exists(), f"missing sample: {path}"

    doc = ezdwg.read(str(path))

    assert doc.version == expected_version
    assert doc.decode_version == expected_version
    assert doc.decode_path == str(path)


@pytest.mark.parametrize("source_version", ["AC1021", "AC1024", "AC1027"])
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
