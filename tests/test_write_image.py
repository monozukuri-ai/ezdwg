"""Native AC1015 IMAGE writing: IMAGEDEF table, reactors, classes and the image dictionary."""

from __future__ import annotations

import pytest

import ezdwg
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export


def images(path):
    return list(ezdwg.read(str(path)).modelspace().query("IMAGE"))


def test_image_roundtrip_with_definition(tmp_path):
    doc = ezdwg.new()
    doc.image_defs["logo"] = {"file_path": "images/logo.png", "size": (640, 480), "pixel_size": (0.1, 0.1)}
    msp = doc.modelspace()
    image = msp.add_image("logo", (10, 20), width=64, layer="0", color_index=2)
    assert image.dxf["u_vector"] == pytest.approx((0.1, 0.0, 0.0))
    assert image.dxf["v_vector"] == pytest.approx((0.0, 0.1, 0.0))
    msp.add_image("logo", (100, 20), width=32, height=32, rotation=90, fade=20, clipping=True)
    out = tmp_path / "image.dwg"
    result = doc.save(out)
    assert result.written_entities == 2 and not result.diagnostics
    back = images(out)
    assert len(back) == 2
    first = next(e for e in back if e.dxf["insert"] == (10.0, 20.0, 0.0))
    assert first.dxf["file_path"] == "images/logo.png" and first.dxf["image_def"] == "logo"
    assert first.dxf["size"] == (640.0, 480.0) and first.dxf["color_index"] == 2
    assert first.dxf["clip_type"] == 1 and first.dxf["clip_vertices"] == [(-0.5, -0.5), (639.5, 479.5)]
    rotated = next(e for e in back if e.dxf["insert"] == (100.0, 20.0, 0.0))
    assert rotated.dxf["u_vector"] == pytest.approx((0.0, 0.05, 0.0))
    assert rotated.dxf["v_vector"] == pytest.approx((-32 / 480, 0.0, 0.0))
    assert rotated.dxf["fade"] == 20 and rotated.dxf["clipping"] is True
    (definition,) = ezdwg.raw.decode_image_defs(str(out))
    assert definition[1] == "images/logo.png" and definition[2] == (640.0, 480.0)
    assert definition[5] == (0.1, 0.1)


def test_image_edit_resave_and_reexport(tmp_path):
    doc = ezdwg.new()
    doc.image_defs["scan"] = {"file_path": "scan.tif", "size": (100, 50)}
    doc.modelspace().add_image("scan", (0, 0), width=50)
    doc.modelspace().add_image("scan", (0, 60), width=25)
    source = tmp_path / "source.dwg"
    doc.save(source)
    edit = ezdwg.read(str(source)).edit()
    assert set(edit.image_defs) == {"scan"} and edit.image_defs["scan"]["file_path"] == "scan.tif"
    first = next(edit.modelspace().query("IMAGE"))
    assert first.dxf["image_def"] == "scan"
    edit.modelspace().update(first, color_index=4)
    out = tmp_path / "resaved.dwg"
    assert edit.save(out).written_entities == 2
    assert {e.dxf["color_index"] for e in images(out)} == {4, 256}
    assert ezdwg.to_dwg(str(source), str(tmp_path / "reexport.dwg"), strict=True).written_entities == 2
    assert len(images(tmp_path / "reexport.dwg")) == 2


def test_polygonal_clip_and_explicit_vectors(tmp_path):
    doc = ezdwg.new()
    doc.image_defs["map"] = {"file_path": "map.jpg", "size": (200, 100), "resolution_units": 5}
    image = doc.modelspace().add_image(
        "map",
        (5, 5),
        u_vector=(0.2, 0, 0),
        v_vector=(0, 0.3, 0),
        clip_type=2,
        clip_vertices=[(0, 0), (200, 0), (100, 100)],
        display_props=7,
        clipping=True,
    )
    out = tmp_path / "clip.dwg"
    doc.save(out)
    (back,) = images(out)
    assert back.dxf["clip_type"] == 2 and len(back.dxf["clip_vertices"]) == 3
    assert back.dxf["u_vector"] == (0.2, 0.0, 0.0) and back.dxf["v_vector"] == (0.0, 0.3, 0.0)
    assert image.dxf["size"] == (200.0, 100.0)


@pytest.mark.parametrize(
    "definition,kwargs,match",
    [
        (None, {}, "undefined image definition"),
        ({"file_path": "a.png", "size": (0, 10)}, {}, "positive"),
        ({"file_path": "", "size": (10, 10)}, {}, "file_path"),
        ({"file_path": "a.png", "size": (10, 10)}, {"u_vector": (1, 0, 0), "v_vector": (2, 0, 0)}, "parallel"),
        ({"file_path": "a.png", "size": (10, 10)}, {"brightness": 101}, "brightness"),
        ({"file_path": "a.png", "size": (10, 10)}, {"clip_type": 2, "clip_vertices": [(0, 0)]}, "three"),
    ],
)
def test_invalid_images_are_rejected(definition, kwargs, match):
    doc = ezdwg.new()
    if definition is not None:
        doc.image_defs["a"] = definition
    with pytest.raises(ValueError, match=match):
        doc.modelspace().add_image("a", (0, 0), width=10, **kwargs)


def test_image_external_read(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    doc = ezdwg.new()
    doc.image_defs["logo"] = {"file_path": "logo.png", "size": (640, 480)}
    doc.modelspace().add_image("logo", (10, 20), width=64)
    doc.modelspace().add_line((0, 0), (1, 1))
    source = tmp_path / "image.dwg"
    doc.save(source)
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf = ezdxf.readfile(convert(oda_command, resaved, tmp_path / "dxf", "DXF"))
    (image,) = dxf.modelspace().query("IMAGE")
    assert image.dxf.insert.isclose((10, 20, 0))
    assert image.dxf.image_size.isclose((640, 480))
    assert image.image_def.dxf.filename == "logo.png"
    assert image.image_def.dxf.image_size.isclose((640, 480))
    assert len(images(resaved)) == 1
