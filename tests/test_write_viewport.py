"""Native AC1015 paper space: the Layout1 sheet, *Paper_Space entities and VIEWPORTs."""

from __future__ import annotations

import pytest

import ezdwg
from ezdwg._viewport import MODEL_VIEWPORT_STATUS, PAPER_VIEWPORT_STATUS, VIEWPORT_LOCKED
from tests.test_write_dwg_oda import convert
from tests.test_write_dwg_oda import oda_command as oda_command  # noqa: PLC0414 -- pytest fixture re-export


def sheet_drawing() -> ezdwg.Drawing:
    """A 20 m x 10 m rectangle at real size (mm) on an A3 sheet at 1:100."""
    doc = ezdwg.new()
    doc.layers["WALLS"] = {"color_index": 3}
    doc.layers["HIDDEN"] = {"color_index": 1}
    msp = doc.modelspace()
    corners = [(0, 0), (20000, 0), (20000, 10000), (0, 10000)]
    for start, end in zip(corners, corners[1:] + corners[:1]):
        msp.add_line(start, end, layer="WALLS")
    msp.add_circle((10000, 5000), 1000, layer="HIDDEN")
    doc.set_paper(420, 297, margins=(5, 5, 5, 5))
    psp = doc.paperspace()
    psp.add_viewport(
        (205, 143.5),
        400,
        277,
        view_center=(10000, 5000),
        view_height=277 * 100,
        frozen_layers=["HIDDEN"],
    )
    psp.add_text("A3 1:100", (10, 5), 5)
    psp.add_line((-5, -5), (415, -5), color_index=5)
    return doc


def paper_viewports(doc: ezdwg.Document) -> tuple[ezdwg.Entity, list[ezdwg.Entity]]:
    """The sheet's own viewport and the user viewports of a read drawing."""
    viewports = list(doc.paperspace().query("VIEWPORT"))
    sheet = [v for v in viewports if v.dxf["status_flags"] == PAPER_VIEWPORT_STATUS]
    assert len(sheet) == 1
    return sheet[0], [v for v in viewports if v is not sheet[0]]


def test_paper_space_roundtrip_through_the_reader(tmp_path):
    doc = sheet_drawing()
    assert doc.layout["paper_size"] == "ISO_A3_(420.00_x_297.00_MM)"
    out = tmp_path / "sheet.dwg"
    result = doc.save(out)
    assert result.written_entities == 8 and not result.diagnostics

    back = ezdwg.read(str(out))
    assert sorted(e.dxftype for e in back.modelspace().query("*")) == ["CIRCLE"] + ["LINE"] * 4
    paper = list(back.paperspace().query("*"))
    assert sorted(e.dxftype for e in paper) == ["LINE", "TEXT", "VIEWPORT", "VIEWPORT"]
    # Stored like AutoCAD's current paper-space layout: entmode 1, no owner handle.
    assert {back.entity_placement(e.handle) for e in paper} == {(1, None)}

    sheet, (viewport,) = paper_viewports(back)
    assert sheet.dxf["center"] == (205.0, 143.5, 0.0)
    assert (sheet.dxf["width"], sheet.dxf["height"]) == (420.0, 297.0)
    assert sheet.dxf["view_center"] == (205.0, 143.5) and sheet.dxf["view_height"] == 297.0
    assert sheet.dxf["view_direction"] == (0.0, 0.0, 1.0) and sheet.dxf["lens_length"] == 50.0
    assert viewport.dxf["center"] == (205.0, 143.5, 0.0)
    assert (viewport.dxf["width"], viewport.dxf["height"]) == (400.0, 277.0)
    assert viewport.dxf["view_center"] == (10000.0, 5000.0)
    assert viewport.dxf["view_height"] / viewport.dxf["height"] == pytest.approx(100.0)
    assert viewport.dxf["status_flags"] == MODEL_VIEWPORT_STATUS | VIEWPORT_LOCKED
    assert viewport.dxf["frozen_layers"] == ["HIDDEN"]
    assert viewport.dxf["render_mode"] == 0 and viewport.dxf["view_twist_angle"] == 0.0

    layouts = back.layouts()
    layout = layouts["Layout1"]
    assert (layout["paper_width"], layout["paper_height"]) == (420.0, 297.0)
    assert layout["margins"] == (5.0, 5.0, 5.0, 5.0)
    assert layout["paper_size"] == "ISO_A3_(420.00_x_297.00_MM)"
    assert layout["paper_units"] == 1 and layout["plot_rotation"] == 0
    assert (layout["plot_layout_flags"], layout["plot_type"]) == (688, 5)
    assert layout["limmin"] == (-5.0, -5.0) and layout["limmax"] == (415.0, 292.0)
    assert layout["extmin"] == (-5.0, -5.0, 0.0) and layout["extmax"] == (415.0, 292.0, 0.0)
    assert layout["last_active_viewport_handle"] == sheet.handle
    assert layout["active"] is True
    assert layouts["Model"]["model"] is True and layouts["Model"]["last_active_viewport_handle"] == 0


def test_sheet_viewport_only_with_paper_space_entities(tmp_path):
    doc = ezdwg.new()
    doc.modelspace().add_line((0, 0), (1, 1))
    out = tmp_path / "model_only.dwg"
    doc.save(out)
    back = ezdwg.read(str(out))
    assert not list(back.paperspace().query("*"))
    assert back.layouts()["Layout1"]["last_active_viewport_handle"] == 0
    assert back.layouts()["Layout1"]["extmax"] == (0.0, 0.0, 0.0)

    # A title block alone initialises the sheet with its own viewport.
    doc.paperspace().add_text("Title", (10, 10), 5)
    doc.save(out)
    back = ezdwg.read(str(out))
    sheet, others = paper_viewports(back)
    assert not others and sheet.dxf["width"] == 420.0
    assert back.layouts()["Layout1"]["last_active_viewport_handle"] == sheet.handle


def test_paper_space_layout_export_and_set_paper(tmp_path):
    doc = sheet_drawing()
    psp = doc.paperspace()
    # Exporting the paper-space layout alone writes only its entities.
    out = tmp_path / "paper_only.dwg"
    assert psp.export_dwg(out).written_entities == 3
    back = ezdwg.read(str(out))
    assert not list(back.modelspace().query("*"))
    assert len(list(back.paperspace().query("VIEWPORT"))) == 2

    assert doc.set_paper(297, 420, name="Sheet A", plot_rotation=1)["paper_size"] == "ISO_A3_(297.00_x_420.00_MM)"
    assert doc.set_paper(500, 300, paper_size="Custom")["paper_size"] == "Custom"
    assert doc.set_paper(1000, 700)["paper_size"] == "User_(1000.00_x_700.00_MM)"
    doc.save(out)
    layout = ezdwg.read(str(out)).layouts()["Sheet A"]
    assert (layout["paper_width"], layout["paper_height"]) == (1000.0, 700.0)
    assert layout["plot_rotation"] == 1
    # Rotated paper: the layout plane spans height x width.
    assert layout["limmax"] == (695.0, 995.0)


@pytest.mark.parametrize(
    "kwargs,match",
    [
        ({"width": 0}, "positive"),
        ({"view_height": -1}, "positive"),
        ({"view_center": (1, 2, 3)}, "two coordinates"),
        ({"view_direction": (0, 0, 0)}, "nonzero"),
        ({"status_flags": -1}, "status_flags"),
        ({"frozen_layers": ["A", "a"]}, "twice"),
        ({"render_mode": 7}, "render_mode"),
    ],
)
def test_invalid_viewports_are_rejected(kwargs, match):
    doc = ezdwg.new()
    arguments = {
        "center": (210, 148.5),
        "width": 400,
        "height": 277,
        "view_center": (0, 0),
        "view_height": 100,
    }
    arguments.update(kwargs)
    with pytest.raises(ValueError, match=match):
        doc.paperspace().add_viewport(**arguments)


def test_viewport_requires_paper_space_and_defined_layers(tmp_path):
    doc = ezdwg.new()
    with pytest.raises(ValueError, match="paper space"):
        doc.modelspace().add_viewport((0, 0), 1, 1, view_center=(0, 0), view_height=1)
    with pytest.raises(ValueError, match="paper space"):
        doc.blocks.new("B").add_entity(
            "VIEWPORT", {"center": (0, 0), "width": 1, "height": 1, "view_center": (0, 0), "view_height": 1}
        )
    doc.paperspace().add_viewport((0, 0), 1, 1, view_center=(0, 0), view_height=1, frozen_layers=["MISSING"])
    with pytest.raises(ValueError, match="undefined layer"):
        doc.save(tmp_path / "bad.dwg")


@pytest.mark.parametrize(
    "layout,match",
    [
        ({"paper_width": 0}, "positive"),
        ({"margins": (300, 0, 200, 0)}, "printable"),
        ({"name": "Model"}, "Model"),
        ({"plot_rotation": 4}, "plot_rotation"),
        ({"unknown": 1}, "unsupported layout"),
    ],
)
def test_invalid_layouts_are_rejected(tmp_path, layout, match):
    doc = ezdwg.new()
    doc.modelspace().add_line((0, 0), (1, 1))
    doc.layout.update(layout)
    with pytest.raises(ValueError, match=match):
        doc.save(tmp_path / "bad.dwg")


def test_existing_paper_space_is_ignored_by_imports(tmp_path):
    """Imports keep model space and blocks; paper space is reported as before."""
    source = tmp_path / "sheet.dwg"
    sheet_drawing().save(source)
    with pytest.raises(ValueError, match="paper space"):
        ezdwg.read(str(source)).edit()
    edited = ezdwg.read(str(source)).edit(strict=False)
    assert [d.attribute for d in edited.diagnostics if d.dxftype == "DOCUMENT"] == ["spaces"]
    assert len(list(edited.modelspace())) == 5 and not list(edited.paperspace())
    assert edited.layout["paper_width"] == 420.0 and edited.layout["paper_size"] == ""
    out = tmp_path / "edited.dwg"
    assert edited.save(out, strict=False).written_entities == 5
    assert not list(ezdwg.read(str(out)).paperspace().query("*"))
    assert ezdwg.to_dwg(str(source), str(tmp_path / "reexport.dwg")).written_entities == 5


def test_paper_space_external_read(oda_command, tmp_path):
    ezdxf = pytest.importorskip("ezdxf")
    source = tmp_path / "sheet.dwg"
    sheet_drawing().save(source)
    resaved = convert(oda_command, source, tmp_path / "resaved", "DWG")
    dxf = ezdxf.readfile(convert(oda_command, resaved, tmp_path / "dxf", "DXF"))
    audit = dxf.audit()
    assert not audit.errors and not audit.fixes
    assert len(dxf.modelspace()) == 5

    layout = dxf.layouts.get("Layout1")
    settings = layout.dxf_layout.dxf
    assert (settings.paper_width, settings.paper_height) == (420.0, 297.0)
    assert settings.paper_size == "ISO_A3_(420.00_x_297.00_MM)"
    assert (settings.left_margin, settings.bottom_margin, settings.right_margin, settings.top_margin) == (5.0,) * 4
    assert (settings.plot_paper_units, settings.plot_type, settings.standard_scale_type) == (1, 5, 16)
    assert settings.plot_layout_flags == 688
    assert settings.limmin.isclose((-5, -5, 0)) and settings.limmax.isclose((415, 292, 0))
    assert dxf.header["$PLIMMIN"] == (-5.0, -5.0) and dxf.header["$PLIMMAX"] == (415.0, 292.0)
    assert sorted(e.dxftype() for e in layout) == ["LINE", "TEXT", "VIEWPORT", "VIEWPORT"]
    assert all(e.dxf.paperspace == 1 for e in layout)

    sheet, viewport = layout.viewports()
    assert sheet.dxf.id == 1 and sheet.dxf.flags == PAPER_VIEWPORT_STATUS
    assert sheet.dxf.center.isclose((205, 143.5, 0))
    assert (sheet.dxf.width, sheet.dxf.height) == (420.0, 297.0)
    assert settings.viewport_handle == sheet.dxf.handle
    assert viewport.dxf.id == 2
    assert viewport.dxf.flags == MODEL_VIEWPORT_STATUS | VIEWPORT_LOCKED
    assert viewport.dxf.center.isclose((205, 143.5, 0))
    assert (viewport.dxf.width, viewport.dxf.height) == (400.0, 277.0)
    assert viewport.dxf.view_center_point.isclose((10000, 5000, 0))
    assert viewport.dxf.view_height / viewport.dxf.height == pytest.approx(100.0)
    assert viewport.dxf.view_direction_vector.isclose((0, 0, 1))
    assert viewport.dxf.perspective_lens_length == 50.0 and viewport.dxf.circle_zoom == 1000
    # The UCS tail of the body decodes in place: world UCS, elevation 0.
    assert viewport.dxf.ucs_per_viewport == 1 and viewport.dxf.ucs_ortho_type == 0
    assert viewport.dxf.ucs_x_axis.isclose((1, 0, 0)) and viewport.dxf.ucs_y_axis.isclose((0, 1, 0))
    assert viewport.dxf.elevation == 0.0 and viewport.dxf.render_mode == 0

    # ODA writes the frozen layers (331) of a viewport in DXF 2018 only.
    modern = ezdxf.readfile(convert(oda_command, resaved, tmp_path / "dxf2018", "DXF", "ACAD2018"))
    _, viewport = modern.layouts.get("Layout1").viewports()
    assert viewport.dxf.id == 2 and viewport.frozen_layers == ["HIDDEN"]

    # ODA keeps the sheet readable by ezdwg as well.
    back = ezdwg.read(str(resaved))
    sheet, (model,) = paper_viewports(back)
    assert model.dxf["view_height"] == 27700.0 and model.dxf["frozen_layers"] == ["HIDDEN"]
    assert back.layouts()["Layout1"]["paper_width"] == 420.0
