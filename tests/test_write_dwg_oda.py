"""Opt-in independent AC1015 interoperability check.

EZDWG_ODA_FILE_CONVERTER=/usr/bin/ODAFileConverter uv run pytest -q tests/test_write_dwg_oda.py
The converter is an oracle only; the native writer never invokes it.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

import pytest

import ezdwg


@pytest.fixture
def oda_command() -> list[str]:
    converter = os.environ.get("EZDWG_ODA_FILE_CONVERTER")
    if not converter:
        pytest.skip("set EZDWG_ODA_FILE_CONVERTER to run independent DWG checks")
    if not Path(converter).is_file():
        pytest.fail(f"ODA File Converter not found: {converter}")
    if sys.platform.startswith("linux") and not os.environ.get("DISPLAY"):
        xvfb = shutil.which("xvfb-run")
        if not xvfb:
            pytest.fail("headless ODA check requires xvfb-run")
        return [xvfb, "-a", converter]
    return [converter]


def convert(
    command: list[str], source: Path, output: Path, kind: str, version: str = "ACAD2000"
) -> Path:
    output.mkdir()
    result = subprocess.run(
        [
            *command,
            str(source.parent),
            str(output),
            version,
            kind,
            "0",
            "0",
            source.name,
        ],
        capture_output=True,
        check=False,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, (result.stdout, result.stderr)
    # ODA may exit 0 even on a failed read or partially written output.
    errors = {p.name: p.read_text(errors="replace") for p in output.glob("*.err")}
    assert not any(errors.values()), errors
    target = output / (source.stem + "." + kind.lower())
    assert target.is_file(), (result.stdout, result.stderr)
    return target


def test_created_and_edited_drawing_external_read(oda_command, tmp_path):
    from tests.test_drawing import populated_drawing

    ezdxf = pytest.importorskip("ezdxf")
    source, output = tmp_path / "created.dwg", tmp_path / "edited.dwg"
    populated_drawing().save(source)
    drawing = ezdwg.read(str(source)).edit()
    msp = drawing.modelspace()
    line = next(msp.query("LINE"))
    msp.update(line, end=(20, 30, 0), color_index=1)
    msp.delete(next(msp.query("CIRCLE")))
    msp.add_point((50, 60, 7), layer="作図")
    msp.update(next(msp.query("MTEXT")), text="Edited\\Ptext")
    drawing.save(output)
    resaved = convert(oda_command, output, tmp_path / "resaved", "DWG")
    dxf = convert(oda_command, resaved, tmp_path / "dxf", "DXF")
    doc = ezdxf.readfile(dxf)
    audit = doc.audit()
    assert not audit.errors and not audit.fixes
    external = doc.modelspace()
    assert len(external) == 9
    assert not external.query("CIRCLE")
    assert len(external.query("POINT")) == 2
    assert tuple(external.query("LINE").first.dxf.end) == (20, 30, 0)
    assert external.query("LINE").first.dxf.color == 1
    assert external.query("MTEXT").first.text == "Edited\\Ptext"
    assert any(tuple(p.dxf.location) == (50, 60, 7) for p in external.query("POINT"))
    assert doc.header["$INSUNITS"] == 4


@pytest.mark.parametrize("case", ["empty", "mixed", "multipage"])
def test_native_ac1015_external_read_and_resave(
    oda_command, tmp_path: Path, case: str
) -> None:
    ezdxf = pytest.importorskip("ezdxf")
    source = tmp_path / "native.dwg"
    lines, arcs, circles, polys, texts, mtexts, points, rays, xlines = (
        [] for _ in range(9)
    )
    expected: dict[str, int] = {}
    if case == "mixed":
        # Deliberately sparse, unsorted handles and a low input handle: the
        # database allocation and entity links must not assume +/-1 adjacency.
        lines = [
            (1, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
            (0xABC, -2.0, -3.0, 0.0, 9.0, 7.0, 0.0),
        ]
        arcs = [(0x400, 2.0, 3.0, 0.0, 5.0, 0.25, 1.5)]
        circles = [(0x200, 4.0, 5.0, 0.0, 2.5)]
        polys = [
            (
                0x100,
                0x200,
                [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)],
                [0.0, 0.5, 0.0],
                [],
                None,
            )
        ]
        texts = [(0x800, "HELLO", (1.5, 2.5, 0.0), 2.0, 0.2)]
        mtexts = [(0x700, "MULTI", (3.0, 4.0, 0.0), (1.0, 0.0, 0.0), 12.0, 1.5, 1, 1)]
        points = [(0x600, 7.0, 8.0, 0.0, 0.3)]
        rays = [(0x500, (9.0, 1.0, 0.0), (1.0, 0.0, 0.0))]
        xlines = [(0x300, (10.0, 2.0, 0.0), (0.0, 1.0, 0.0))]
        expected = {
            t: 1
            for t in (
                "ARC",
                "CIRCLE",
                "LWPOLYLINE",
                "TEXT",
                "MTEXT",
                "POINT",
                "RAY",
                "XLINE",
            )
        }
        expected["LINE"] = 2
    elif case == "multipage":
        lines = [
            (100 + 3 * i, float(i), 2.0, 0.0, float(i) + 1, 5.0, 0.0)
            for i in range(1500)
        ]
        expected = {"LINE": 1500}
    ezdwg.raw.write_ac1015_dwg(
        str(source),
        lines,
        arcs,
        circles,
        polys,
        texts,
        mtexts,
        points,
        rays,
        xlines,
    )
    dxf = convert(oda_command, source, tmp_path / "dxf", "DXF")
    rewritten = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf_again = convert(oda_command, rewritten, tmp_path / "dxf-again", "DXF")
    assert rewritten.read_bytes().startswith(b"AC1015")
    for path in (dxf, dxf_again):
        doc = ezdxf.readfile(path)
        msp = doc.modelspace()
        assert dict(Counter(e.dxftype() for e in msp)) == expected
        audit = doc.audit()
        assert not audit.errors, [(e.code, e.message) for e in audit.errors]
        assert not audit.fixes, [(e.code, e.message) for e in audit.fixes]
        for handle, sx, sy, sz, ex, ey, ez in lines:
            line = doc.entitydb[f"{handle:X}"]
            assert tuple(line.dxf.start) == pytest.approx((sx, sy, sz))
            assert tuple(line.dxf.end) == pytest.approx((ex, ey, ez))
        if case == "mixed":
            import math

            arc = msp.query("ARC").first
            assert arc.dxf.radius == pytest.approx(5.0)
            assert arc.dxf.start_angle == pytest.approx(math.degrees(0.25))
            assert arc.dxf.end_angle == pytest.approx(math.degrees(1.5))
            assert msp.query("CIRCLE").first.dxf.radius == pytest.approx(2.5)
            poly = msp.query("LWPOLYLINE").first
            assert poly.closed
            assert list(poly.get_points("xyb")) == [
                (0.0, 0.0, 0.0),
                (2.0, 0.0, 0.5),
                (2.0, 1.0, 0.0),
            ]
            assert msp.query("TEXT").first.dxf.text == "HELLO"
            assert msp.query("MTEXT").first.text == "MULTI"
            assert tuple(msp.query("POINT").first.dxf.location) == (7.0, 8.0, 0.0)
            assert tuple(msp.query("RAY").first.dxf.unit_vector) == (1.0, 0.0, 0.0)
            assert tuple(msp.query("XLINE").first.dxf.unit_vector) == (0.0, 1.0, 0.0)
    assert (
        dict(
            Counter(e.dxftype for e in ezdwg.read(str(rewritten)).modelspace().query())
        )
        == expected
    )


def test_styled_ac1015_external_read_and_rewrite(oda_command, tmp_path):
    from tests.test_write_dwg_attributes import make_styled_drawing

    ezdxf = pytest.importorskip("ezdxf")
    from ezdxf.lldxf.encoding import decode_mif_to_unicode

    source = tmp_path / "styled.dwg"
    make_styled_drawing(source)
    native_dxf = convert(oda_command, source, tmp_path / "native-dxf", "DXF")
    # Rewrite an independent reader's saved file as well as the native fixture.
    oda_saved = convert(oda_command, source, tmp_path / "oda-saved", "DWG")
    rewritten = tmp_path / "rewritten.dwg"
    result = ezdwg.to_dwg(str(oda_saved), str(rewritten), strict=True)
    assert not result.diagnostics
    rewritten_dxf = convert(oda_command, rewritten, tmp_path / "rewritten-dxf", "DXF")
    for path in (native_dxf, rewritten_dxf):
        doc = ezdxf.readfile(path)
        audit = doc.audit()
        assert not audit.errors and not audit.fixes
        entities = list(doc.modelspace())
        assert len(entities) == 5
        assert doc.header["$INSUNITS"] == 4
        assert doc.header["$LTSCALE"] == 3.0
        layer = next(
            l for l in doc.layers if decode_mif_to_unicode(l.dxf.name) == "寸法"
        )
        assert layer.is_off() and layer.is_frozen() and layer.is_locked()
        assert layer.dxf.plot == 0
        assert layer.dxf.lineweight == 35
        assert layer.dxf.linetype == "DASHED"
        line, byblock = [e for e in entities if e.dxftype() == "LINE"]
        assert line.dxf.color == 256
        assert decode_mif_to_unicode(line.dxf.layer) == "寸法"
        assert line.dxf.linetype == "DASHED"
        assert line.dxf.ltscale == 2.0
        assert line.dxf.lineweight == 50
        assert line.dxf.thickness == 2.0
        assert tuple(line.dxf.extrusion) == (0.0, 1.0, 0.0)
        assert byblock.dxf.color == 0
        assert byblock.dxf.linetype.upper() == "BYBLOCK"
        assert byblock.dxf.lineweight == -2
        assert byblock.dxf.invisible == 1
        poly = next(e for e in entities if e.dxftype() == "LWPOLYLINE")
        assert not poly.closed
        assert poly.dxf.elevation == 7.0
        assert poly.dxf.thickness == 0.5
        assert tuple(poly.dxf.extrusion) == (0.0, 1.0, 0.0)
        text = next(e for e in entities if e.dxftype() == "TEXT")
        assert text.dxf.text == "日本語 ABC"
        assert decode_mif_to_unicode(text.dxf.style) == "日本語"
        assert text.dxf.halign == 2 and text.dxf.valign == 3
        assert tuple(text.dxf.align_point) == (5.0, 6.0, 3.0)
        assert text.dxf.width == 0.8
        assert text.dxf.oblique == pytest.approx(10.0)
        assert text.dxf.text_generation_flag == 2
        assert doc.styles.get(text.dxf.style).dxf.font == "txt.shx"
        assert doc.styles.get(text.dxf.style).dxf.flags & 4
        assert [
            tag.value
            for tag in doc.linetypes.get("DASHED").pattern_tags.tags
            if tag.code == 49
        ] == [1.0, -0.5]
        mtext = next(e for e in entities if e.dxftype() == "MTEXT")
        assert decode_mif_to_unicode(mtext.text) == "注記\\P二行目"
        assert mtext.dxf.line_spacing_style == 2
        assert mtext.dxf.line_spacing_factor == 1.5


def test_external_attribute_losses_are_reported(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    source = tmp_path / "modern.dxf"
    doc = ezdxf.new("R2013")
    doc.appids.add("KEEP_DATA")
    doc.linetypes.new("GAS").setup_pattern(
        'A,.5,-.2,["GAS",STANDARD,S=.1,R=0.,X=-.1,Y=-.05],-.2', length=0.9
    )
    line = doc.modelspace().add_line(
        (0.0, 0.0), (3.0, 4.0), dxfattribs={"true_color": 0, "linetype": "GAS"}
    )
    line.transparency = 0.5
    line.set_xdata("KEEP_DATA", [(1000, "keep this value")])
    doc.saveas(source)
    dwg = convert(oda_command, source, tmp_path / "modern-dwg", "DWG", "ACAD2013")
    out = tmp_path / "out.dwg"
    out.write_bytes(b"keep existing file")
    with pytest.raises(ValueError, match="diagnostics"):
        ezdwg.to_dwg(str(dwg), str(out), strict=True)
    assert out.read_bytes() == b"keep existing file"
    result = ezdwg.to_dwg(str(dwg), str(out))
    losses = {d.attribute for d in result.diagnostics}
    assert {"true_color", "transparency", "xdata", "linetype"} <= losses
    assert result.written_entities == 1
