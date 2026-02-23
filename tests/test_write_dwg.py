from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
import ezdwg.cli as cli_module
import ezdwg.document as document_module


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def test_raw_write_ac1015_line_dwg_smoke(tmp_path: Path) -> None:
    output = tmp_path / "raw_line_out.dwg"
    ezdwg.raw.write_ac1015_line_dwg(
        str(output),
        [(0x30, 1.0, 2.0, 0.0, 4.5, 7.0, 0.0)],
    )

    assert output.exists()
    doc = ezdwg.read(str(output))
    lines = list(doc.modelspace().query("LINE"))
    assert len(lines) == 1
    assert lines[0].handle == 0x30
    assert lines[0].dxf["start"] == (1.0, 2.0, 0.0)
    assert lines[0].dxf["end"] == (4.5, 7.0, 0.0)


def test_raw_write_ac1015_dwg_writes_lwpolyline(tmp_path: Path) -> None:
    output = tmp_path / "raw_lwpolyline_out.dwg"
    ezdwg.raw.write_ac1015_dwg(
        str(output),
        [],
        [],
        [],
        [
            (
                0x42,
                1,
                [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)],
                [],
                [],
                None,
            )
        ],
        [],
        [],
        [],
    )

    assert output.exists()
    doc = ezdwg.read(str(output))
    polys = list(doc.modelspace().query("LWPOLYLINE"))
    assert len(polys) == 1
    assert polys[0].handle == 0x42
    assert polys[0].dxf["closed"] is True
    assert polys[0].dxf["points"] == [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (2.0, 1.0, 0.0)]


def test_raw_write_ac1015_dwg_writes_ray_and_xline(tmp_path: Path) -> None:
    output = tmp_path / "raw_ray_xline_out.dwg"
    ezdwg.raw.write_ac1015_dwg(
        str(output),
        [],
        [],
        [],
        [],
        [],
        [],
        [],
        [(0x50, (1.0, 2.0, 0.0), (1.0, 0.0, 0.0))],
        [(0x51, (3.0, 4.0, 0.0), (0.0, 1.0, 0.0))],
    )

    assert output.exists()
    doc = ezdwg.read(str(output))
    rays = list(doc.modelspace().query("RAY"))
    xlines = list(doc.modelspace().query("XLINE"))
    assert len(rays) == 1
    assert len(xlines) == 1
    assert rays[0].handle == 0x50
    assert rays[0].dxf["start"] == (1.0, 2.0, 0.0)
    assert rays[0].dxf["unit_vector"] == (1.0, 0.0, 0.0)
    assert xlines[0].handle == 0x51
    assert xlines[0].dxf["start"] == (3.0, 4.0, 0.0)
    assert xlines[0].dxf["unit_vector"] == (0.0, 1.0, 0.0)


def test_to_dwg_writes_ray_and_xline_from_document(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (0x60, 10, 0, 0x28, "RAY", "Entity"),
            (0x61, 11, 0, 0x29, "XLINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_ray_entities",
        lambda _path: [(0x60, (1.0, 2.0, 0.0), (1.0, 0.0, 0.0))],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_xline_entities",
        lambda _path: [(0x61, (3.0, 4.0, 0.0), (0.0, 1.0, 0.0))],
    )

    output = tmp_path / "ray_xline_written.dwg"
    doc = document_module.Document(path="dummy_ray_xline_write.dwg", version="AC1021")
    result = ezdwg.to_dwg(doc, str(output), types="RAY XLINE", version="AC1015")

    assert output.exists()
    assert result.total_entities == 2
    assert result.written_entities == 2
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    rays = list(out_doc.modelspace().query("RAY"))
    xlines = list(out_doc.modelspace().query("XLINE"))
    assert len(rays) == 1
    assert len(xlines) == 1
    assert rays[0].dxf["start"] == (1.0, 2.0, 0.0)
    assert rays[0].dxf["unit_vector"] == (1.0, 0.0, 0.0)
    assert xlines[0].dxf["start"] == (3.0, 4.0, 0.0)
    assert xlines[0].dxf["unit_vector"] == (0.0, 1.0, 0.0)


def test_to_dwg_writes_line_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "line_2000.dwg"
    output = tmp_path / "line_2000_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_line = next(src_doc.modelspace().query("LINE"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.target_version == "AC1015"
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_line = next(out_doc.modelspace().query("LINE"))
    assert out_line.dxf["start"] == src_line.dxf["start"]
    assert out_line.dxf["end"] == src_line.dxf["end"]


def test_to_dwg_writes_arc_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "arc_2007.dwg"
    output = tmp_path / "arc_2007_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_arc = next(src_doc.modelspace().query("ARC"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_arc = next(out_doc.modelspace().query("ARC"))
    assert out_arc.dxf["center"] == src_arc.dxf["center"]
    assert abs(float(out_arc.dxf["radius"]) - float(src_arc.dxf["radius"])) < 1.0e-9
    assert abs(float(out_arc.dxf["start_angle"]) - float(src_arc.dxf["start_angle"])) < 1.0e-9
    assert abs(float(out_arc.dxf["end_angle"]) - float(src_arc.dxf["end_angle"])) < 1.0e-9


def test_to_dwg_writes_circle_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "circle_2007.dwg"
    output = tmp_path / "circle_2007_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_circle = next(src_doc.modelspace().query("CIRCLE"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_circle = next(out_doc.modelspace().query("CIRCLE"))
    assert out_circle.dxf["center"] == src_circle.dxf["center"]
    assert abs(float(out_circle.dxf["radius"]) - float(src_circle.dxf["radius"])) < 1.0e-9


def test_to_dwg_writes_point_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "point2d_2007.dwg"
    output = tmp_path / "point2d_2007_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_point = next(src_doc.modelspace().query("POINT"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_point = next(out_doc.modelspace().query("POINT"))
    assert out_point.dxf["location"] == src_point.dxf["location"]
    assert abs(float(out_point.dxf["x_axis_angle"]) - float(src_point.dxf["x_axis_angle"])) < 1.0e-9


def test_to_dwg_writes_text_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "text_2000.dwg"
    output = tmp_path / "text_2000_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_text = next(src_doc.modelspace().query("TEXT"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_text = next(out_doc.modelspace().query("TEXT"))
    assert out_text.dxf["text"] == src_text.dxf["text"]
    assert out_text.dxf["insert"] == src_text.dxf["insert"]
    assert abs(float(out_text.dxf["height"]) - float(src_text.dxf["height"])) < 1.0e-9
    assert abs(float(out_text.dxf["rotation"]) - float(src_text.dxf["rotation"])) < 1.0e-9


def test_to_dwg_writes_mtext_from_source_sample(tmp_path: Path) -> None:
    source = SAMPLES / "mtext_2000.dwg"
    output = tmp_path / "mtext_2000_written.dwg"

    src_doc = ezdwg.read(str(source))
    src_mtext = next(src_doc.modelspace().query("MTEXT"))
    result = ezdwg.to_dwg(str(source), str(output), version="AC1015")

    assert output.exists()
    assert result.total_entities == 1
    assert result.written_entities == 1
    assert result.skipped_entities == 0

    out_doc = ezdwg.read(str(output))
    out_mtext = next(out_doc.modelspace().query("MTEXT"))
    assert out_mtext.dxf["text"] == src_mtext.dxf["text"]
    assert out_mtext.dxf["insert"] == src_mtext.dxf["insert"]
    assert abs(float(out_mtext.dxf["char_height"]) - float(src_mtext.dxf["char_height"])) < 1.0e-9


def test_to_dwg_strict_rejects_unsupported_entity_type(tmp_path: Path) -> None:
    source = SAMPLES / "insert_2004.dwg"
    output = tmp_path / "insert_written_as_dwg.dwg"

    with pytest.raises(ValueError, match="failed to write 1 entities"):
        ezdwg.to_dwg(
            str(source),
            str(output),
            types="INSERT",
            version="AC1015",
            strict=True,
        )


def test_cli_write_writes_line(tmp_path: Path, capsys) -> None:
    source = SAMPLES / "line_2007.dwg"
    output = tmp_path / "line_from_cli_written.dwg"

    code = cli_module._run_write(
        str(source),
        str(output),
        types="LINE",
        dwg_version="AC1015",
        strict=False,
    )
    captured = capsys.readouterr()

    assert code == 0
    assert "target_version: AC1015" in captured.out
    assert "written_entities: 1" in captured.out
    assert output.exists()

    out_doc = ezdwg.read(str(output))
    out_lines = list(out_doc.modelspace().query("LINE"))
    assert len(out_lines) == 1


def test_cli_write_rejects_unsupported_version(tmp_path: Path, capsys) -> None:
    source = SAMPLES / "line_2007.dwg"
    output = tmp_path / "line_from_cli_unsupported_version.dwg"

    code = cli_module._run_write(
        str(source),
        str(output),
        types="LINE",
        dwg_version="AC1018",
        strict=False,
    )
    captured = capsys.readouterr()

    assert code == 2
    assert "unsupported DWG write version: AC1018" in captured.err


def test_cli_main_dispatches_write_command(monkeypatch) -> None:
    captured: dict[str, object] = {}

    def _fake_run_write(
        input_path: str,
        output_path: str,
        *,
        types: str | None = None,
        dwg_version: str = "AC1015",
        strict: bool = False,
    ) -> int:
        captured["input_path"] = input_path
        captured["output_path"] = output_path
        captured["types"] = types
        captured["dwg_version"] = dwg_version
        captured["strict"] = strict
        return 0

    monkeypatch.setattr(cli_module, "_run_write", _fake_run_write)

    code = cli_module.main(
        [
            "write",
            "in.dwg",
            "out.dwg",
            "--types",
            "LINE",
            "--dwg-version",
            "AC1015",
            "--strict",
        ]
    )

    assert code == 0
    assert captured == {
        "input_path": "in.dwg",
        "output_path": "out.dwg",
        "types": "LINE",
        "dwg_version": "AC1015",
        "strict": True,
    }
