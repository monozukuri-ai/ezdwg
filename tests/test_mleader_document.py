"""MULTILEADER entities: the leaders, and the text or block label."""

from __future__ import annotations

import struct
from pathlib import Path

import pytest

import ezdwg
import ezdwg.cli as cli_module
import ezdwg.document as document_module
from ezdwg import raw
from tests._catalog_mocks import mock_object_catalog

ROOT = Path(__file__).resolve().parents[1]
# The same drawing saved in seven versions. From R2000 on the reader gives its
# 15 MULTILEADER; all are in model space on layer "0" and have a text label,
# one leader, one leader line and one point.
SAMPLES = {
    version: ROOT / f"test_dwg/acadsharp/sample_{version}.dwg"
    for version in ("AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032")
}
R14_SAMPLE = ROOT / "test_dwg/acadsharp/sample_AC1014.dwg"
FIRST_HANDLE = 0x7B8
# R2010 added a class version, an attachment direction per leader, and more.
FROM_R2010 = {"AC1024", "AC1027", "AC1032"}
BY_LAYER = 0xC0000000
BY_BLOCK = 0xC1000000
TEXT_WIDTH = 1.4471214188267394
ARROW_SIZE = 0.18
# The dogleg of the first MULTILEADER is two arrow sizes long.
DOGLEG_LENGTH = 2 * ARROW_SIZE
TEXTS = (
    ["MLeader text, hello!", "MULTILEADER TEST"]
    + ["MULTILEADER\\PTEST\\P123"] * 10
    + ["MULTILEADER TEST"] * 3
)


def _mleaders(version: str) -> list:
    path = SAMPLES[version]
    assert path.exists(), f"missing sample: {path}"
    return list(ezdwg.read(str(path)).entities().query("MULTILEADER"))


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_mleader_is_read_with_its_leader_and_text(version: str) -> None:
    doc = ezdwg.read(str(SAMPLES[version]))
    mleaders = list(doc.entities().query("MULTILEADER"))
    assert len(mleaders) == 15
    first = mleaders[0]
    assert first.handle == FIRST_HANDLE
    assert doc.entity_placement(first.handle) == (2, None)
    dxf = first.dxf
    assert dxf["layer_handle"] == 0x10
    assert dxf["class_version"] == (2 if version in FROM_R2010 else None)
    assert dxf["style_handle"] == 0xD8
    assert dxf["content_type"] == 2
    assert dxf["override_flags"] == 279552
    assert dxf["leader_line_type"] == 1
    assert dxf["leader_linetype_handle"] == 0x14
    assert dxf["leader_lineweight"] == -2
    assert (dxf["landing_enabled"], dxf["dogleg_enabled"]) == (True, True)
    assert dxf["dogleg_length"] == pytest.approx(DOGLEG_LENGTH)
    # The default arrowhead: no block.
    assert dxf["arrow_handle"] == 0
    assert dxf["arrow_size"] == pytest.approx(0.18)
    assert dxf["text_style_handle"] == 0x11
    assert (dxf["left_attachment"], dxf["right_attachment"]) == (1, 1)
    assert dxf["text_frame"] is False
    assert (dxf["block_handle"], dxf["block_connection"], dxf["attributes"]) == (0, 0, [])

    context = dxf["context"]
    assert context["scale"] == 1.0
    assert context["content_base"] == pytest.approx((1083.6810451657636, 23.239243872835004, 0.0))
    assert context["text_height"] == pytest.approx(0.18)
    assert context["arrow_size"] == pytest.approx(0.18)
    assert context["landing_gap"] == pytest.approx(0.09)
    assert (context["left_attachment"], context["right_attachment"]) == (1, 1)
    assert (context["text_alignment"], context["attachment_type"]) == (0, 0)
    assert context["plane_origin"] == pytest.approx((1050.2415627732776, 8.17554702864777, 0.0))
    assert context["plane_x_axis"] == (1.0, 0.0, 0.0)
    assert context["plane_y_axis"] == (0.0, 1.0, 0.0)
    assert context["plane_normal_reversed"] is False
    assert context["block"] is None

    (leader,) = context["leaders"]
    assert (leader["has_last_point"], leader["has_dogleg"]) == (True, True)
    assert leader["last_point"] == pytest.approx((1083.3210451657637, 23.239243872835004, 0.0))
    assert leader["dogleg_vector"] == (1.0, 0.0, 0.0)
    assert leader["dogleg_length"] == pytest.approx(DOGLEG_LENGTH)
    assert leader["index"] == 0
    assert leader["attachment_direction"] == (0 if version in FROM_R2010 else None)
    (line,) = leader["lines"]
    assert line["index"] == 0
    assert line["points"] == [pytest.approx((1050.2415627732776, 8.17554702864777, 0.0))]

    text = context["text"]
    assert text["text"] == "MLeader text, hello!"
    assert text["normal"] == (0.0, 0.0, 1.0)
    assert text["style_handle"] == 0x11
    assert text["location"] == pytest.approx((1083.7710451657636, 23.347047419901852, 0.0))
    assert text["direction"] == (1.0, 0.0, 0.0)
    assert text["rotation"] == 0.0
    assert text["width"] == pytest.approx(TEXT_WIDTH)
    assert (text["attachment"], text["drawing_direction"]) == (1, 5)
    assert (text["background_scale"], text["background_on"]) == (0.0, False)
    if version == "AC1015":
        # R2000 stores a color index only.
        assert dxf["leader_color"] == (0, 0)
        assert text["color"] == (256, 0)
        assert text["background_color"] == (256, 0)
    else:
        assert dxf["leader_color"] == (0, BY_BLOCK)
        assert text["color"] == (0, BY_LAYER)
        assert text["background_color"] == (0, BY_LAYER)


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_the_15_mleaders_of_the_sample_differ_where_the_drawing_does(version: str) -> None:
    mleaders = _mleaders(version)
    assert [m.handle for m in mleaders] == [FIRST_HANDLE, *range(0xB1A, 0xB28)]
    assert [m.dxf["context"]["text"]["text"] for m in mleaders] == TEXTS
    assert [m.dxf["context"]["text"]["attachment"] for m in mleaders] == [
        1, 1, 1, 2, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1
    ]  # fmt: skip
    assert [m.dxf["text_frame"] for m in mleaders] == [False] * 5 + [True] * 7 + [False] * 3
    assert [m.dxf["context"]["text"]["background_on"] for m in mleaders] == (
        [False] * 9 + [True] * 3 + [False] * 3
    )
    turned = [m.dxf["context"]["text"]["rotation"] for m in mleaders]
    assert turned == [pytest.approx(0.2617993877991494 if 8 <= i <= 11 else 0.0) for i in range(15)]
    # The last one is attached at its top or bottom, which only R2010 and
    # later can store: the drawing itself differs there in the older versions.
    new = version in FROM_R2010
    assert [m.dxf["left_attachment"] for m in mleaders][-3:] == [5, 0, 0 if new else 4]
    assert [m.dxf["right_attachment"] for m in mleaders][-3:] == [1, 1, 8 if new else 0]
    directions = [m.dxf["context"]["leaders"][0]["attachment_direction"] for m in mleaders]
    assert directions == ([0] * 14 + [1] if version in FROM_R2010 else [None] * 15)
    # Each has its own arrow point.
    points = {m.dxf["context"]["leaders"][0]["lines"][0]["points"][0] for m in mleaders}
    assert len(points) == 15


def _comparable(dxf: dict, *, colors: bool) -> dict:
    """The row without what a later version adds, and without the colors when asked."""
    out = {key: value for key, value in dxf.items() if key != "class_version"}
    context = dict(out["context"])
    context["leaders"] = [
        {key: value for key, value in leader.items() if key != "attachment_direction"}
        for leader in context["leaders"]
    ]
    if not colors:
        out.pop("leader_color")
        context["text"] = {
            key: value
            for key, value in context["text"].items()
            if key not in {"color", "background_color"}
        }
    out["context"] = context
    return out


def test_the_versions_give_the_same_rows() -> None:
    rows = {version: [m.dxf for m in _mleaders(version)] for version in SAMPLES}
    newest = rows["AC1032"]
    for version in ("AC1024", "AC1027"):
        assert rows[version] == newest, version
    # R2004 has the layout of R2007 with its strings in another place.
    assert rows["AC1018"] == rows["AC1021"]
    # R2000 has no color values.
    assert [_comparable(row, colors=False) for row in rows["AC1015"]] == [
        _comparable(row, colors=False) for row in rows["AC1021"]
    ]
    # R2007 lacks what R2010 added. The last MULTILEADER differs in the
    # drawing itself (see the test above), the other 14 do not.
    assert [_comparable(row, colors=True) for row in rows["AC1021"][:14]] == [
        _comparable(row, colors=True) for row in newest[:14]
    ]


def test_raw_mleader_rows() -> None:
    rows = raw.decode_mleader_entities(str(SAMPLES["AC1032"]))
    assert len(rows) == 15
    handle, class_version, context, properties, leader_line, text_properties, block, attributes = rows[0]
    assert (handle, class_version) == (FIRST_HANDLE, 2)
    leaders, sizes, attachments, text, block_label, plane = context
    assert len(leaders) == 1
    assert leaders[0][6] == [(0, [pytest.approx((1050.2415627732776, 8.17554702864777, 0.0))])]
    assert sizes[1] == pytest.approx((1083.6810451657636, 23.239243872835004, 0.0))
    assert (sizes[0], *sizes[2:]) == pytest.approx((1.0, 0.18, 0.18, 0.09))
    assert attachments == (1, 1, 0, 0)
    assert text[0] == "MLeader text, hello!"
    assert block_label is None
    assert plane[3] is False
    assert properties == (0xD8, 279552, 2)
    assert leader_line[:6] == (1, (0, BY_BLOCK), 0x14, -2, True, True)
    assert leader_line[6:] == pytest.approx((DOGLEG_LENGTH, 0, ARROW_SIZE))
    assert text_properties == (0x11, 1, 1, False)
    assert (block, attributes) == ((0, 0), [])

    assert [row[0] for row in raw.decode_mleader_entities(str(SAMPLES["AC1032"]), limit=2)] == [
        FIRST_HANDLE,
        0xB1A,
    ]


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_style_rows_hold_the_mleaders(version: str) -> None:
    rows = {row[0]: row[1:] for row in raw.decode_entity_styles(str(SAMPLES[version]))}
    # Color by layer, no true color, layer "0".
    assert [rows.get(handle) for handle in (FIRST_HANDLE, *range(0xB1A, 0xB28))] == [
        (256, None, 0x10)
    ] * 15


def test_r14_mleaders_are_not_read() -> None:
    assert R14_SAMPLE.exists(), f"missing sample: {R14_SAMPLE}"
    # The drawing holds the 15 objects, as its object list shows.
    names = [row[4] for row in raw.list_object_headers_with_type(str(R14_SAMPLE))]
    assert names.count("MULTILEADER") == 15
    assert raw.decode_mleader_entities(str(R14_SAMPLE)) == []
    assert list(ezdwg.read(str(R14_SAMPLE)).entities().query("MULTILEADER")) == []


def test_drawing_without_a_mleader_gives_no_rows(tmp_path) -> None:
    doc = ezdwg.new("AC1032")
    doc.modelspace().add_line((0, 0, 0), (1, 0, 0))
    path = tmp_path / "plain.dwg"
    doc.save(path)

    assert raw.decode_mleader_entities(str(path)) == []
    assert list(ezdwg.read(str(path)).entities().query("MULTILEADER")) == []


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_mleader_rows_come_after_the_leader_rows(version: str) -> None:
    doc = ezdwg.read(str(SAMPLES[version]))
    types = list(dict.fromkeys(entity.dxftype for entity in doc.entities().query()))
    at = types.index("MULTILEADER")
    assert types[at - 1 : at + 2] == ["LEADER", "MULTILEADER", "HATCH"]
    by_type: dict[str, int] = {}
    for entity in doc.modelspace().iter_entities():
        by_type[entity.dxftype] = by_type.get(entity.dxftype, 0) + 1
    assert by_type["MULTILEADER"] == 15


def _with_raw_double(data: bytes, old: float, new: float) -> bytes:
    """`data` with its one raw double `old` replaced, at whatever bit it begins."""
    old_bytes, new_bytes = struct.pack("<d", old), struct.pack("<d", new)
    total_bits = len(data) * 8
    stream = int.from_bytes(data, "big")
    found = []
    for shift in range(8):
        shifted = ((stream << shift) & ((1 << total_bits) - 1)).to_bytes(len(data), "big")
        at = shifted.find(old_bytes)
        while at >= 0:
            found.append(at * 8 + shift)
            at = shifted.find(old_bytes, at + 1)
    (bit,) = found
    low = total_bits - bit - 64
    kept = stream & ~(((1 << 64) - 1) << low)
    return (kept | (int.from_bytes(new_bytes, "big") << low)).to_bytes(len(data), "big")


@pytest.mark.parametrize("bad", [float("nan"), float("inf"), float("-inf")])
def test_mleader_with_a_number_that_is_not_finite_is_dropped(tmp_path, bad: float) -> None:
    # R2000 stores its objects as plain bits: the text width of the first
    # MULTILEADER is written over in a copy of the file.
    source = SAMPLES["AC1015"].read_bytes()

    moved = tmp_path / "moved.dwg"
    moved.write_bytes(_with_raw_double(source, TEXT_WIDTH, 0.375))
    rows = raw.decode_mleader_entities(str(moved))
    assert len(rows) == 15
    assert (rows[0][0], rows[0][2][3][6]) == (FIRST_HANDLE, 0.375)

    broken = tmp_path / "broken.dwg"
    broken.write_bytes(_with_raw_double(source, TEXT_WIDTH, bad))
    handles = [row[0] for row in raw.decode_mleader_entities(str(broken))]
    assert handles == list(range(0xB1A, 0xB28))
    assert len(list(ezdwg.read(str(broken)).entities().query("MULTILEADER"))) == 14
    # The dropped record does not use up the limit.
    assert [row[0] for row in raw.decode_mleader_entities(str(broken), limit=1)] == [0xB1A]


def test_to_dxf_counts_a_mleader_as_skipped_and_strict_raises(tmp_path) -> None:
    pytest.importorskip("ezdxf")
    doc = ezdwg.read(str(SAMPLES["AC1032"]))
    output = str(tmp_path / "out.dxf")

    # The default export selects the types it can write: no MULTILEADER.
    assert "MULTILEADER" not in ezdwg.to_dxf(doc, output).skipped_by_type

    result = ezdwg.to_dxf(doc, output, types="MULTILEADER")
    assert (result.total_entities, result.written_entities) == (15, 0)
    assert result.skipped_by_type == {"MULTILEADER": 15}
    skipped = ezdwg.to_dxf(doc, output, include_unsupported=True).skipped_by_type
    assert skipped["MULTILEADER"] == 15

    with pytest.raises(ValueError, match="MULTILEADER:15"):
        ezdwg.to_dxf(doc, output, types="MULTILEADER", strict=True)
    with pytest.raises(ValueError, match="MULTILEADER:15"):
        ezdwg.to_dxf(doc, output, include_unsupported=True, strict=True)


def test_to_dxf_leaves_a_mleader_out_of_the_block_it_writes(monkeypatch, tmp_path) -> None:
    ezdxf = pytest.importorskip("ezdxf")
    patched_raw = document_module.raw
    monkeypatch.setattr(patched_raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(patched_raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    # A block with a MULTILEADER and a line, and one insert of it.
    mock_object_catalog(
        monkeypatch,
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 500, "MULTILEADER", "Entity"),
            (111, 12, 0, 0x13, "LINE", "Entity"),
            (101, 13, 0, 0x05, "ENDBLK", "Entity"),
            (200, 14, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        patched_raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "WITH_MLEADER"), (101, "ENDBLK", "WITH_MLEADER")],
    )
    monkeypatch.setattr(
        patched_raw,
        "decode_insert_entities",
        lambda _path: [(200, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, "WITH_MLEADER")],
    )
    line = (111, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0)
    monkeypatch.setattr(patched_raw, "decode_line_entities", lambda _path: [line])
    monkeypatch.setattr(
        patched_raw, "decode_line_arc_circle_entities", lambda _path: ([line], [], [])
    )
    # A row of the sample, under the handle of the MULTILEADER in the block.
    row = (110, *raw.decode_mleader_entities(str(SAMPLES["AC1032"]))[0][1:])
    monkeypatch.setattr(patched_raw, "decode_mleader_entities", lambda _path, limit=None: [row])

    output = tmp_path / "block_out.dxf"
    doc = document_module.Document(path="dummy_mleader_block.dwg", version="AC1032")
    (mleader,) = doc.entities().query("MULTILEADER")
    assert mleader.dxf["context"]["text"]["text"] == "MLeader text, hello!"

    # The block export does not count the MULTILEADER it leaves out, and
    # strict does not raise for it.
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010", strict=True)
    assert (result.total_entities, result.written_entities) == (1, 1)
    assert result.skipped_by_type == {}
    block = ezdxf.readfile(str(output)).blocks.get("WITH_MLEADER")
    assert [entity.dxftype() for entity in block] == ["LINE"]


def test_cli_inspect_counts_the_mleaders(capsys) -> None:
    assert cli_module.main(["inspect", str(SAMPLES["AC1032"])]) == 0
    lines = capsys.readouterr().out.splitlines()
    assert "MULTILEADER: 15" in lines
    assert not [line for line in lines if line.startswith("raw_only[MULTILEADER]")]
    assert not [line for line in lines if line.startswith("decode_gap[MULTILEADER]")]


def test_cli_inspect_shows_a_mleader_that_is_not_read(capsys) -> None:
    assert cli_module.main(["inspect", str(R14_SAMPLE)]) == 0
    lines = capsys.readouterr().out.splitlines()
    assert "decode_gap[MULTILEADER]: 15" in lines
    assert not [line for line in lines if line.startswith("MULTILEADER:")]
