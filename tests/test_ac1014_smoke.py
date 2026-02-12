from __future__ import annotations

import math
from pathlib import Path

import ezdwg
import ezdwg.cli as cli_module
from ezdwg import raw


ROOT = Path(__file__).resolve().parents[1]
R14_LINE_SAMPLE = ROOT / "test_dwg/line_R14.dwg"
R14_ARC_SAMPLE = ROOT / "test_dwg/arc_R14.dwg"
R14_CIRCLE_SAMPLE = ROOT / "test_dwg/circle_R14.dwg"
R14_ELLIPSE_SAMPLE = ROOT / "test_dwg/ellipse_R14.dwg"
R14_POINT2D_SAMPLE = ROOT / "test_dwg/point2d_R14.dwg"
R14_POINT3D_SAMPLE = ROOT / "test_dwg/point3d_R14.dwg"
R14_LWPOLYLINE_SAMPLE = ROOT / "test_dwg/polyline2d_line_R14.dwg"


def test_ac1014_raw_headers_and_type_presence() -> None:
    assert R14_LINE_SAMPLE.exists(), f"missing sample: {R14_LINE_SAMPLE}"
    assert raw.detect_version(str(R14_LINE_SAMPLE)) == "AC1014"

    rows = raw.list_object_headers_with_type(str(R14_LINE_SAMPLE), limit=500)
    assert len(rows) >= 100

    names = {row[4] for row in rows}
    assert "LINE" in names


def test_ac1014_line_decode_smoke() -> None:
    assert R14_LINE_SAMPLE.exists(), f"missing sample: {R14_LINE_SAMPLE}"

    line_rows = raw.decode_line_entities(str(R14_LINE_SAMPLE), limit=16)
    assert len(line_rows) >= 1
    for row in line_rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_LINE_SAMPLE))
    lines = list(doc.modelspace().query("LINE"))
    assert len(lines) == len(line_rows)
    assert len(lines) >= 1
    # line_R14.dwg is a canonical diagonal line sample.
    handle, x1, y1, z1, x2, y2, z2 = line_rows[0]
    assert handle > 0
    assert abs(x1 - 50.0) < 1.0e-6
    assert abs(y1 - 50.0) < 1.0e-6
    assert abs(z1) < 1.0e-6
    assert abs(x2 - 100.0) < 1.0e-6
    assert abs(y2 - 100.0) < 1.0e-6
    assert abs(z2) < 1.0e-6


def test_ac1014_arc_decode_smoke() -> None:
    assert R14_ARC_SAMPLE.exists(), f"missing sample: {R14_ARC_SAMPLE}"
    assert raw.detect_version(str(R14_ARC_SAMPLE)) == "AC1014"

    arc_rows = raw.decode_arc_entities(str(R14_ARC_SAMPLE), limit=16)
    assert len(arc_rows) >= 1
    for row in arc_rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_ARC_SAMPLE))
    arcs = list(doc.modelspace().query("ARC"))
    assert len(arcs) == len(arc_rows)
    assert len(arcs) >= 1
    handle, cx, cy, cz, r, a0, a1 = arc_rows[0]
    assert handle > 0
    assert abs(cx - 75.0) < 1.0e-6
    assert abs(cy - 50.0) < 1.0e-6
    assert abs(cz) < 1.0e-6
    assert abs(r - 25.0) < 1.0e-6
    assert abs(a0 - 0.0) < 1.0e-6
    assert abs(a1 - math.pi) < 1.0e-6


def test_ac1014_circle_decode_smoke() -> None:
    assert R14_CIRCLE_SAMPLE.exists(), f"missing sample: {R14_CIRCLE_SAMPLE}"
    assert raw.detect_version(str(R14_CIRCLE_SAMPLE)) == "AC1014"

    circle_rows = raw.decode_circle_entities(str(R14_CIRCLE_SAMPLE), limit=16)
    assert len(circle_rows) >= 1
    for row in circle_rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_CIRCLE_SAMPLE))
    circles = list(doc.modelspace().query("CIRCLE"))
    assert len(circles) == len(circle_rows)
    assert len(circles) >= 1
    handle, cx, cy, cz, r = circle_rows[0]
    assert handle > 0
    assert abs(cx - 50.0) < 1.0e-6
    assert abs(cy - 50.0) < 1.0e-6
    assert abs(cz) < 1.0e-6
    assert abs(r - 50.0) < 1.0e-6


def test_ac1014_ellipse_decode_smoke() -> None:
    assert R14_ELLIPSE_SAMPLE.exists(), f"missing sample: {R14_ELLIPSE_SAMPLE}"
    assert raw.detect_version(str(R14_ELLIPSE_SAMPLE)) == "AC1014"

    ellipse_rows = raw.decode_ellipse_entities(str(R14_ELLIPSE_SAMPLE), limit=16)
    assert len(ellipse_rows) >= 1
    for row in ellipse_rows:
        assert row[0] > 0
        for vec in row[1:4]:
            for value in vec:
                assert math.isfinite(value)
        for value in row[4:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_ELLIPSE_SAMPLE))
    ellipses = list(doc.modelspace().query("ELLIPSE"))
    assert len(ellipses) == len(ellipse_rows)
    assert len(ellipses) >= 1
    handle, center, major_axis, extrusion, axis_ratio, start_angle, end_angle = ellipse_rows[0]
    assert handle > 0
    assert abs(center[0] - 100.0) < 1.0e-6
    assert abs(center[1] - 100.0) < 1.0e-6
    assert abs(center[2]) < 1.0e-6
    assert abs(major_axis[0] + 50.0) < 1.0e-6
    assert abs(major_axis[1] + 50.0) < 1.0e-6
    assert abs(major_axis[2]) < 1.0e-6
    assert abs(extrusion[0]) < 1.0e-6
    assert abs(extrusion[1]) < 1.0e-6
    assert abs(extrusion[2] - 1.0) < 1.0e-6
    assert abs(axis_ratio - 0.4242640687119286) < 1.0e-9
    assert abs(start_angle) < 1.0e-6
    assert abs(end_angle - (2.0 * math.pi)) < 1.0e-6


def test_ac1014_point2d_decode_smoke() -> None:
    assert R14_POINT2D_SAMPLE.exists(), f"missing sample: {R14_POINT2D_SAMPLE}"
    assert raw.detect_version(str(R14_POINT2D_SAMPLE)) == "AC1014"

    point_rows = raw.decode_point_entities(str(R14_POINT2D_SAMPLE), limit=16)
    assert len(point_rows) >= 1
    for row in point_rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_POINT2D_SAMPLE))
    points = list(doc.modelspace().query("POINT"))
    assert len(points) == len(point_rows)
    assert len(points) >= 1
    handle, x, y, z, x_axis_angle = point_rows[0]
    assert handle > 0
    assert abs(x - 50.0) < 1.0e-6
    assert abs(y - 50.0) < 1.0e-6
    assert abs(z) < 1.0e-6
    assert abs(x_axis_angle) < 1.0e-6


def test_ac1014_point3d_decode_smoke() -> None:
    assert R14_POINT3D_SAMPLE.exists(), f"missing sample: {R14_POINT3D_SAMPLE}"
    assert raw.detect_version(str(R14_POINT3D_SAMPLE)) == "AC1014"

    point_rows = raw.decode_point_entities(str(R14_POINT3D_SAMPLE), limit=16)
    assert len(point_rows) >= 1
    for row in point_rows:
        assert row[0] > 0
        for value in row[1:]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(R14_POINT3D_SAMPLE))
    points = list(doc.modelspace().query("POINT"))
    assert len(points) == len(point_rows)
    assert len(points) >= 1
    handle, x, y, z, x_axis_angle = point_rows[0]
    assert handle > 0
    # Keep this tolerant: the R14 sample keeps Y/Z at 50 and angle at zero.
    assert abs(y - 50.0) < 1.0e-6
    assert abs(z - 50.0) < 1.0e-6
    assert abs(x_axis_angle) < 1.0e-6
    assert abs(x) < 1.0e-6 or abs(x - 50.0) < 1.0e-6


def test_ac1014_lwpolyline_decode_smoke(capsys) -> None:
    assert R14_LWPOLYLINE_SAMPLE.exists(), f"missing sample: {R14_LWPOLYLINE_SAMPLE}"
    lw_rows = raw.decode_lwpolyline_entities(str(R14_LWPOLYLINE_SAMPLE), limit=16)
    assert len(lw_rows) >= 1
    handle, flags, points, bulges, widths, const_width = lw_rows[0]
    assert handle > 0
    assert flags == 0
    assert len(points) == 3
    assert abs(points[0][0] - 50.0) < 1.0e-6
    assert abs(points[0][1] - 50.0) < 1.0e-6
    assert abs(points[1][0] - 100.0) < 1.0e-6
    assert abs(points[1][1] - 100.0) < 1.0e-6
    assert abs(points[2][0] - 150.0) < 1.0e-6
    assert abs(points[2][1] - 50.0) < 1.0e-6
    assert list(bulges) == []
    assert list(widths) == []
    assert const_width is None

    doc = ezdwg.read(str(R14_LWPOLYLINE_SAMPLE))
    polylines = list(doc.modelspace().query("LWPOLYLINE"))
    assert len(polylines) == len(lw_rows)

    code = cli_module._run_inspect(str(R14_LWPOLYLINE_SAMPLE))
    captured = capsys.readouterr()
    assert code == 0
    assert "version: AC1014" in captured.out
    assert "total_entities: 1" in captured.out
    assert "LWPOLYLINE: 1" in captured.out
    assert "decode_gap[LWPOLYLINE]" not in captured.out
