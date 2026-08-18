"""modelspace() / paperspace() / entities() partition entities by their stored
placement (common-header entmode + owner handle)."""

from __future__ import annotations

from collections import Counter
from pathlib import Path

import ezdwg

ROOT = Path(__file__).resolve().parents[1]
SAMPLE = ROOT / "test_dwg" / "acadsharp" / "sample_AC1032.dwg"


def test_modelspace_excludes_block_definitions_and_paper_space() -> None:
    doc = ezdwg.read(str(SAMPLE))
    model = Counter(e.dxftype for e in doc.modelspace().query("*"))
    paper = Counter(e.dxftype for e in doc.paperspace().query("*"))
    everything = Counter(e.dxftype for e in doc.entities().query("*"))

    # Counts of the ACadSharp DXF twin's ENTITIES section (model space part).
    assert model["TEXT"] == 29
    assert model["LINE"] == 26
    assert model["LWPOLYLINE"] == 21
    assert model["DIMENSION"] == 11
    assert model["CIRCLE"] == 8
    assert model["HATCH"] == 8
    # Dimension/arrow blocks contribute the other LINE/CIRCLE/INSERT entities.
    assert everything["LINE"] > model["LINE"]
    assert everything["INSERT"] > model["INSERT"]
    # The six layout viewports live in paper space only.
    assert paper["VIEWPORT"] == 6
    assert model["VIEWPORT"] == 0
    assert sum(model.values()) + sum(paper.values()) <= sum(everything.values())


def test_block_definition_contents_have_block_owner() -> None:
    doc = ezdwg.read(str(SAMPLE))
    header_names = dict(ezdwg.raw.decode_block_header_names(str(SAMPLE)))
    model_handles = {e.handle for e in doc.modelspace().query("*")}
    block_content = 0
    for entity in doc.entities().query("LINE CIRCLE INSERT"):
        placement = doc.entity_placement(entity.handle)
        assert placement is not None
        mode, owner = placement
        if mode == 0 and owner in header_names and not header_names[owner].startswith("*Model_Space"):
            block_content += 1
            assert entity.handle not in model_handles
        elif mode == 2:
            assert entity.handle in model_handles
    assert block_content > 0


def test_unknown_owner_stays_in_modelspace(tmp_path: Path) -> None:
    # Writer-generated minimal files store entmode 0 with a placeholder owner and
    # no block records at all; nothing must disappear from modelspace().
    output = tmp_path / "line.dwg"
    ezdwg.raw.write_ac1015_line_dwg(str(output), [(0x30, 1.0, 2.0, 0.0, 4.5, 7.0, 0.0)])
    doc = ezdwg.read(str(output))
    assert [e.handle for e in doc.modelspace().query("LINE")] == [0x30]
    assert doc.entity_placement(0x30) is not None
