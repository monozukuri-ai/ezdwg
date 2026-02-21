from __future__ import annotations

from pathlib import Path

import ezdwg.cli as cli_module
from ezdwg.entity import Entity


class _DummyLayout:
    def __init__(self, entities: list[Entity]) -> None:
        self._entities = entities

    def query(self, types=None):  # noqa: ANN001
        return iter(self._entities)


class _DummyDoc:
    def __init__(self, path: str, entities: list[Entity]) -> None:
        self.version = "AC1021"
        self.decode_version = "AC1021"
        self.decode_path = path
        self._layout = _DummyLayout(entities)

    def modelspace(self) -> _DummyLayout:
        return self._layout


def test_cli_inspect_reports_record_diagnostics(monkeypatch, tmp_path: Path, capsys) -> None:
    dummy_path = tmp_path / "dummy.dwg"
    dummy_path.write_bytes(b"dwg")
    entities = [
        Entity(
            dxftype="LONG_TRANSACTION",
            handle=100,
            dxf={
                "record_size": 14,
                "ascii_preview": "LONGTX",
                "likely_handle_ref_details": [
                    {"handle": 120, "type_name": "LINE"},
                    {"handle": 999, "type_name": "UNKNOWN(0x999)"},
                ],
                "decoded_handle_ref_details": [
                    {"handle": 1, "type_name": "LAYER"},
                    {"handle": 2, "type_name": "UNKNOWN", "type_code": 0x33},
                ],
            },
        ),
        Entity(
            dxftype="OLEFRAME",
            handle=101,
            dxf={
                "record_size": None,
                "ascii_preview": None,
                "likely_handle_refs": [200, 201],
            },
        ),
        Entity(
            dxftype="OLE2FRAME",
            handle=102,
            dxf={
                "record_size": 4,
                "ascii_preview": "OLE2",
                "likely_handle_ref_details": [
                    {"handle": 300, "type_name": "UNKNOWN(0x300)"},
                ],
            },
        ),
    ]
    monkeypatch.setattr(cli_module, "read", lambda _path: _DummyDoc(_path, entities))
    monkeypatch.setattr(
        cli_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (100, 10, 0, 0x4C, "LONG_TRANSACTION", "Entity"),
            (2, 11, 0, 0x33, "LAYER", "Object"),
            (300, 12, 0, 0x300, "UNKNOWN(0x300)", "Object"),
        ],
    )

    code = cli_module._run_inspect(str(dummy_path))
    captured = capsys.readouterr()

    assert code == 0
    assert (
        "record_diag[LONG_TRANSACTION]: entities=1 record_bytes=1 ascii=1 "
        "likely_refs=2 unresolved_likely_refs=1 decoded_refs=2 unresolved_decoded_refs=1"
    ) in captured.out
    assert (
        "record_diag[OLEFRAME]: entities=1 record_bytes=0 ascii=0 "
        "likely_refs=2 unresolved_likely_refs=0"
    ) in captured.out
    assert (
        "record_diag[OLE2FRAME]: entities=1 record_bytes=1 ascii=1 "
        "likely_refs=1 unresolved_likely_refs=1"
    ) in captured.out
    assert "record_diag_unknown_handles[LONG_TRANSACTION]:" in captured.out
    assert "999:1(missing)" in captured.out
    assert "2:1(LAYER/OBJECT)" in captured.out
    assert "record_diag_unknown_type_codes[LONG_TRANSACTION]:" in captured.out
    assert "0x999:1(unmapped)" in captured.out
    assert "0x33:1(LAYER/OBJECT)" in captured.out
    assert "record_diag_unknown_handles[OLE2FRAME]: 300:1(UNKNOWN(0x300)/OBJECT)" in captured.out
    assert "record_diag_unknown_type_codes[OLE2FRAME]: 0x300:1(UNKNOWN(0x300)/OBJECT)" in captured.out


def test_cli_inspect_verbose_expands_unknown_top_n(
    monkeypatch,
    tmp_path: Path,
    capsys,
) -> None:
    dummy_path = tmp_path / "dummy_verbose.dwg"
    dummy_path.write_bytes(b"dwg")
    likely_details = (
        [{"handle": 101, "type_name": "UNKNOWN(0x101)"}] * 6
        + [{"handle": 102, "type_name": "UNKNOWN(0x102)"}] * 5
        + [{"handle": 103, "type_name": "UNKNOWN(0x103)"}] * 4
        + [{"handle": 104, "type_name": "UNKNOWN(0x104)"}] * 3
    )
    entities = [
        Entity(
            dxftype="LONG_TRANSACTION",
            handle=100,
            dxf={
                "record_size": 10,
                "ascii_preview": None,
                "likely_handle_ref_details": likely_details,
                "decoded_handle_ref_details": [],
            },
        ),
    ]
    monkeypatch.setattr(cli_module, "read", lambda _path: _DummyDoc(_path, entities))
    monkeypatch.setattr(
        cli_module.raw,
        "list_object_headers_with_type",
        lambda _path: [],
    )

    code = cli_module._run_inspect(str(dummy_path))
    captured_default = capsys.readouterr()

    assert code == 0
    assert "101:6(missing)" in captured_default.out
    assert "102:5(missing)" in captured_default.out
    assert "103:4(missing)" in captured_default.out
    assert "104:3(missing)" not in captured_default.out

    code = cli_module._run_inspect(str(dummy_path), verbose=True)
    captured_verbose = capsys.readouterr()

    assert code == 0
    assert "104:3(missing)" in captured_verbose.out
