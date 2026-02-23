from __future__ import annotations

import math

import ezdwg.document as document_module


def _patch_empty_color_maps(monkeypatch) -> None:
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [])
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


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
