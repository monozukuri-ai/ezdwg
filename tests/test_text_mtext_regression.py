from __future__ import annotations

from pathlib import Path
from typing import Iterator

import ezdwg
from ezdwg import raw


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def _iter_dxf_entities(path: Path) -> Iterator[dict[str, object]]:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    section_name: str | None = None
    expect_section_name = False
    current_entity: dict[str, object] | None = None

    for i in range(0, len(lines) - 1, 2):
        code = lines[i].strip()
        value = lines[i + 1].strip()

        if code == "0":
            if current_entity is not None and section_name == "ENTITIES":
                yield current_entity
                current_entity = None

            if value == "SECTION":
                expect_section_name = True
                continue

            if value == "ENDSEC":
                section_name = None
                continue

            if section_name == "ENTITIES":
                current_entity = {"type": value, "groups": []}
            continue

        if expect_section_name and code == "2":
            section_name = value
            expect_section_name = False
            continue

        if section_name == "ENTITIES" and current_entity is not None:
            groups = current_entity["groups"]
            assert isinstance(groups, list)
            groups.append((code, value))

    if current_entity is not None and section_name == "ENTITIES":
        yield current_entity


def _dxf_entities_of_type(path: Path, entity_type: str) -> list[dict[str, object]]:
    return [entity for entity in _iter_dxf_entities(path) if entity["type"] == entity_type]


def _group_str(entity: dict[str, object], code: str, default: str = "") -> str:
    groups = entity["groups"]
    assert isinstance(groups, list)
    for group_code, raw_value in groups:
        if group_code == code:
            return raw_value
    return default


def test_text_counts_match_paired_dxf() -> None:
    for stem in ["text_2000", "text_2004"]:
        dxf_count = len(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", "TEXT"))
        rows = raw.decode_text_entities(str(SAMPLES / f"{stem}.dwg"))
        assert dxf_count == 1
        assert len(rows) == dxf_count


def test_mtext_counts_match_paired_dxf() -> None:
    for stem in ["mtext_2000", "mtext_2004"]:
        dxf_count = len(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", "MTEXT"))
        rows = raw.decode_mtext_entities(str(SAMPLES / f"{stem}.dwg"))
        assert dxf_count == 1
        assert len(rows) == dxf_count


def test_text_string_matches_paired_dxf() -> None:
    for stem in ["text_2000", "text_2004"]:
        dxf_text = _group_str(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", "TEXT")[0], "1")
        entities = list(ezdwg.read(str(SAMPLES / f"{stem}.dwg")).modelspace().query("TEXT"))
        assert len(entities) == 1
        assert entities[0].dxf["text"] == dxf_text


def test_mtext_string_matches_paired_dxf() -> None:
    for stem in ["mtext_2000", "mtext_2004"]:
        dxf_text = _group_str(_dxf_entities_of_type(SAMPLES / f"{stem}.dxf", "MTEXT")[0], "1")
        entities = list(ezdwg.read(str(SAMPLES / f"{stem}.dwg")).modelspace().query("MTEXT"))
        assert len(entities) == 1
        # High-level API returns normalized plain text.
        assert dxf_text in entities[0].dxf["text"] or entities[0].dxf["text"] in dxf_text
