from __future__ import annotations

from pathlib import Path
from typing import Iterator


def iter_dxf_entities(path: Path) -> Iterator[dict[str, object]]:
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


def dxf_entities_of_type(path: Path, entity_type: str) -> list[dict[str, object]]:
    return [entity for entity in iter_dxf_entities(path) if entity["type"] == entity_type]


def group_float(entity: dict[str, object], code: str, default: float = 0.0) -> float:
    groups = entity["groups"]
    assert isinstance(groups, list)
    for group_code, raw_value in groups:
        if group_code == code:
            return float(raw_value)
    return default


def dxf_lwpolyline_points(entity: dict[str, object]) -> list[tuple[float, float, float]]:
    groups = entity["groups"]
    assert isinstance(groups, list)

    points: list[tuple[float, float, float]] = []
    pending_x: float | None = None
    for group_code, raw_value in groups:
        if group_code == "10":
            pending_x = float(raw_value)
            continue
        if group_code == "20" and pending_x is not None:
            points.append((pending_x, float(raw_value), 0.0))
            pending_x = None
    return points


def triplet_close(
    actual: tuple[float, float, float],
    expected: tuple[float, float, float],
    eps: float = 1e-9,
) -> bool:
    return (
        abs(actual[0] - expected[0]) < eps
        and abs(actual[1] - expected[1]) < eps
        and abs(actual[2] - expected[2]) < eps
    )
