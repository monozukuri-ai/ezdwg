from __future__ import annotations

from copy import deepcopy

import pytest

import ezdwg
import ezdwg.convert as convert_module
from ezdwg.entity import Entity
from tests.test_write_dwg_attributes import make_styled_drawing


def populated_drawing():
    doc = ezdwg.new()
    doc.layers["作図"] = {"color_index": 2, "linetype": "DASHED", "lineweight": 35}
    doc.layers["Unused"] = {"color_index": 3}
    doc.linetypes["DASHED"] = {"description": "破線", "dashes": [1.0, -0.5]}
    doc.text_styles["日本語"] = {"font": "txt.shx", "width": 0.9}
    doc.header["insunits"] = 4
    msp = doc.modelspace()
    msp.add_line([0, 0], [10, 5], layer="作図", lineweight=50)
    msp.add_point((1, 2, 3))
    msp.add_circle((2, 3), 5)
    msp.add_arc((3, 4), 2, 30, 150)
    msp.add_ray((4, 5), (1, 0, 0))
    msp.add_xline((5, 6), (0, 1, 0))
    msp.add_lwpolyline(
        [[0, 0], [3, 0], [3, 2]], closed=True, elevation=2, bulges=[0.0, 0.5]
    )
    msp.add_text("寸法", (1, 2), height=2, rotation=30, style="日本語")
    msp.add_mtext("注記\\P二行目", (2, 3), char_height=2, rect_width=20, style="日本語")
    return doc


def test_new_nine_types_attributes_and_unused_tables_roundtrip(tmp_path):
    doc = populated_drawing()
    out = tmp_path / "nested" / "new.dwg"
    result = doc.save(out)
    assert result.source_path == ""
    assert result.written_entities == 9
    assert not result.diagnostics
    assert doc.path == str(out)
    loaded = ezdwg.read(str(out))
    entities = {e.dxftype: e for e in loaded.modelspace().query()}
    assert set(entities) == {e.dxftype for e in doc.modelspace()}
    assert entities["LINE"].dxf["end"] == (10, 5, 0)
    assert entities["LINE"].dxf["color_index"] == 256
    assert entities["LINE"].dxf["lineweight"] == 50
    assert entities["ARC"].dxf["start_angle"] == pytest.approx(30)
    assert entities["LWPOLYLINE"].dxf["closed"]
    assert entities["LWPOLYLINE"].dxf["elevation"] == 2
    assert entities["TEXT"].dxf["text"] == "寸法"
    assert entities["MTEXT"].dxf["raw_text"] == "注記\\P二行目"
    assert loaded.layers["作図"].linetype == "DASHED"
    assert loaded.layers["Unused"].color == 3
    assert loaded.insunits == 4
    assert {e.handle for e in loaded.modelspace().query()} == {
        e.handle for e in doc.modelspace()
    }


def test_add_update_delete_live_query_and_save_same_path(tmp_path):
    doc = ezdwg.new()
    msp = doc.modelspace()
    line = msp.add_line((0, 0), (1, 1))
    point = msp.add_point((0, 0))
    out = tmp_path / "drawing.dwg"
    doc.save(out)
    old_read = ezdwg.read(str(out))
    assert len(list(old_read.modelspace().query())) == 2  # populate decode caches
    assert msp.update(line, end=[3, 4]) is line
    line.dxf["start"] = [1, 2]
    msp.delete(point)
    circle = msp.add_circle((0, 0), 2)
    assert circle.handle > point.handle
    assert list(msp.query("l*,circle")) == [line, circle]
    assert msp.get(line.handle) is line
    doc.save()
    loaded = {e.handle: e for e in ezdwg.read(str(out)).modelspace().query()}
    assert set(loaded) == {line.handle, circle.handle}
    assert loaded[line.handle].dxf["start"] == (1, 2, 0)
    assert loaded[line.handle].dxf["end"] == (3, 4, 0)
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))


def test_edit_snapshot_is_independent_and_keeps_attributes(tmp_path):
    source, out = tmp_path / "source.dwg", tmp_path / "edited.dwg"
    make_styled_drawing(source)
    original = source.read_bytes()
    source_doc = ezdwg.read(str(source))
    edit = source_doc.edit()
    msp = edit.modelspace()
    msp.update(10, end=(20, 30, 1))
    msp.update(13, text="変更後")
    msp.update(14, text="編集\\P済み")
    msp.delete(11)
    assert next(source_doc.modelspace().query("LINE")).dxf["end"] != (20, 30, 1)
    assert source.read_bytes() == original
    source.unlink()  # no source decode is permitted when saving the snapshot
    result = edit.save(out)
    assert result.source_path == str(source)
    assert result.written_entities == 4
    loaded = ezdwg.read(str(out))
    rows = {e.handle: e.dxf for e in loaded.modelspace().query()}
    assert rows[10]["end"] == (20, 30, 1)
    assert rows[10]["linetype"] == "DASHED"
    assert rows[10]["thickness"] == 2
    assert rows[13]["text"] == "変更後"
    assert rows[14]["raw_text"] == "編集\\P済み"
    assert rows[14]["line_spacing_factor"] == 1.5
    assert loaded.layers["寸法"].locked
    assert loaded.insunits == 4


def test_edit_save_without_path_replaces_source(tmp_path):
    source = tmp_path / "source.dwg"
    doc = ezdwg.new()
    doc.modelspace().add_circle((0, 0), 2)
    doc.save(source)
    edit = ezdwg.read(str(source)).edit()
    circle = next(edit.modelspace().query())
    circle.dxf["radius"] = 5
    edit.save()
    assert next(ezdwg.read(str(source)).modelspace().query()).dxf["radius"] == 5


def test_import_loss_remains_visible_until_entity_deleted(monkeypatch, tmp_path):
    source = tmp_path / "source.dwg"
    doc = ezdwg.new()
    doc.modelspace().add_line((0, 0), (1, 1))
    doc.save(source)
    source_doc = ezdwg.read(str(source))
    entity = next(source_doc.modelspace().query())
    entity.dxf["true_color"] = 0x112233
    monkeypatch.setattr(
        convert_module,
        "_resolve_dwg_export_entities",
        lambda *args: [entity, Entity("HATCH", 99, {})],
    )
    with pytest.raises(ValueError, match="lossless editable snapshot"):
        source_doc.edit()
    edit = source_doc.edit(strict=False)
    assert {d.attribute for d in edit.diagnostics} >= {"true_color", "geometry"}
    out = tmp_path / "out.dwg"
    out.write_bytes(b"original")
    with pytest.raises(ValueError, match="failed to write"):
        edit.save(out)
    assert out.read_bytes() == b"original"
    result = edit.save(out, strict=False)
    assert result.skipped_by_type == {"HATCH": 1}
    assert any(d.attribute == "true_color" for d in result.diagnostics)
    edit.modelspace().delete(entity.handle)
    edit.modelspace().delete(99)
    assert not edit.diagnostics
    assert edit.save(out).written_entities == 0
    # Explicit reuse creates a new entity, without the deleted import's losses.
    edit.modelspace().add_entity("POINT", {"location": (0, 0)}, handle=entity.handle)
    assert not edit.diagnostics
    assert edit.save(out).written_entities == 1


def test_import_non_modelspace_contents_are_reported(monkeypatch, tmp_path):
    source = tmp_path / "source.dwg"
    ezdwg.new().save(source)
    monkeypatch.setattr(
        convert_module,
        "_resolve_dwg_export_entities",
        lambda layout, _: [Entity("LINE", 12, {})] if layout.name == "ENTITIES" else [],
    )
    with pytest.raises(
        ValueError, match="outside the editable model-space/block subset"
    ):
        ezdwg.read(str(source)).edit()
    edit = ezdwg.read(str(source)).edit(strict=False)
    assert any(d.attribute == "spaces" for d in edit.diagnostics)
    with pytest.raises(ValueError, match="spaces"):
        edit.save(tmp_path / "out.dwg")


def test_add_copies_inputs_and_failed_update_does_not_mutate():
    doc = ezdwg.new()
    points = [[0, 0], [1, 1]]
    poly = doc.modelspace().add_lwpolyline(points)
    points[0][0] = 99
    assert poly.dxf["points"][0] == (0, 0)
    original = deepcopy(poly.dxf)
    with pytest.raises(ValueError, match="width"):
        doc.modelspace().update(poly, widths=[(-1, 1)])
    assert poly.dxf == original
    with pytest.raises(ValueError, match="does not belong"):
        ezdwg.new().modelspace().delete(poly)
    with pytest.raises(ValueError, match="duplicate"):
        doc.modelspace().add_entity("POINT", {"location": (0, 0)}, handle=poly.handle)


@pytest.mark.parametrize("handle", [0, -1, True, 1.5, 2**64])
def test_invalid_handles(handle):
    with pytest.raises(ValueError, match="handle"):
        ezdwg.new().modelspace().add_entity(
            "POINT", {"location": (0, 0)}, handle=handle
        )


@pytest.mark.parametrize(
    "change",
    [
        {"radius": -1},
        {"radius": float("nan")},
        {"layer": "Missing"},
        {"linetype": "Missing"},
        {"color_index": 300},
        {"lineweight": 123},
        {"center": (float("inf"), 0, 0)},
        {"true_color": -1},
    ],
)
def test_invalid_direct_mutation_preserves_output(tmp_path, change):
    doc = ezdwg.new()
    circle = doc.modelspace().add_circle((0, 0), 2)
    out = tmp_path / "out.dwg"
    doc.save(out)
    original = out.read_bytes()
    circle.dxf.update(change)
    with pytest.raises(ValueError):
        doc.save()
    assert out.read_bytes() == original
    assert doc.path == str(out)
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))


def test_unknown_attributes_and_versions_reject():
    with pytest.raises(ValueError, match="version"):
        ezdwg.new("AC1032")
    with pytest.raises(ValueError, match="unsupported"):
        ezdwg.new().modelspace().add_point((0, 0), colour=2)
    with pytest.raises(ValueError, match="output path"):
        ezdwg.new().save()


def test_unsupported_direct_attribute_reports_in_non_strict(tmp_path):
    doc = ezdwg.new()
    doc.modelspace().add_point((0, 0)).dxf["transparency"] = 0.5
    result = doc.save(tmp_path / "out.dwg", strict=False)
    assert result.written_entities == 1
    assert any(d.attribute == "transparency" for d in result.diagnostics)


def test_failed_replacement_keeps_state_and_cleans_temporary(tmp_path):
    doc = ezdwg.new()
    doc.modelspace().add_point((0, 0))
    destination = tmp_path / "directory.dwg"
    destination.mkdir()
    sentinel = destination / "keep"
    sentinel.write_text("original")
    with pytest.raises(OSError, match="regular file"):
        doc.save(destination)
    assert doc.path is None
    assert sentinel.read_text() == "original"
    assert not list(tmp_path.glob(".ezdwg-*.tmp"))


def test_symlink_destination_rejects_without_modifying_target(tmp_path):
    target = tmp_path / "target.dwg"
    target.write_bytes(b"original")
    link = tmp_path / "link.dwg"
    try:
        link.symlink_to(target)
    except OSError:
        pytest.skip("symlinks unavailable")
    with pytest.raises(OSError, match="symlink"):
        ezdwg.new().save(link)
    assert link.is_symlink()
    assert target.read_bytes() == b"original"


def test_to_dwg_accepts_drawing_and_editable_layout(tmp_path):
    doc = populated_drawing()
    result = ezdwg.to_dwg(doc, str(tmp_path / "all.dwg"), strict=True)
    assert result.written_entities == 9
    result = ezdwg.to_dwg(
        doc.modelspace(), str(tmp_path / "lines.dwg"), types="LINE", strict=True
    )
    assert result.written_entities == 1
