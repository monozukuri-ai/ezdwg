from __future__ import annotations

import pytest

import ezdwg.convert as convert_module

# Degree 3, closed and clamped: the first control point is repeated at the end.
CLOSED_CONTROL_POINTS = [
    (0.0, 0.0, 0.0),
    (10.0, 0.0, 0.0),
    (15.0, 8.0, 0.0),
    (10.0, 16.0, 0.0),
    (0.0, 16.0, 0.0),
    (-5.0, 8.0, 0.0),
    (0.0, 0.0, 0.0),
]
CLOSED_KNOTS = [0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0]


def _modelspace():
    ezdxf = pytest.importorskip("ezdxf")
    return ezdxf.new(dxfversion="R2018").modelspace()


def test_write_spline_keeps_decoded_closed_clamped_spline() -> None:
    msp = _modelspace()
    dxf = {
        "degree": 3,
        "closed": True,
        "periodic": True,
        "knots": CLOSED_KNOTS,
        "control_points": CLOSED_CONTROL_POINTS,
        "weights": [],
    }

    assert convert_module._write_spline(msp, dxf, {})

    spline = msp.query("SPLINE")[0]
    assert [tuple(p) for p in spline.control_points] == CLOSED_CONTROL_POINTS
    assert list(spline.knots) == CLOSED_KNOTS
    assert spline.dxf.flags & spline.CLOSED
    assert spline.dxf.flags & spline.PERIODIC
    points = list(spline.construction_tool().flattening(0.01))
    assert points[0].isclose(points[-1], abs_tol=1e-9)


def test_write_spline_keeps_decoded_rational_closed_spline() -> None:
    msp = _modelspace()
    weights = [1.0, 0.5, 2.0, 1.0, 0.5, 2.0, 1.0]
    dxf = {
        "degree": 3,
        "closed": True,
        "periodic": False,
        "rational": True,
        "knots": CLOSED_KNOTS,
        "control_points": CLOSED_CONTROL_POINTS,
        "weights": weights,
    }

    assert convert_module._write_spline(msp, dxf, {})

    spline = msp.query("SPLINE")[0]
    assert list(spline.weights) == weights
    assert list(spline.knots) == CLOSED_KNOTS
    assert spline.dxf.flags & spline.CLOSED
    assert spline.dxf.flags & spline.RATIONAL
    assert not spline.dxf.flags & spline.PERIODIC


def test_write_spline_closed_without_fitting_knots_wraps_control_points() -> None:
    msp = _modelspace()
    dxf = {
        "degree": 3,
        "closed": True,
        "knots": [0.0, 1.0],
        "control_points": CLOSED_CONTROL_POINTS,
        "weights": [],
    }

    assert convert_module._write_spline(msp, dxf, {})

    # set_closed: duplicate end point dropped, then the first `degree` points appended.
    spline = msp.query("SPLINE")[0]
    unique = CLOSED_CONTROL_POINTS[:-1]
    assert [tuple(p) for p in spline.control_points] == unique + unique[:3]
    assert list(spline.knots) == [float(i) for i in range(len(unique) + 3 + 4)]
    assert spline.dxf.flags & spline.CLOSED
    assert spline.dxf.flags & spline.PERIODIC
