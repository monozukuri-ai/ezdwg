from __future__ import annotations

import math
from pathlib import Path

import pytest

import ezdwg
import ezdwg.document as document_module


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._insert_owner_handle_map.cache_clear()


def test_query_attrib_maps_text_and_attribute_fields(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_attrib_entities",
        lambda _path: [
            (
                0x101,
                "VAL",
                "TAG",
                None,
                (1.0, 2.0, 0.0),
                None,
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 2.5, 0.0, 1.0),
                (0, 0, 0),
                2,
                True,
                (None, 0x404),
            )
        ],
    )

    doc = document_module.Document(path="dummy_attrib.dwg", version="AC1015")
    entities = list(doc.modelspace().query("ATTRIB"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert entities[0].dxftype == "ATTRIB"
    assert dxf["text"] == "VAL"
    assert dxf["tag"] == "TAG"
    assert dxf["attribute_flags"] == 2
    assert dxf["lock_position"] is True
    assert dxf["height"] == 2.5
    assert dxf["owner_handle"] == 0x404
    assert dxf["owner_type"] == "INSERT"


def test_query_attdef_includes_prompt(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_attdef_entities",
        lambda _path: [
            (
                0x202,
                "Default",
                "NAME",
                "Enter name",
                (3.0, 4.0, 0.0),
                (3.0, 4.0, 0.0),
                (0.0, 0.0, 1.0),
                (0.0, 0.0, 1.5, 0.0, 1.0),
                (0, 1, 0),
                0,
                False,
                (None, 0x300),
            )
        ],
    )

    doc = document_module.Document(path="dummy_attdef.dwg", version="AC1018")
    entities = list(doc.modelspace().query("ATTDEF"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert entities[0].dxftype == "ATTDEF"
    assert dxf["tag"] == "NAME"
    assert dxf["prompt"] == "Enter name"
    assert dxf["lock_position"] is False
    assert dxf["owner_handle"] == 0x300
    assert dxf["owner_type"] == "BLOCK"


def test_query_minsert_maps_array_parameters(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_minsert_entities",
        lambda _path: [
            (
                0x303,
                10.0,
                20.0,
                0.0,
                2.0,
                3.0,
                1.0,
                math.pi / 2.0,
                (4, 5, 6.5, 7.5, "BLK_ARRAY"),
            )
        ],
    )

    doc = document_module.Document(path="dummy_minsert.dwg", version="AC1021")
    entities = list(doc.modelspace().query("MINSERT"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert entities[0].dxftype == "MINSERT"
    assert dxf["insert"] == (10.0, 20.0, 0.0)
    assert dxf["xscale"] == 2.0
    assert dxf["yscale"] == 3.0
    assert dxf["zscale"] == 1.0
    assert abs(dxf["rotation"] - 90.0) < 1.0e-9
    assert dxf["column_count"] == 4
    assert dxf["row_count"] == 5
    assert dxf["column_spacing"] == 6.5
    assert dxf["row_spacing"] == 7.5
    assert dxf["name"] == "BLK_ARRAY"
    assert entities[0].to_points() == [(10.0, 20.0, 0.0)]


def test_query_insert_maps_transform_parameters(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_entities",
        lambda _path: [
            (
                0x404,
                100.0,
                50.0,
                0.0,
                2.0,
                1.5,
                1.0,
                math.pi / 6.0,
                "BLOCK_A",
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_insert_owner_handles",
        lambda _path: [(0x404, 0x20)],
    )

    doc = document_module.Document(path="dummy_insert.dwg", version="AC1018")
    entities = list(doc.modelspace().query("INSERT"))

    assert len(entities) == 1
    dxf = entities[0].dxf
    assert entities[0].dxftype == "INSERT"
    assert dxf["insert"] == (100.0, 50.0, 0.0)
    assert dxf["xscale"] == 2.0
    assert dxf["yscale"] == 1.5
    assert dxf["zscale"] == 1.0
    assert abs(dxf["rotation"] - 30.0) < 1.0e-9
    assert dxf["name"] == "BLOCK_A"
    assert dxf["owner_handle"] == 0x20
    assert entities[0].to_points() == [(100.0, 50.0, 0.0)]


def test_query_insert_real_fixture_exposes_high_level_contract() -> None:
    doc = ezdwg.read(str(SAMPLES / "insert_2004.dwg"))

    entities = list(doc.modelspace().query("INSERT"))

    assert len(entities) == 1
    entity = entities[0]
    assert entity.dxftype == "INSERT"
    assert entity.handle == 0x2C
    assert entity.dxf["name"] == "BLK1"
    assert entity.dxf["insert"] == (100.0, 50.0, 0.0)
    assert entity.dxf["xscale"] == 2.0
    assert entity.dxf["yscale"] == 1.5
    assert entity.dxf["zscale"] == 1.0
    assert math.isclose(entity.dxf["rotation"], 15.0)
    assert entity.to_points() == [(100.0, 50.0, 0.0)]


def test_query_insert_and_minsert_uses_combined_decoder(monkeypatch) -> None:
    _patch_empty_color_maps(monkeypatch)
    calls = {"combined": 0, "insert": 0, "minsert": 0}

    def _decode_combined(_path: str):
        calls["combined"] += 1
        return (
            [
                (
                    0x501,
                    1.0,
                    2.0,
                    0.0,
                    1.0,
                    1.0,
                    1.0,
                    0.0,
                    "BLOCK_A",
                )
            ],
            [
                (
                    0x502,
                    3.0,
                    4.0,
                    0.0,
                    1.0,
                    1.0,
                    1.0,
                    0.0,
                    (2, 3, 5.0, 6.0, "BLOCK_B"),
                )
            ],
        )

    def _decode_insert(_path: str):
        calls["insert"] += 1
        return []

    def _decode_minsert(_path: str):
        calls["minsert"] += 1
        return []

    monkeypatch.setattr(document_module.raw, "decode_insert_minsert_entities", _decode_combined)
    monkeypatch.setattr(document_module.raw, "decode_insert_entities", _decode_insert)
    monkeypatch.setattr(document_module.raw, "decode_minsert_entities", _decode_minsert)

    doc = document_module.Document(path="dummy_insert_minsert.dwg", version="AC1021")
    entities = list(doc.modelspace().query("INSERT MINSERT"))

    assert len(entities) == 2
    assert entities[0].dxftype == "INSERT"
    assert entities[1].dxftype == "MINSERT"
    assert calls["combined"] == 1
    assert calls["insert"] == 0
    assert calls["minsert"] == 0


def _written_attribute_values(
    tmp_path, version: str, text: str, height: float
) -> dict[str, tuple[str, float]]:
    """Text and height, by tag, of an ATTDEF and an ATTRIB written and read back."""
    doc = ezdwg.new(version)
    block = doc.blocks.new("B1")
    block.add_line((0, 0, 0), (1, 0, 0))
    block.add_attdef("DEF", text, (0, 0, 0), height, prompt="p")
    ref = doc.modelspace().add_blockref("B1", (5, 5, 0))
    doc.modelspace().add_attrib(ref, "TAG", text, (5, 5, 0), height)
    # A new name for each drawing: the reader caches a file by its path, size
    # and modification time, and two can be written in one clock tick.
    path = tmp_path / f"attrib_{len(list(tmp_path.iterdir()))}.dwg"
    doc.save(path)

    return {
        entity.dxf["tag"]: (entity.dxf["text"], entity.dxf["height"])
        for entity in ezdwg.read(str(path)).entities().iter_entities()
        if entity.dxftype in {"ATTRIB", "ATTDEF"}
    }


@pytest.mark.parametrize(
    ("text", "height"),
    [
        ("\u00b0", 2.5),  # a symbol on its own
        ("\ue000B", 2.5),  # a symbol font character of the private use area
        ("A\u0085", 2.5),  # a C1 control character
        ("T1", 1.0e-9),  # a very small text height
    ],
)
def test_exact_attribute_is_kept_when_its_value_looks_like_noise(
    tmp_path, text: str, height: float
) -> None:
    """A record read at its exact layout is not dropped by the candidate check."""
    values = _written_attribute_values(tmp_path, "AC1032", text, height)
    assert values == {"DEF": (text, height), "TAG": (text, height)}


def test_attribute_without_an_exact_layout_still_passes_the_candidate_check(
    tmp_path,
) -> None:
    """R2000 has no exact layout: its reader applies the check as before."""
    values = _written_attribute_values(tmp_path, "AC1015", "T1", 2.5)
    assert values == {"DEF": ("T1", 2.5), "TAG": ("T1", 2.5)}
    # A known limit, not a wanted result: with no exact layout the check
    # still takes a symbol on its own for noise and drops the record.
    assert _written_attribute_values(tmp_path, "AC1015", "\u00b0", 2.5) == {}


def test_r2018_attribute_that_the_exact_layout_refuses_goes_to_the_fallback(tmp_path) -> None:
    """The exact layout takes no tag with a control character.

    The record then goes to the candidate search, which does not find it, and
    it is not returned. No drawing at hand makes that search return noise for
    R2010 or later, so the check behind it is tested on R2000 only.
    """
    doc = ezdwg.new("AC1032")
    block = doc.blocks.new("B1")
    block.add_line((0, 0, 0), (1, 0, 0))
    ref = doc.modelspace().add_blockref("B1", (5, 5, 0))
    doc.modelspace().add_attrib(ref, "TAG", "T1", (5, 5, 0), 2.5)
    doc.modelspace().add_attrib(ref, "BEL\x07", "T2", (5, 9, 0), 2.5)
    path = tmp_path / "attrib.dwg"
    doc.save(path)

    tags = [entity.dxf["tag"] for entity in ezdwg.read(str(path)).entities().query("ATTRIB")]
    assert tags == ["TAG"]
