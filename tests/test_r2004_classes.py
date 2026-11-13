"""R2004 classes section: every class of the drawing gets its name."""

from __future__ import annotations

from pathlib import Path

import ezdwg
from ezdwg import raw

ROOT = Path(__file__).resolve().parents[1]
# One of its classes has a maintenance version above 255.
SAMPLE = ROOT / "test_dwg/acadsharp/sample_AC1018.dwg"
# The same drawing, saved as R2000.
NEIGHBOUR = ROOT / "test_dwg/acadsharp/sample_AC1015.dwg"
FIRST_CLASS_NUMBER = 500


def test_r2004_class_based_objects_have_their_class_names() -> None:
    assert SAMPLE.exists(), f"missing sample: {SAMPLE}"
    names_by_code: dict[int, str] = {}
    for _handle, _offset, _size, type_code, type_name, _type_class in raw.list_object_headers_with_type(
        str(SAMPLE)
    ):
        if type_code >= FIRST_CLASS_NUMBER:
            names_by_code[type_code] = type_name

    assert len(names_by_code) == 42
    assert not [name for name in names_by_code.values() if name.startswith("UNKNOWN")]
    # A class in front of the one with the large version, and classes after it.
    assert {"DICTIONARYVAR", "MLEADERSTYLE", "SORTENTSTABLE", "IMAGE"} <= set(
        names_by_code.values()
    )


def _entities(path: Path) -> tuple[dict[str, int], list[dict]]:
    doc = ezdwg.read(str(path))
    counts: dict[str, int] = {}
    for entity in doc.entities().query():
        counts[entity.dxftype] = counts.get(entity.dxftype, 0) + 1
    return counts, [dict(entity.dxf) for entity in doc.entities().query("IMAGE")]


def test_r2004_sample_returns_the_entities_of_the_same_drawing_in_r2000() -> None:
    counts, images = _entities(SAMPLE)
    assert (counts, images) == _entities(NEIGHBOUR)
    # IMAGE is a class-based entity: without the class names it was not returned.
    (image,) = images
    assert image["insert"] == (0, 15, 0)
    assert image["size"] == (994, 965)
    assert image["image_def"] == "image"
