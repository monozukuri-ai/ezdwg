from __future__ import annotations

from pathlib import Path

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export

CONTROL = [(0.0, 0.0, 1.0), (2.0, 5.0, 2.0), (5.0, -1.0, 3.0), (8.0, 2.0, 0.0)]
FIT = [(0.0, 0.0, 1.0), (2.0, 4.0, 2.0), (5.0, -1.0, 3.0), (8.0, 2.0, 0.0)]
KNOTS = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]


def examples():
    wrap = [(0.0, 0.0, 0.0), (4.0, 0.0, 1.0), (4.0, 3.0, 0.0), (0.0, 3.0, 2.0)]
    return [
        {"scenario": 1, "degree": 3, "control_points": CONTROL, "knots": KNOTS},
        {
            "scenario": 1,
            "degree": 2,
            "control_points": CONTROL,
            "knots": [2, 2, 2, 4, 7, 7, 7],
            "rational": True,
            "weights": [1, 0.5, 2, 1],
        },
        {
            "scenario": 1,
            "degree": 3,
            "control_points": [*CONTROL, CONTROL[0]],
            "knots": [0, 0, 0, 0, 0.5, 1, 1, 1, 1],
            "closed": True,
        },
        {
            "scenario": 1,
            "degree": 3,
            "control_points": wrap + wrap[:3],
            "knots": list(range(11)),
            "closed": True,
            "periodic": True,
        },
        {
            "scenario": 2,
            "degree": 3,
            "fit_points": FIT,
            "fit_tolerance": 1e-8,
            "start_tangent": (1.0, 0.0, 0.0),
            "end_tangent": (0.0, 1.0, 0.0),
        },
        {"scenario": 2, "degree": 3, "fit_points": FIT[:2]},
    ]


def assert_external(entity, expected):
    assert entity.dxftype() == "SPLINE"
    assert entity.dxf.degree == expected["degree"]
    if expected["scenario"] == 1:
        if not expected.get("periodic", False):
            for p, q in zip(
                entity.control_points, expected["control_points"], strict=True
            ):
                assert tuple(p) == pytest.approx(q)
            assert list(entity.weights) == pytest.approx(expected.get("weights", []))
        assert bool(entity.dxf.flags & 1) == expected.get("closed", False)
        # ODA exports DXF periodic for closed non-periodic curves too, and may
        # clamp periodic knots/control points. Compare the evaluated curve below.
        if expected.get("periodic", False):
            assert entity.dxf.flags & 2
        assert bool(entity.dxf.flags & 4) == expected.get("rational", False)
        # ODA may normalize parameter values; compare geometry at equal domain fractions.
        from ezdxf.math import BSpline

        reference = BSpline(
            expected["control_points"],
            order=expected["degree"] + 1,
            knots=expected["knots"],
            weights=expected.get("weights"),
        )
        actual = entity.construction_tool()
        knots = reference.knots()
        actual_knots = actual.knots()
        p, n = expected["degree"], len(expected["control_points"])
        for fraction in (0, 0.1, 0.3, 0.5, 0.8, 1):
            u = knots[p] + (knots[n] - knots[p]) * fraction
            v = actual_knots[p] + (actual_knots[n] - actual_knots[p]) * fraction
            assert tuple(actual.point(v)) == pytest.approx(
                tuple(reference.point(u)), abs=1e-8
            )
    else:
        for p, q in zip(entity.fit_points, expected["fit_points"], strict=True):
            assert tuple(p) == pytest.approx(q)
        for key in ("start_tangent", "end_tangent"):
            if key in expected:
                assert tuple(entity.dxf.get(key)) == pytest.approx(expected[key])
        assert entity.dxf.fit_tolerance == pytest.approx(
            expected.get("fit_tolerance", 1e-10)
        )


def test_spline_new_edit_snapshot_and_helpers(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    control = msp.add_spline(CONTROL)
    assert control.dxf["knots"] == KNOTS
    fit = msp.add_spline_fit(FIT, start_tangent=(1, 0, 0), end_tangent=(0, 1, 0))
    doc.layers["曲線"] = {"color_index": 3}
    msp.update(control, layer="曲線", color_index=2, lineweight=35, invisible=True)
    path = tmp_path / "spline.dwg"
    result = doc.save(path)
    assert result.written_entities == 2 and not result.diagnostics
    rows = {r["handle"]: r for r in ezdwg.raw.decode_write_splines(str(path))}
    assert rows[control.handle]["control_points"] == CONTROL
    assert rows[fit.handle]["start_tangent"] == (1, 0, 0)
    read = ezdwg.read(str(path))
    entity = next(read.modelspace().query("SPLINE"))
    assert entity.dxf["control_points"] == CONTROL
    assert entity.dxf["lineweight"] == 35 and entity.dxf["invisible"]
    edited = read.edit()
    assert edited.modelspace().get(fit.handle).dxf["end_tangent"] == (0, 1, 0)
    edited.modelspace().update(control.handle, rational=True, weights=[1, 0.5, 2, 1])
    path.unlink()
    edited.save(path)
    assert ezdwg.raw.decode_write_splines(str(path))[0]["weights"] == [1, 0.5, 2, 1]


def test_spline_oda_control_fit_periodic_and_blocks(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    shapes = examples()
    for data in shapes:
        doc.modelspace().add_entity("SPLINE", data)
    part = doc.blocks.new("Part")
    part.add_entity("SPLINE", shapes[1])
    assembly = doc.blocks.new("Assembly")
    assembly.add_blockref("Part", (10, 20), scale=(-2, 3, 1))
    doc.modelspace().add_blockref("Assembly", (100, 200))
    path = tmp_path / "splines.dwg"
    doc.save(path)
    direct = convert(oda_command, path, tmp_path / "direct", "DXF")
    resaved = convert(oda_command, path, tmp_path / "resaved", "DWG")
    again = convert(oda_command, resaved, tmp_path / "again", "DXF")
    for output in (direct, again):
        external = ezdxf.readfile(output)
        audit = external.audit()
        assert not audit.errors and not audit.fixes
        for e, expected in zip(
            external.modelspace().query("SPLINE"), shapes, strict=True
        ):
            assert_external(e, expected)
        assert_external(external.blocks["Part"].query("SPLINE").first, shapes[1])
    filtered = tmp_path / "filtered.dwg"
    edited = ezdwg.read(str(resaved)).edit()
    edited.save(tmp_path / "edited-resaved.dwg")
    result = ezdwg.to_dwg(str(resaved), str(filtered), types="INSERT", strict=True)
    assert result.written_entities == 3
    assert len(ezdwg.read(str(filtered)).edit().blocks["Part"]) == 1


@pytest.mark.parametrize(
    "version", ["AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032"]
)
def test_existing_spline_fixture_export_and_known_losses(tmp_path, version):
    source = (
        Path(__file__).resolve().parents[1]
        / "test_dwg"
        / "acadsharp"
        / f"sample_{version}.dwg"
    )
    expected = ezdwg.raw.decode_spline_entities(str(source))
    assert expected
    output = tmp_path / "spline.dwg"
    with pytest.raises(ValueError, match="xdata|knot_parameter"):
        ezdwg.to_dwg(str(source), str(output), types="SPLINE", strict=True)
    assert not output.exists()
    result = ezdwg.to_dwg(str(source), str(output), types="SPLINE", strict=False)
    assert result.diagnostics
    actual = ezdwg.raw.decode_spline_entities(str(output))
    assert actual[0] == expected[0]
    if version in {"AC1027", "AC1032"}:
        assert result.skipped_by_type == {"SPLINE": 1}
    else:
        assert actual == expected


INVALID = [
    {"knots": [-1e308] * 4 + [1e308] * 4},
    {"closed": True},
    {"degree": 0},
    {"degree": 11},
    {"degree": 2.5},
    {"scenario": 3},
    {"knots": KNOTS[:-1]},
    {"knots": [0, 0, 0, 0, 1, 0.5, 1, 1]},
    {"knots": [0] * 8},
    {"knots": [0, 0, 0, 0, 0, 1, 1, 1]},
    {"knots": [0, 0, 0, 0, 1, 1, 1, float("inf")]},
    {"control_points": CONTROL[:3]},
    {"control_points": [(float("nan"), 0, 0), *CONTROL[1:]]},
    {"rational": True},
    {"rational": True, "weights": [1, 0, 1, 1]},
    {"rational": True, "weights": [1, -1, 1, 1]},
    {"rational": True, "weights": [1, float("nan"), 1, 1]},
    {"weights": [1] * 4},
    {"knot_tolerance": -1},
    {"ctrl_tolerance": float("inf")},
    {"fit_tolerance": 0},
    {"start_tangent": (1, 0, 0)},
    {"fit_points": FIT},
    {"closed": False, "periodic": True},
    {"scenario": 2, "control_points": [], "knots": [], "fit_points": FIT[:1]},
    {"scenario": 2, "control_points": [], "knots": [], "fit_points": FIT, "degree": 2},
    {
        "scenario": 2,
        "control_points": [],
        "knots": [],
        "fit_points": [FIT[0], FIT[0], FIT[1]],
    },
    {"scenario": 2, "control_points": [], "knots": [], "fit_points": [*FIT, FIT[0]]},
    {
        "scenario": 2,
        "control_points": [],
        "knots": [],
        "fit_points": FIT,
        "end_tangent": (float("nan"), 0, 0),
    },
]


@pytest.mark.parametrize("invalid", INVALID)
def test_invalid_spline_edit_raw_and_strict_preserve_destination(tmp_path, invalid):
    doc = ezdwg.new()
    spline = doc.modelspace().add_spline(CONTROL)
    original = dict(spline.dxf)
    with pytest.raises((ValueError, TypeError)):
        doc.modelspace().update(spline, **invalid)
    assert spline.dxf == original
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    with pytest.raises((ValueError, TypeError, OverflowError)):
        ezdwg.raw.write_ac1015_dwg(
            str(path),
            [],
            [],
            [],
            [],
            [],
            [],
            splines=[{"handle": 10, **examples()[0], **invalid}],
        )
    assert path.read_bytes() == b"keep"
    spline.dxf.update(invalid)
    with pytest.raises(ValueError):
        doc.save(path)
    assert path.read_bytes() == b"keep"
    result = doc.save(path, strict=False)
    assert result.skipped_by_type == {"SPLINE": 1}


@pytest.mark.parametrize(
    "property,value", [("thickness", 2), ("extrusion", (0, 1, 0)), ("elevation", 3)]
)
def test_spline_rejects_nonrepresentable_formatting(tmp_path, property, value):
    path = tmp_path / "keep.dwg"
    path.write_bytes(b"keep")
    with pytest.raises(ValueError):
        ezdwg.new().modelspace().add_spline(CONTROL, **{property: value})
    with pytest.raises(ValueError):
        ezdwg.raw.write_ac1015_dwg(
            str(path),
            [],
            [],
            [],
            [],
            [],
            [],
            splines=[{"handle": 10, **examples()[0]}],
            entity_attributes={10: {property: value}},
        )
    assert path.read_bytes() == b"keep"


@pytest.mark.parametrize("missing", [True, False])
def test_spline_import_metadata_loss_is_reported(tmp_path, monkeypatch, missing):
    doc = ezdwg.new()
    doc.modelspace().add_spline(CONTROL)
    source = tmp_path / "source.dwg"
    doc.save(source)
    rows = ezdwg.raw.decode_write_splines(str(source))
    rows[0]["spline_flags1"] = 2  # CV frame display is not stored in AC1015.
    monkeypatch.setattr(
        ezdwg.raw, "decode_write_splines", lambda _: [] if missing else rows
    )
    output = tmp_path / "keep.dwg"
    output.write_bytes(b"keep")
    with pytest.raises(ValueError, match="metadata|display"):
        ezdwg.to_dwg(str(source), str(output), strict=True)
    assert output.read_bytes() == b"keep"
    result = ezdwg.to_dwg(str(source), str(output), strict=False)
    assert result.diagnostics
    assert result.written_entities == (0 if missing else 1)


@pytest.mark.parametrize("degree", [1, 10])
def test_spline_degree_limits_external_read(oda_command, tmp_path, degree):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    controls = [(float(i), float(i % 3), float(i % 2)) for i in range(degree + 1)]
    entity = doc.modelspace().add_spline(controls, degree=degree)
    path = tmp_path / "degree.dwg"
    doc.save(path)
    external = ezdxf.readfile(convert(oda_command, path, tmp_path / "output", "DXF"))
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    assert_external(external.modelspace().query("SPLINE").first, entity.dxf)


@pytest.mark.parametrize("version", ["ACAD2000", "ACAD2004", "ACAD2013"])
def test_independent_spline_control_fit_import_edit_export(
    oda_command, tmp_path, version
):
    ezdxf = pytest.importorskip("ezdxf")
    source = ezdxf.new(
        {"ACAD2000": "R2000", "ACAD2004": "R2004", "ACAD2013": "R2013"}[version]
    )
    shapes = examples()
    for data in shapes:
        if data["scenario"] == 1:
            if data.get("weights"):
                source.modelspace().add_rational_spline(
                    data["control_points"],
                    data["weights"],
                    degree=data["degree"],
                    knots=data["knots"],
                )
            else:
                e = source.modelspace().add_open_spline(
                    data["control_points"], degree=data["degree"], knots=data["knots"]
                )
                e.dxf.flags = int(data.get("closed", False)) | (
                    int(data.get("periodic", False)) << 1
                )
        else:
            e = source.modelspace().add_spline(data["fit_points"], degree=3)
            for key in ("fit_tolerance", "start_tangent", "end_tangent"):
                if key in data:
                    e.dxf.set(key, data[key])
    path = tmp_path / "independent.dxf"
    source.saveas(path)
    dwg = convert(oda_command, path, tmp_path / "input", "DWG", version)
    # Compare against the independent DWG's external geometry after ODA's own
    # parameter/closed-curve normalization, rather than assuming unchanged DXF flags.
    baseline = ezdxf.readfile(convert(oda_command, dwg, tmp_path / "baseline", "DXF"))
    doc = ezdwg.read(str(dwg)).edit()
    assert len(doc.modelspace()) == len(shapes)
    for e in doc.modelspace().query("SPLINE"):
        doc.modelspace().update(e, color_index=4)
    first = next(doc.modelspace().query("SPLINE"))
    moved = [(x, y, z + 1) for x, y, z in first.dxf["control_points"]]
    doc.modelspace().update(first, control_points=moved)
    output = tmp_path / "native.dwg"
    doc.save(output)
    external = ezdxf.readfile(convert(oda_command, output, tmp_path / "output", "DXF"))
    audit = external.audit()
    assert not audit.errors and not audit.fixes
    for i, (a, b) in enumerate(
        zip(
            baseline.modelspace().query("SPLINE"),
            external.modelspace().query("SPLINE"),
            strict=True,
        )
    ):
        assert b.dxf.color == 4
        ac, bc = a.construction_tool(), b.construction_tool()
        ak, bk = ac.knots(), bc.knots()
        for fraction in (0, 0.15, 0.35, 0.5, 0.8, 1):
            ap = ac.point(ak[ac.degree] + (ak[ac.count] - ak[ac.degree]) * fraction)
            bp = bc.point(bk[bc.degree] + (bk[bc.count] - bk[bc.degree]) * fraction)
            assert tuple(bp) == pytest.approx(
                (ap.x, ap.y, ap.z + (1 if i == 0 else 0)), abs=1e-8
            )
