"""WIPEOUT entities: the mask with its placement frame and clip boundary."""

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
# The same drawing saved as R2000, R2004, R2007, R2010, R2013 and R2018. It
# holds one WIPEOUT, in model space on layer "0".
SAMPLES = {
    version: ROOT / f"test_dwg/acadsharp/sample_{version}.dwg"
    for version in ("AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032")
}
WIPEOUT_HANDLE = 0x44D
FRAME = 31.707742926072683
HALF = 0.5
BOUNDARY = [
    (-HALF, HALF),
    (0.3260511008800897, 0.49206350233605894),
    (0.2505944934505404, -0.21031745948902658),
    (-0.47617156637515023, -HALF),
]
# The corners of the mask in world coordinates, in the order of BOUNDARY:
# where a DXF library places the boundary that an independent DWG to DXF
# converter, used as a program only, writes for the sample.
WORLD_CORNERS = [
    (271.9211288808192, 3.988123351850959),
    (298.1133448313244, 4.239771779512582),
    (295.72078612087466, 26.51068675323006),
    (272.6766747285269, 35.695866277923635),
]


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_wipeout_is_read_with_its_frame_and_boundary(version: str) -> None:
    path = SAMPLES[version]
    assert path.exists(), f"missing sample: {path}"
    doc = ezdwg.read(str(path))

    (wipeout,) = doc.entities().query("WIPEOUT")
    assert wipeout.handle == WIPEOUT_HANDLE
    dxf = wipeout.dxf
    assert dxf["insert"] == pytest.approx((271.9211288808192, 3.988123351850959, 0.0))
    assert dxf["u_vector"] == pytest.approx((FRAME, 0.0, 0.0))
    assert dxf["v_vector"] == pytest.approx((0.0, FRAME, 0.0))
    assert dxf["size"] == (1.0, 1.0)
    assert dxf["display_props"] == 7
    assert dxf["clipping"] is True
    assert (dxf["brightness"], dxf["contrast"], dxf["fade"]) == (50, 50, 0)
    assert dxf["clip_type"] == 2
    # The clip mode came with R2010.
    assert dxf["clip_mode"] == (None if version in {"AC1015", "AC1018", "AC1021"} else 0)
    assert dxf["clip_vertices"] == BOUNDARY
    assert dxf["layer_handle"] == 0x10
    assert doc.entity_placement(wipeout.handle) == (2, None)


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_clip_vertex_maps_to_its_world_point(version: str) -> None:
    (wipeout,) = ezdwg.read(str(SAMPLES[version])).entities().query("WIPEOUT")
    dxf = wipeout.dxf
    (ix, iy, _), (ux, uy, _), (vx, vy, _) = dxf["insert"], dxf["u_vector"], dxf["v_vector"]
    rows = dxf["size"][1]
    # Columns count from the left of the frame, rows from its top.
    corners = [
        (
            ix + ux * (x + 0.5) + vx * (rows - 0.5 - y),
            iy + uy * (x + 0.5) + vy * (rows - 0.5 - y),
        )
        for x, y in dxf["clip_vertices"]
    ]
    assert corners == [pytest.approx(corner) for corner in WORLD_CORNERS]


@pytest.mark.parametrize("version", sorted(SAMPLES))
def test_wipeout_is_one_of_the_entities_of_a_layout(version: str) -> None:
    doc = ezdwg.read(str(SAMPLES[version]))
    by_type: dict[str, int] = {}
    for entity in doc.modelspace().iter_entities():
        by_type[entity.dxftype] = by_type.get(entity.dxftype, 0) + 1
    assert by_type["WIPEOUT"] == 1
    # The record is the one of IMAGE, but the two types stay apart.
    assert WIPEOUT_HANDLE not in [e.handle for e in doc.entities().query("IMAGE")]


def test_raw_wipeout_rows() -> None:
    rows = raw.decode_wipeout_entities(str(SAMPLES["AC1032"]))
    assert len(rows) == 1
    (
        handle,
        insert,
        u_vector,
        v_vector,
        size,
        display_props,
        clipping,
        levels,
        clip_type,
        clip_vertices,
        clip_mode,
    ) = rows[0]
    # The fields of the IMAGE row up to the clip vertices, then the clip mode.
    (image,) = raw.decode_image_entities(str(SAMPLES["AC1032"]))
    assert len(rows[0]) == len(image) - 1
    assert [type(value) for value in rows[0][:10]] == [type(value) for value in image[:10]]
    assert handle == WIPEOUT_HANDLE
    assert insert == pytest.approx((271.9211288808192, 3.988123351850959, 0.0))
    assert u_vector == pytest.approx((FRAME, 0.0, 0.0))
    assert v_vector == pytest.approx((0.0, FRAME, 0.0))
    assert size == (1.0, 1.0)
    assert (display_props, clipping, levels) == (7, True, (50, 50, 0))
    assert (clip_type, clip_mode) == (2, 0)
    assert clip_vertices == BOUNDARY
    assert raw.decode_wipeout_entities(str(SAMPLES["AC1032"]), limit=0) == []


def test_drawing_without_a_wipeout_gives_no_rows(tmp_path) -> None:
    doc = ezdwg.new("AC1032")
    doc.modelspace().add_line((0, 0, 0), (1, 0, 0))
    path = tmp_path / "plain.dwg"
    doc.save(path)

    assert raw.decode_wipeout_entities(str(path)) == []
    assert list(ezdwg.read(str(path)).entities().query("WIPEOUT")) == []


def test_wipeout_rows_come_after_the_image_rows() -> None:
    doc = ezdwg.read(str(SAMPLES["AC1032"]))
    types = list(dict.fromkeys(entity.dxftype for entity in doc.entities().query()))
    at = types.index("WIPEOUT")
    assert types[at - 1 : at + 2] == ["IMAGE", "WIPEOUT", "TOLERANCE"]


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
def test_wipeout_with_a_number_that_is_not_finite_is_dropped(tmp_path, bad: float) -> None:
    # R2000 stores its objects as plain bits: one vertex of the sample's
    # boundary is written over in a copy of the file.
    source = SAMPLES["AC1015"].read_bytes()
    vertex_x = BOUNDARY[1][0]

    moved = tmp_path / "moved.dwg"
    moved.write_bytes(_with_raw_double(source, vertex_x, 0.125))
    (row,) = raw.decode_wipeout_entities(str(moved))
    assert row[9][1] == (0.125, BOUNDARY[1][1])

    broken = tmp_path / "broken.dwg"
    broken.write_bytes(_with_raw_double(source, vertex_x, bad))
    assert raw.decode_wipeout_entities(str(broken)) == []
    assert list(ezdwg.read(str(broken)).entities().query("WIPEOUT")) == []


def _with_the_image_typed_as_a_wipeout(data: bytes) -> tuple[bytes, int]:
    """The R2000 sample with its IMAGE record typed as a WIPEOUT, and the handle of that record."""
    rows = raw.list_object_headers_with_type(str(SAMPLES["AC1015"]))
    ((handle, offset, _size, image_class),) = [row[:4] for row in rows if row[4] == "IMAGE"]
    (wipeout_class,) = {row[3] for row in rows if row[4] == "WIPEOUT"}
    # A record begins with its size (16 bits) and its type: for a class, two
    # zero bits and the class number as a raw short.
    low = len(data) * 8 - (offset * 8 + 16) - 18
    stream = int.from_bytes(data, "big")
    image_bits = int.from_bytes(struct.pack("<H", image_class), "big")
    wipeout_bits = int.from_bytes(struct.pack("<H", wipeout_class), "big")
    assert (stream >> low) & 0x3FFFF == image_bits
    stream ^= (image_bits ^ wipeout_bits) << low
    return stream.to_bytes(len(data), "big"), handle


def test_limit_counts_the_wipeouts_that_are_returned(tmp_path) -> None:
    # No public drawing has two WIPEOUT. The record of a WIPEOUT is the one
    # of an IMAGE, so a copy of the R2000 sample whose IMAGE record is typed
    # as a WIPEOUT has two: the true one first.
    two, second = _with_the_image_typed_as_a_wipeout(SAMPLES["AC1015"].read_bytes())
    path = tmp_path / "two.dwg"
    path.write_bytes(two)

    def handles(file, limit=None) -> list[int]:
        return [row[0] for row in raw.decode_wipeout_entities(str(file), limit=limit)]

    assert handles(path) == [WIPEOUT_HANDLE, second]
    assert handles(path, limit=1) == [WIPEOUT_HANDLE]

    # The first one gets a number that is not finite and is dropped. It does
    # not use up the limit.
    broken = tmp_path / "broken.dwg"
    broken.write_bytes(_with_raw_double(two, BOUNDARY[1][0], float("nan")))
    assert handles(broken) == [second]
    assert handles(broken, limit=1) == [second]


def test_to_dxf_counts_a_wipeout_as_skipped_and_strict_raises(tmp_path) -> None:
    pytest.importorskip("ezdxf")
    doc = ezdwg.read(str(SAMPLES["AC1032"]))
    output = str(tmp_path / "out.dxf")

    # The default export selects the types it can write: no WIPEOUT.
    assert "WIPEOUT" not in ezdwg.to_dxf(doc, output).skipped_by_type

    result = ezdwg.to_dxf(doc, output, types="WIPEOUT")
    assert (result.total_entities, result.written_entities) == (1, 0)
    assert result.skipped_by_type == {"WIPEOUT": 1}
    assert ezdwg.to_dxf(doc, output, include_unsupported=True).skipped_by_type["WIPEOUT"] == 1

    with pytest.raises(ValueError, match="WIPEOUT:1"):
        ezdwg.to_dxf(doc, output, types="WIPEOUT", strict=True)
    with pytest.raises(ValueError, match="WIPEOUT:1"):
        ezdwg.to_dxf(doc, output, include_unsupported=True, strict=True)


def test_to_dxf_leaves_a_wipeout_out_of_the_block_it_writes(monkeypatch, tmp_path) -> None:
    ezdxf = pytest.importorskip("ezdxf")
    patched_raw = document_module.raw
    monkeypatch.setattr(patched_raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(patched_raw, "decode_layer_colors", lambda _path: [])
    document_module._present_supported_types.cache_clear()
    document_module._entity_handles_by_type_name.cache_clear()
    document_module._block_and_endblk_name_maps.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()

    # A block with a mask and a line, and one insert of it.
    mock_object_catalog(
        monkeypatch,
        lambda _path: [
            (100, 10, 0, 0x04, "BLOCK", "Entity"),
            (110, 11, 0, 500, "WIPEOUT", "Entity"),
            (111, 12, 0, 0x13, "LINE", "Entity"),
            (101, 13, 0, 0x05, "ENDBLK", "Entity"),
            (200, 14, 0, 0x07, "INSERT", "Entity"),
        ],
    )
    monkeypatch.setattr(
        patched_raw,
        "decode_block_entity_names",
        lambda _path: [(100, "BLOCK", "MASKED"), (101, "ENDBLK", "MASKED")],
    )
    monkeypatch.setattr(
        patched_raw,
        "decode_insert_entities",
        lambda _path: [(200, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, "MASKED")],
    )
    line = (111, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0)
    monkeypatch.setattr(patched_raw, "decode_line_entities", lambda _path: [line])
    monkeypatch.setattr(
        patched_raw, "decode_line_arc_circle_entities", lambda _path: ([line], [], [])
    )
    mask = (
        110,
        (0.0, 0.0, 0.0),
        (4.0, 0.0, 0.0),
        (0.0, 4.0, 0.0),
        (1.0, 1.0),
        7,
        True,
        (50, 50, 0),
        1,
        [(-HALF, -HALF), (HALF, HALF)],
        0,
    )
    monkeypatch.setattr(patched_raw, "decode_wipeout_entities", lambda _path, limit=None: [mask])

    output = tmp_path / "block_out.dxf"
    doc = document_module.Document(path="dummy_wipeout_block.dwg", version="AC1032")
    (wipeout,) = doc.entities().query("WIPEOUT")
    assert wipeout.dxf["clip_type"] == 1
    assert wipeout.dxf["clip_vertices"] == [(-HALF, -HALF), (HALF, HALF)]

    # The block export does not count the mask it leaves out, and strict does
    # not raise for it.
    result = ezdwg.to_dxf(doc, str(output), types="INSERT", dxf_version="R2010", strict=True)
    assert (result.total_entities, result.written_entities) == (1, 1)
    assert result.skipped_by_type == {}
    block = ezdxf.readfile(str(output)).blocks.get("MASKED")
    assert [entity.dxftype() for entity in block] == ["LINE"]


def test_cli_inspect_counts_the_wipeout(capsys) -> None:
    assert cli_module.main(["inspect", str(SAMPLES["AC1032"])]) == 0
    lines = capsys.readouterr().out.splitlines()
    assert "WIPEOUT: 1" in lines
    assert not [line for line in lines if line.startswith("raw_only[WIPEOUT]")]
