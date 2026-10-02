"""One drawing saved in every file format version (public ACadSharp samples).

The files share their entity handles, so what is read from one version can be
checked against the others. The expected values come from the DXF export of the
drawing.
"""

from __future__ import annotations

import math
from collections import Counter
from pathlib import Path
from typing import Any

import pytest

import ezdwg


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg" / "acadsharp"
VERSIONS = ("AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032")
# R13/R14 files have neither lineweights nor a plot flag.
R2000_PLUS = VERSIONS[1:]
R2010_PLUS = ("AC1024", "AC1027", "AC1032")


def sample(version: str) -> str:
    return str(SAMPLES / f"sample_{version}.dwg")


@pytest.fixture(scope="module", params=VERSIONS)
def document(request: pytest.FixtureRequest) -> ezdwg.Document:
    return ezdwg.read(sample(request.param))


def _entities(doc: ezdwg.Document, dxftype: str) -> dict[int, dict[str, Any]]:
    return {entity.handle: entity.dxf for entity in doc.entities().query(dxftype)}


# ---------------------------------------------------------------- layer state


def test_layer_states(document: ezdwg.Document) -> None:
    # R14 keeps symbol names in upper case.
    layers = {name.upper(): layer for name, layer in document.layers().items()}
    r14 = document.version == "AC1014"

    def state(name: str) -> tuple[bool, bool, bool]:
        layer = layers[name]
        return layer["off"], layer["frozen"], layer["locked"]

    assert state("0") == (False, False, False)
    assert state("LAYER_OFF") == (True, False, False)
    assert state("LAYER_FREEZE") == (False, True, False)
    assert state("LAYER_LOCK") == (False, False, True)

    # The plot flag and the lineweight of a layer came with R2000.
    assert layers["0"]["plot"] is True
    assert layers["LAYER_NOPLOT"]["plot"] is r14
    assert layers["DEFPOINTS"]["plot"] is r14
    assert layers["0"]["lineweight"] == -3
    assert layers["LAYER_LW_035"]["lineweight"] == (-3 if r14 else 35)
    assert layers["LAYER_LW_050"]["lineweight"] == (-3 if r14 else 50)


@pytest.mark.parametrize("version", VERSIONS)
def test_raw_layer_states_cover_every_layer(version: str) -> None:
    names = dict(ezdwg.raw.decode_layer_names(sample(version)))
    rows = ezdwg.raw.decode_layer_states(sample(version))

    assert {handle for handle, *_ in rows} == set(names)
    by_name = {names[handle].upper(): tuple(state) for handle, *state in rows}
    # (frozen, off, frozen in new viewports, locked, plot, lineweight)
    assert by_name["LAYER_OFF"][:4] == (False, True, False, False)
    assert by_name["LAYER_FREEZE"][:4] == (True, False, False, False)
    assert by_name["LAYER_LOCK"][:4] == (False, False, False, True)
    assert by_name["LAYER_VP_FREEZE"][:4] == (False, False, True, False)


@pytest.mark.parametrize("version", R2010_PLUS)
def test_layer_with_a_color_book_color_keeps_its_name(version: str) -> None:
    # The string stream of this layer also holds the color and book names.
    layers = ezdwg.read(sample(version)).layers()

    assert "Layer_color_book" in layers
    assert "DIC COLOR GUIDE(R)" not in layers


# ------------------------------------------------- lineweight and visibility


def test_entity_visibility(document: ezdwg.Document) -> None:
    invisible = sorted(
        entity.handle
        for entity in document.entities().query()
        if entity.dxf.get("invisible")
    )
    if document.version == "AC1014":
        # The two lines exist under other handles in the R14 file.
        assert len(invisible) == 2
    else:
        assert invisible == [0x58C, 0xA51]
    lines = _entities(document, "LINE")
    assert sum(1 for dxf in lines.values() if dxf["invisible"] is False) == len(lines) - 2


@pytest.mark.parametrize("version", R2000_PLUS)
def test_entity_lineweights(version: str) -> None:
    doc = ezdwg.read(sample(version))
    lines = _entities(doc, "LINE")
    inserts = _entities(doc, "INSERT")

    # Hundredths of a millimetre; -1 is BYLAYER and -2 BYBLOCK.
    weights = Counter(dxf["lineweight"] for dxf in lines.values())
    assert weights[0] == 2 and weights[13] == 9 and weights[15] == 2 and weights[40] == 4
    assert set(weights) == {-2, -1, 0, 13, 15, 40}
    assert lines[0x2CF]["lineweight"] == 13
    assert lines[0x58C]["lineweight"] == 0
    assert Counter(dxf["lineweight"] for dxf in inserts.values()) == {-2: 2, -1: 10, 15: 2}


def test_r14_entities_have_no_lineweight() -> None:
    lines = _entities(ezdwg.read(sample("AC1014")), "LINE")

    assert {dxf["lineweight"] for dxf in lines.values()} == {None}


@pytest.mark.parametrize("version", R2010_PLUS)
def test_common_entity_data_behind_a_large_graphic(version: str) -> None:
    # These MULTILEADER and ACAD_TABLE entities carry a saved graphic of more
    # than 255 bytes, whose size is a two-byte BLL.
    rows = {
        handle: (lineweight, invisible)
        for handle, lineweight, invisible in ezdwg.raw.decode_entity_lineweights(sample(version))
    }

    for handle in (0x528, 0xB1E, 0xB22, 0xB25, 0xB27):
        assert rows[handle] == (-1, False)


# ----------------------------------------------------------------- attributes

ATTRIBS = {
    0x705: ("ATTINFO", "17", (920.6796, 16.3529), 1.8169),
    0x79F: ("PRESET_ATT", "hello", (994.5583, 41.7095), 1.7207),
    0x7A0: ("VERIFY_ATT", "bla bla", (986.6882, 1.5545), 1.7207),
}
ATTDEFS = {
    0x6F8: ("ATTINFO", "1", "Enter number:", 0),
    0x796: ("MULTI_LINE_ATT", "", "this is an example of prompt", 0),
    0x797: ("PRESET_ATT", "", "a preset prompt", 8),
    0x798: ("CONSTANT_ATT", "", "", 2),
    0x799: ("VERIFY_ATT", "", "verify_prompt", 4),
}


def test_attribs_of_every_version(document: ezdwg.Document) -> None:
    attribs = _entities(document, "ATTRIB")

    assert set(attribs) == {*ATTRIBS, 0x79D}
    for handle, (tag, text, insert, height) in ATTRIBS.items():
        dxf = attribs[handle]
        assert (dxf["tag"], dxf["text"]) == (tag, text)
        assert dxf["insert"][:2] == pytest.approx(insert, abs=1e-3)
        assert dxf["height"] == pytest.approx(height, abs=1e-3)
        assert dxf["attribute_flags"] == 0
    # Top-aligned: the alignment point is the anchor of the text.
    preset = attribs[0x79F]
    assert (preset["halign"], preset["valign"]) == (0, 3)
    assert preset["align_point"][:2] == pytest.approx((994.5583, 43.4301), abs=1e-3)

    # A multi-line attribute. R2018 stores it as one attribute with an embedded
    # MTEXT; earlier versions store one single-line attribute per line.
    multiline = attribs[0x79D]
    assert multiline["text"] == "my multi line text for the attrrib"
    if document.version == "AC1032":
        assert multiline["tag"] == "MULTI_LINE_ATT"
        assert multiline["valign"] == 3
    else:
        assert multiline["tag"] == "MULTI_LINE_ATT_001"


def test_attribs_belong_to_their_insert(document: ezdwg.Document) -> None:
    owners = {
        handle: document.entity_placement(handle) for handle in (0x705, 0x79D, 0x79F, 0x7A0)
    }

    assert owners == {
        0x705: (0, 0x704),
        0x79D: (0, 0x79C),
        0x79F: (0, 0x79C),
        0x7A0: (0, 0x79C),
    }
    inserts = _entities(document, "INSERT")
    assert {0x704, 0x79C} <= set(inserts)


def test_attdefs_of_every_version(document: ezdwg.Document) -> None:
    attdefs = _entities(document, "ATTDEF")

    assert set(attdefs) == set(ATTDEFS)
    for handle, (tag, text, prompt, flags) in ATTDEFS.items():
        dxf = attdefs[handle]
        assert (dxf["tag"], dxf["text"], dxf["prompt"]) == (tag, text, prompt)
        assert dxf["attribute_flags"] == flags
        assert dxf["height"] > 0


# ------------------------------------------------------------ text and mtext


@pytest.mark.parametrize("version", [v for v in VERSIONS if v != "AC1018"])
def test_text_and_mtext_match_the_r2004_save(version: str) -> None:
    # R2004 stores its strings inline; R2007 and later keep them in the string
    # stream of the object.
    reference = ezdwg.read(sample("AC1018"))
    doc = ezdwg.read(sample(version))

    texts = _entities(doc, "TEXT")
    expected_texts = _entities(reference, "TEXT")
    assert set(texts) == set(expected_texts) and len(texts) == 29
    for handle, expected in expected_texts.items():
        dxf = texts[handle]
        assert dxf["text"] == expected["text"], hex(handle)
        assert (dxf["halign"], dxf["valign"]) == (expected["halign"], expected["valign"])
        assert dxf["insert"] == pytest.approx(expected["insert"])
        assert dxf["height"] == pytest.approx(expected["height"])

    mtexts = _entities(doc, "MTEXT")
    expected_mtexts = _entities(reference, "MTEXT")
    assert len(mtexts) == len(expected_mtexts) == 36
    shared = set(mtexts) & set(expected_mtexts)
    assert len(shared) >= 32
    for handle in shared:
        dxf, expected = mtexts[handle], expected_mtexts[handle]
        assert dxf["text"] == expected["text"], hex(handle)
        assert dxf["insert"] == pytest.approx(expected["insert"])
        assert dxf["char_height"] == pytest.approx(expected["char_height"])
        assert dxf["attachment_point"] == expected["attachment_point"]


# ------------------------------------------------ dimension styles, tolerance


def test_dimstyle_sizes(document: ezdwg.Document) -> None:
    # R14 keeps symbol names in upper case.
    styles = {name.upper(): style for name, style in document.dimstyles().items()}

    assert len(styles) == 6
    assert styles["STANDARD"]["handle"] == 0x27
    for name, dimscale, dimtxt, dimasz in (
        ("STANDARD", 1.0, 0.18, 0.18),
        ("ISO-25", 1.0, 2.5, 2.5),
        ("ANNOTATIVE", 0.0, 0.18, 0.18),
        ("BESCHRIFTUNG", 0.0, 2.5, 2.5),
    ):
        style = styles[name]
        assert style["dimscale"] == pytest.approx(dimscale)
        assert style["dimtxt"] == pytest.approx(dimtxt)
        assert style["dimasz"] == pytest.approx(dimasz)


def test_tolerances_of_every_version(document: ezdwg.Document) -> None:
    tolerances = _entities(document, "TOLERANCE")

    assert set(tolerances) == {0x822, 0x823, 0x824}
    assert tolerances[0x822]["insert"][:2] == pytest.approx((214.1752, -4.3204), abs=1e-3)
    assert tolerances[0x823]["text"].replace("^J", "\n") == "99\na"
    assert tolerances[0x824]["text"] == "{\\Fgdt;p}"
    for dxf in tolerances.values():
        assert dxf["rotation"] == pytest.approx(0.0)
        # No height is stored with the entity: it is the one of the style.
        assert dxf["dimstyle_handle"] == 0x27
        assert dxf["height"] == pytest.approx(0.18)

    stored = {row[0]: row[5] for row in ezdwg.raw.decode_tolerance_entities(document.path)}
    assert stored == {0x822: 0.0, 0x823: 0.0, 0x824: 0.0}


# ------------------------------------------------------------ R14 against R2004

# Types whose R13/R14 layout differs from the one R2000 introduced.
R14_TYPES = (
    "INSERT",
    "MTEXT",
    "HATCH",
    "SOLID",
    "SPLINE",
    "ATTRIB",
    "ATTDEF",
    "DIMENSION",
    "TOLERANCE",
    "MLINE",
    "LEADER",
    "3DFACE",
    "SHAPE",
    "POINT",
    "POLYLINE_3D",
    "POLYLINE_PFACE",
)
# Keys that the two versions cannot share: what R2000 added, handles of objects
# that are not the same in both files, and the alignment point, which R13/R14
# always store.
_VERSION_KEYS = {
    "lineweight",
    "align_point",
    "layer_handle",
    "linetype_handle",
    "style_handle",
    "dimstyle_handle",
    "mlinestyle_handle",
    "shapefile_handle",
    "owner_handle",
    "owner_type",
    "lock_position",
    "common",
    "actual_measurement",
    "attachment_point",
    "char_height",
    "char_height_source",
    "line_spacing_factor",
    "line_spacing_style",
    # R14 writes a paragraph break of MTEXT as "\\P", R2004 as a line feed.
    "raw_text",
    # The background fill of MTEXT came with R2004.
    "background_flags",
    "background_scale_factor",
    "background_color_index",
    "background_true_color",
    "background_transparency",
    "resolved_color_index",
    "resolved_true_color",
    "color_index",
    "true_color",
    # R2004 lists the vertices a polyline owns, but not its SEQEND; in an R14
    # file the SEQEND is found behind the vertices.
    "seqend_handle",
}


def _close(left: Any, right: Any) -> bool:
    if isinstance(left, bool) or isinstance(right, bool):
        return left == right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return math.isclose(left, right, rel_tol=1e-9, abs_tol=1e-9)
    if isinstance(left, (list, tuple)) and isinstance(right, (list, tuple)):
        return len(left) == len(right) and all(_close(a, b) for a, b in zip(left, right))
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(_close(left[k], right[k]) for k in left)
    return left == right


def test_r14_decodes_the_types_of_the_later_versions() -> None:
    r14 = Counter(entity.dxftype for entity in ezdwg.read(sample("AC1014")).entities().query())
    r2004 = Counter(entity.dxftype for entity in ezdwg.read(sample("AC1018")).entities().query())

    for dxftype in R14_TYPES:
        assert r14[dxftype] == r2004[dxftype] > 0, dxftype


@pytest.mark.parametrize("dxftype", R14_TYPES)
def test_r14_entities_match_the_r2004_entities(dxftype: str) -> None:
    r14 = _entities(ezdwg.read(sample("AC1014")), dxftype)
    r2004 = _entities(ezdwg.read(sample("AC1018")), dxftype)

    # The contents of some anonymous blocks have other handles in the R14 file.
    shared = set(r14) & set(r2004)
    assert len(shared) >= 0.6 * len(r2004)
    for handle in sorted(shared):
        old, new = r14[handle], r2004[handle]
        keys = (set(old) & set(new)) - _VERSION_KEYS
        different = sorted(key for key in keys if not _close(old[key], new[key]))
        # R14 keeps symbol names in upper case.
        if "name" in different and old["name"] == new["name"].upper():
            different.remove("name")
        assert not different, (hex(handle), different)


def test_r14_block_references_and_dimensions_name_their_block() -> None:
    doc = ezdwg.read(sample("AC1014"))

    # R14 keeps symbol names in upper case.
    inserts = _entities(doc, "INSERT")
    assert inserts[0x704]["name"] == "MYBLOCK"
    assert inserts[0x783]["name"] == inserts[0x79C]["name"] == "MY_BLOCK_V2"
    assert all(dxf["name"] for dxf in inserts.values())

    dimensions = _entities(doc, "DIMENSION")
    assert len(dimensions) == 11
    blocks = {dxf["anonymous_block_name"] for dxf in dimensions.values()}
    assert len(blocks) == 11 and all(name.startswith("*D") for name in blocks)
    linear = dimensions[0x514]
    assert linear["dimtype"] == "LINEAR"
    assert linear["defpoint"][:2] == pytest.approx((360.36, 44.29), abs=0.01)
    assert linear["text_midpoint"][:2] == pytest.approx((339.07, 34.45), abs=0.01)


def test_r14_entities_are_placed_like_the_r2004_entities() -> None:
    r14 = ezdwg.read(sample("AC1014"))
    r2004 = ezdwg.read(sample("AC1018"))
    r14_handles = {entity.handle for entity in r14.entities().query()}
    shared = [
        entity.handle for entity in r2004.entities().query() if entity.handle in r14_handles
    ]

    assert len(shared) > 250
    placed_differently = [
        hex(handle)
        for handle in shared
        if r14.entity_placement(handle) != r2004.entity_placement(handle)
    ]
    assert not placed_differently
    # Block contents stay in their block: model space holds the same entities.
    shared_handles = set(shared)
    assert Counter(
        e.dxftype for e in r14.modelspace().query() if e.handle in shared_handles
    ) == Counter(e.dxftype for e in r2004.modelspace().query() if e.handle in shared_handles)


def test_r14_3d_polyline_keeps_its_vertices() -> None:
    (polyline,) = _entities(ezdwg.read(sample("AC1014")), "POLYLINE_3D").values()
    (mesh,) = _entities(ezdwg.read(sample("AC1014")), "POLYLINE_PFACE").values()

    assert len(polyline["points"]) == 5
    assert polyline["points"][1] == pytest.approx((233.415, 3.349, 5.479), abs=1e-3)
    assert not polyline["closed"]
    assert (mesh["num_vertices"], mesh["num_faces"]) == (5, 2)
    assert len(mesh["vertices"]) == 5 and len(mesh["faces"]) == 2


# ------------------------------------------------------ layouts and viewports


def test_layouts_of_every_version(document: ezdwg.Document) -> None:
    layouts = document.layouts()

    # In tab order; the names and sheets are those of the DXF export.
    assert list(layouts) == ["Model", "Layout1", "Layout2", "MyLayout"]
    assert [entry["tab_order"] for entry in layouts.values()] == [0, 1, 2, 3]
    assert [entry["model"] for entry in layouts.values()] == [True, False, False, False]
    assert [entry["block_record_handle"] for entry in layouts.values()] == [
        0x1F,
        0x58,
        0x5D,
        0x259,
    ]
    sheet = layouts["MyLayout"]
    assert (sheet["paper_width"], sheet["paper_height"]) == (210.0, 297.0)
    assert sheet["paper_size"] == "A4"
    assert sheet["margins"] == pytest.approx((4.2333, 4.2227, 4.26, 4.2333), abs=1e-3)
    assert (sheet["paper_units"], sheet["plot_rotation"]) == (0, 1)
    assert layouts["Layout1"]["paper_size"] == "Letter_(8.50_x_11.00_Inches)"
    assert layouts["Layout1"]["paper_width"] == pytest.approx(215.9, abs=1e-3)

    # The sheet that was current when the file was saved: its entities are
    # stored without an owner.
    assert [name for name, entry in layouts.items() if entry["active"]] == ["MyLayout"]
    unowned = {
        handle
        for handle in _entities(document, "VIEWPORT")
        if document.entity_placement(handle) == (1, None)
    }
    assert unowned == {0x267, 0x26B}

    # R2004+ list the viewports of a layout, the sheet's own viewport first.
    listed = document.version not in ("AC1014", "AC1015")
    assert sheet["viewport_handles"] == ([0x267, 0x26B] if listed else [])
    assert layouts["Layout1"]["viewport_handles"] == ([0x240, 0x245] if listed else [])
    assert layouts["Model"]["viewport_handles"] == []


def test_viewports_of_every_version(document: ezdwg.Document) -> None:
    viewports = _entities(document, "VIEWPORT")

    assert sorted(viewports) == [0x240, 0x245, 0x252, 0x256, 0x267, 0x26B]
    window = viewports[0x26B]
    assert window["center"] == pytest.approx((5.68, 3.96667, 0.0), abs=1e-4)
    assert window["width"] == pytest.approx(9.088, abs=1e-4)
    assert window["height"] == pytest.approx(6.34667, abs=1e-4)
    assert window["frozen_layers"] == [] and window["frozen_layer_handles"] == []
    assert "clip_boundary_handle" not in window

    if document.version == "AC1014":
        # R13/R14 keep the view in the extended data of the entity.
        assert "view_height" not in window
        return
    # What the window shows: DXF groups 12/22, 45, 16, 17, 51 and 90.
    assert window["view_center"] == pytest.approx((6.0, 4.5))
    assert window["view_height"] == pytest.approx(9.10887, abs=1e-4)
    assert window["view_direction"] == (0.0, 0.0, 1.0)
    assert window["view_target"] == (0.0, 0.0, 0.0)
    assert window["view_twist_angle"] == 0.0
    assert window["status_flags"] == 819808
    # The sheet's own viewport shows the sheet at scale 1.
    sheet = viewports[0x267]
    assert sheet["view_center"] == pytest.approx(sheet["center"][:2])
    assert sheet["view_height"] == pytest.approx(sheet["height"])
    assert sheet["status_flags"] == 819232


@pytest.mark.parametrize("version", VERSIONS)
def test_raw_layout_objects(version: str) -> None:
    rows = ezdwg.raw.decode_layout_objects(sample(version))

    assert sorted(row[1] for row in rows) == ["Layout1", "Layout2", "Model", "MyLayout"]
    by_name = {row[1]: row for row in rows}
    handle, _name, tab_order, flags, block_record, paper, limits, extents = by_name[
        "MyLayout"
    ][:8]
    assert (handle, tab_order, flags, block_record) == (0x25A, 3, 1, 0x259)
    assert paper[:2] == (210.0, 297.0) and paper[3:] == ("A4", 0, 1)
    assert all(math.isfinite(value) for point in (*limits, *extents) for value in point)
    assert by_name["MyLayout"][9] == 0x267  # last active viewport
