from __future__ import annotations

import math
from pathlib import Path

import ezdwg
import ezdwg.document as document_module
from ezdwg import raw


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()


def test_query_ray_xline_entities(monkeypatch) -> None:
    _clear_document_caches()

    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [
            (101, 0, 0, 0x28, "RAY", "Entity"),
            (102, 0, 0, 0x29, "XLINE", "Entity"),
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_ray_entities",
        lambda _path: [(101, (10.0, 20.0, 0.0), (1.0, 0.0, 0.0))],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_xline_entities",
        lambda _path: [(102, (30.0, 40.0, 0.0), (0.0, 1.0, 0.0))],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_entity_styles",
        lambda _path: [(101, 1, None, 7), (102, 5, None, 7)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(7, 7, None)],
    )

    doc = document_module.Document(path="dummy_ray_xline.dwg", version="AC1021")
    entities = list(doc.modelspace().query("RAY XLINE"))

    assert [entity.dxftype for entity in entities] == ["RAY", "XLINE"]
    assert entities[0].dxf["start"] == (10.0, 20.0, 0.0)
    assert entities[0].dxf["unit_vector"] == (1.0, 0.0, 0.0)
    assert entities[0].dxf["resolved_color_index"] == 1
    assert entities[1].dxf["start"] == (30.0, 40.0, 0.0)
    assert entities[1].dxf["unit_vector"] == (0.0, 1.0, 0.0)
    assert entities[1].dxf["resolved_color_index"] == 5


def test_ac1032_ray_xline_decode_smoke() -> None:
    sample = Path(__file__).resolve().parents[1] / "test_dwg/acadsharp/sample_AC1032.dwg"
    assert sample.exists(), f"missing sample: {sample}"

    ray_rows = raw.decode_ray_entities(str(sample), limit=256)
    xline_rows = raw.decode_xline_entities(str(sample), limit=256)

    assert len(ray_rows) >= 1
    assert len(xline_rows) >= 1

    for handle, start, unit_vector in [*ray_rows, *xline_rows]:
        assert handle > 0
        for value in [*start, *unit_vector]:
            assert math.isfinite(value)

    doc = ezdwg.read(str(sample))
    assert sum(1 for _ in doc.modelspace().query("RAY")) == len(ray_rows)
    assert sum(1 for _ in doc.modelspace().query("XLINE")) == len(xline_rows)
