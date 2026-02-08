from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
import ezdwg.cli as cli_module
import ezdwg.convert as convert_module
from tests._dxf_helpers import dxf_entities_of_type, group_float


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def test_to_dxf_writes_line_entity(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "line_out.dxf"
    result = ezdwg.to_dxf(
        str(SAMPLES / "line_2007.dwg"),
        str(output),
        types="LINE",
        dxf_version="R2010",
    )

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0
    assert len(dxf_entities_of_type(output, "LINE")) == 1


def test_document_export_dxf_writes_arc_angles(tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    source = SAMPLES / "arc_2007.dwg"
    output = tmp_path / "arc_out.dxf"

    doc = ezdwg.read(str(source))
    source_arc = next(doc.modelspace().query("ARC")).dxf
    result = doc.export_dxf(str(output), types="ARC")

    assert result.total_entities == 1
    arcs = dxf_entities_of_type(output, "ARC")
    assert len(arcs) == 1
    out_arc = arcs[0]
    assert abs(group_float(out_arc, "50") - float(source_arc["start_angle"])) < 1.0e-6
    assert abs(group_float(out_arc, "51") - float(source_arc["end_angle"])) < 1.0e-6


def test_cli_convert_writes_lwpolyline(tmp_path: Path, capsys) -> None:
    pytest.importorskip("ezdxf")

    output = tmp_path / "polyline_out.dxf"
    code = cli_module._run_convert(
        str(SAMPLES / "polyline2d_line_2007.dwg"),
        str(output),
        types="LWPOLYLINE",
        dxf_version="R2010",
        strict=False,
    )
    captured = capsys.readouterr()

    assert code == 0
    assert "written_entities: 1" in captured.out
    assert len(dxf_entities_of_type(output, "LWPOLYLINE")) == 1


def test_to_dxf_strict_raises_on_skipped_entity(monkeypatch, tmp_path: Path) -> None:
    pytest.importorskip("ezdxf")

    monkeypatch.setattr(convert_module, "_write_entity_to_modelspace", lambda *_args, **_kwargs: False)

    with pytest.raises(ValueError, match="failed to convert"):
        convert_module.to_dxf(
            str(SAMPLES / "line_2007.dwg"),
            str(tmp_path / "strict_out.dxf"),
            types="LINE",
            strict=True,
        )
