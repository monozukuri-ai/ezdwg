"""The batch writer behind ``to_dwg``: batches, drawing order, misuse."""

from __future__ import annotations

import pytest

import ezdwg
import ezdwg.convert as convert_module
import ezdwg.drawing as drawing_module
from tests.test_write_blocks import block_drawing


def mixed_drawing() -> ezdwg.Drawing:
    """Every writer group across model space, paper space and blocks."""
    drawing, _root, _nested, _other = block_drawing()
    drawing.layers["Fill"] = {"color_index": 1}
    drawing.linetypes["DASHED"] = {"description": "- -", "dashes": [1.0, -0.5]}
    drawing.text_styles["Narrow"] = {"font": "txt.shx", "width": 0.8}
    drawing.image_defs["scan"] = {
        "file_path": "scan.png",
        "size": (200, 100),
        "pixel_size": (0.1, 0.1),
    }
    msp = drawing.modelspace()
    for i in range(7):
        x = float(i)
        msp.add_text(f"T{i}", (x, 0), height=1.5, style="Narrow")
        msp.add_line((x, 0), (x + 1, 1), layer="Fill", linetype="DASHED")
        msp.add_hatch([{"points": [(x, 2), (x + 1, 2), (x + 1, 3)], "closed": True}], layer="Fill")
        msp.add_lwpolyline([(x, 4), (x + 1, 4), (x + 1, 5)], closed=i % 2 == 0, lineweight=35)
        msp.add_circle((x, 6), 0.4, color_index=3)
        msp.add_arc((x, 8), 0.4, 0, 90)
        msp.add_point((x, 10))
        msp.add_ellipse((x, 12), (0.5, 0, 0), 0.5, color_index=4)
        msp.add_solid([(x, 14, 0), (x + 1, 14, 0), (x + 1, 15, 0), (x, 15, 0)])
        msp.add_mtext(f"M{i}\\PLine", (x, 16), char_height=1, rect_width=5)
    msp.add_ray((0, 20), (1, 0, 0))
    msp.add_xline((0, 21), (0, 1, 0))
    msp.add_3dface([(0, 22, 0), (1, 22, 0), (1, 23, 1), (0, 23, 1)])
    msp.add_trace([(0, 24, 0), (1, 24, 0), (1, 25, 0), (0, 25, 0)])
    msp.add_spline([(0, 26, 0), (1, 27, 0), (2, 26, 0), (3, 27, 0)], 3)
    msp.add_polyline2d([(0, 28), (1, 29), (2, 28)])
    msp.add_polyline3d([(0, 30, 0), (1, 31, 1), (2, 30, 2)])
    msp.add_linear_dim((0, 32), (5, 32), (2.5, 34))
    msp.add_image("scan", (0, 36), width=20)
    psp = drawing.paperspace()
    psp.add_line((0, 0), (420, 0))
    psp.add_viewport((210, 148.5), 400, 280, view_center=(5, 15), view_height=40)
    return drawing


def test_small_batches_write_the_same_bytes(tmp_path, monkeypatch):
    drawing = mixed_drawing()
    reference = tmp_path / "one_batch.dwg"
    assert drawing.save(reference).skipped_entities == 0
    monkeypatch.setattr(drawing_module, "WRITE_BATCH", 3)
    batched = tmp_path / "batched.dwg"
    result = drawing.save(batched)
    assert result.skipped_entities == 0
    assert batched.read_bytes() == reference.read_bytes()
    source = ezdwg.read(str(batched))
    kinds = {e.dxftype for e in source.modelspace().query("*")}
    assert {"LINE", "TEXT", "HATCH", "LWPOLYLINE", "INSERT", "DIMENSION", "IMAGE"} <= kinds


def test_batches_bound_the_prepared_copies(monkeypatch):
    drawing = ezdwg.new()
    for i in range(10):
        drawing.modelspace().add_line((i, 0), (i, 1))
    entities, blocks, _owners = drawing._write_selection(list(drawing.modelspace()))
    plan = drawing._write_plan(entities, blocks, batch=4)
    sizes = [len(prepared) for prepared, _attrs in plan.chunks]
    assert sizes == [4, 4, 2]
    assert plan.metadata["codepage"] == 30


def test_drawing_order_is_kept_across_kinds(tmp_path):
    """A text drawn before a solid fill stays below it: the writer keeps the
    drawing order instead of grouping entities by kind."""
    drawing = ezdwg.new()
    msp = drawing.modelspace()
    first = msp.add_text("under", (0, 0), height=1)
    fill = msp.add_hatch([{"points": [(0, 0), (4, 0), (4, 4)], "closed": True}])
    second = msp.add_text("over", (1, 1), height=1)
    path = tmp_path / "order.dwg"
    drawing.save(path)
    chain = _entity_chain(path)
    assert chain.index(first.handle) < chain.index(fill.handle) < chain.index(second.handle)

    # The raw one-shot API keeps its documented grouping by kind.
    grouped = tmp_path / "grouped.dwg"
    ezdwg.raw.write_dwg(
        str(grouped),
        [],
        [],
        [],
        [],
        [(10, "a", (0.0, 0.0, 0.0), 1.0, 0.0), (12, "b", (1.0, 1.0, 0.0), 1.0, 0.0)],
        [],
        points=[(11, 0.5, 0.5, 0.0, 0.0)],
    )
    assert _entity_chain(grouped) == [10, 12, 11]


def _entity_chain(path) -> list[int]:
    """Model-space entity handles in chain (draw) order."""
    model = {e.handle for e in ezdwg.read(str(path)).modelspace().query("*")}
    links = {
        handle: (prev, nxt)
        for handle, prev, nxt in ezdwg.raw.decode_entity_links(str(path))
        if handle in model
    }
    assert links.keys() == model
    (start,) = [h for h, (prev, _nxt) in links.items() if not prev]
    chain = [start]
    while links[chain[-1]][1]:
        chain.append(links[chain[-1]][1])
    return chain


def test_writer_order_matches_separate_batches(tmp_path):
    writer = ezdwg.raw.DwgWriter()
    writer.add(
        lines=[(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0), (3, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0)],
        texts=[(2, "mid", (0.0, 0.0, 0.0), 1.0, 0.0)],
        order=bytes([0, 4, 0]),
    )
    interleaved = tmp_path / "interleaved.dwg"
    writer.write(str(interleaved))

    writer = ezdwg.raw.DwgWriter()
    writer.add(lines=[(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)])
    writer.add(texts=[(2, "mid", (0.0, 0.0, 0.0), 1.0, 0.0)])
    writer.add(lines=[(3, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0)])
    assert writer.entity_count == 3
    separate = tmp_path / "separate.dwg"
    writer.write(str(separate))
    assert separate.read_bytes() == interleaved.read_bytes()

    writer = ezdwg.raw.DwgWriter()
    writer.add(
        lines=[(1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0), (3, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0)],
        texts=[(2, "mid", (0.0, 0.0, 0.0), 1.0, 0.0)],
    )
    grouped = tmp_path / "grouped.dwg"
    writer.write(str(grouped))
    assert grouped.read_bytes() != interleaved.read_bytes()


def test_streaming_and_buffered_writers_produce_the_same_bytes(tmp_path, monkeypatch):
    """With ``max_handle`` the writer encodes each batch on arrival; the file
    is the same as the buffered document's."""
    drawing = mixed_drawing()
    streamed = tmp_path / "streamed.dwg"
    assert drawing.save(streamed).skipped_entities == 0  # to_dwg passes max_handle

    def buffered_writer(version, max_handle=None):
        return real_writer(version)

    real_writer = ezdwg.raw.DwgWriter
    monkeypatch.setattr(convert_module.raw, "DwgWriter", buffered_writer)
    buffered = tmp_path / "buffered.dwg"
    assert drawing.save(buffered).skipped_entities == 0
    assert buffered.read_bytes() == streamed.read_bytes()


def test_streaming_writer_guards_its_handles(tmp_path):
    line = (5, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    writer = ezdwg.raw.DwgWriter(max_handle=4)
    with pytest.raises(ValueError, match="exceeds the declared maximum"):
        writer.add(lines=[line])
    writer = ezdwg.raw.DwgWriter(max_handle=5)
    writer.add(lines=[line])
    with pytest.raises(ValueError, match="duplicate handle"):
        writer.add(lines=[line])
    writer = ezdwg.raw.DwgWriter(max_handle=5)
    writer.add(lines=[line])
    with pytest.raises(ValueError, match="fixed once"):
        writer.set_tables(layers=[{"name": "0", "color_index": 7}])
    # Sparse raw handles work in both modes; fresh handles go above the ceiling.
    sparse = [(0x1000, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0), (0x3000, 1.0, 1.0, 0.0, 2.0, 2.0, 0.0)]
    for max_handle, name in ((0x3000, "stream.dwg"), (None, "buffer.dwg")):
        writer = ezdwg.raw.DwgWriter(max_handle=max_handle)
        writer.add(lines=sparse, polylines2d=[{"handle": 0x2000, "points": [(0, 0), (1, 1), (2, 0)]}])
        writer.write(str(tmp_path / name))
        doc = ezdwg.read(str(tmp_path / name))
        assert {e.handle for e in doc.modelspace().query("LINE")} == {0x1000, 0x3000}
        (poly,) = doc.modelspace().query("POLYLINE_2D")
        assert poly.handle == 0x2000 and all(h > 0x3000 for h in poly.dxf["vertex_handles"])


def test_writer_rejects_misuse(tmp_path):
    with pytest.raises(ValueError, match="version"):
        ezdwg.raw.DwgWriter("AC1027")
    writer = ezdwg.raw.DwgWriter()
    line = (1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    with pytest.raises(ValueError, match="order lists"):
        writer.add(lines=[line], order=bytes([0, 0]))
    with pytest.raises(ValueError, match="not an entity group"):
        writer.add(lines=[line], order=bytes([99]))
    with pytest.raises(ValueError, match="unknown/zero handles"):
        writer.add(lines=[line], entity_attributes={7: {"layer": "0"}})
    with pytest.raises(ValueError, match="undefined owner block"):
        writer.add(lines=[line], entity_owners={1: "Missing"})
    with pytest.raises(ValueError, match="INSERT owner"):
        writer.add(
            attribs=[{"handle": 5, "text": "v", "tag": "T", "insert": (0.0, 0.0, 0.0), "height": 1.0}]
        )
    writer.add(
        attribs=[{"handle": 5, "text": "v", "tag": "T", "insert": (0.0, 0.0, 0.0), "height": 1.0}],
        entity_owners={5: 9},
    )
    with pytest.raises(ValueError, match="owner handle"):
        writer.write(str(tmp_path / "orphan.dwg"))

    writer = ezdwg.raw.DwgWriter()
    writer.add(lines=[line])
    out = tmp_path / "once.dwg"
    writer.write(str(out))
    assert out.exists()
    with pytest.raises(ValueError, match="already written"):
        writer.write(str(out))
    with pytest.raises(ValueError, match="already written"):
        writer.add(lines=[line])


def test_attribs_attach_to_inserts_from_earlier_batches(tmp_path, monkeypatch):
    drawing, _root, _nested, other = block_drawing()
    monkeypatch.setattr(drawing_module, "WRITE_BATCH", 1)
    path = tmp_path / "blocks.dwg"
    result = drawing.save(path)
    assert result.written_entities == 8
    assert not result.diagnostics
    source = ezdwg.read(str(path))
    assert len(list(source.modelspace().query("INSERT"))) == 2
    assert len(ezdwg.raw.decode_attrib_entities(str(path))) == 2
    inserts = {r[0]: r for r in ezdwg.raw.decode_insert_write_properties(str(path))}
    assert inserts[other][3]  # the outer INSERT keeps its ATTRIB handles


def test_strict_failure_writes_nothing(tmp_path, monkeypatch):
    drawing = ezdwg.new()
    circle = drawing.modelspace().add_circle((0, 0), 2)
    drawing.modelspace().add_line((0, 0), (1, 1))
    circle.dxf["radius"] = -1.0
    monkeypatch.setattr(drawing_module, "WRITE_BATCH", 1)
    out = tmp_path / "strict.dwg"
    with pytest.raises(ValueError, match="failed to write"):
        drawing.save(out)
    assert not out.exists()
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))
    result = ezdwg.to_dwg(drawing, str(out), strict=False)
    assert result.skipped_entities == 1
    assert convert_module.read(str(out)).modelspace().query("LINE")
