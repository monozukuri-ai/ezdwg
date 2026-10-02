from __future__ import annotations

import math
from collections import Counter
from pathlib import Path

import pytest

import ezdwg


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"
EXAMPLES = ROOT / "examples" / "data"
ACADSHARP_SAMPLES = [
    SAMPLES / "acadsharp" / f"sample_{version}.dwg"
    for version in ("AC1018", "AC1021", "AC1027", "AC1032")
]
# One LINE on layer "0" saved by the same AutoCAD in every file format version.
LINE_SAMPLES = [
    SAMPLES / "line_R14.dwg",
    EXAMPLES / "line_2000.dwg",
    SAMPLES / "line_2004.dwg",
    SAMPLES / "line_2007.dwg",
    SAMPLES / "line_2010.dwg",
    SAMPLES / "line_2013.dwg",
]


@pytest.mark.parametrize("path", ACADSHARP_SAMPLES, ids=lambda path: path.stem)
def test_raw_decode_linetypes_reads_names_descriptions_and_dashes(path: Path) -> None:
    rows = ezdwg.raw.decode_linetypes(str(path))
    by_name = {name: (description, length, dashes) for _handle, name, description, length, dashes in rows}

    assert list(by_name)[:3] == ["ByBlock", "ByLayer", "Continuous"]
    assert by_name["Continuous"] == ("Solid line", 0.0, [])
    description, length, dashes = by_name["ACAD_ISO02W100"]
    assert description.startswith("ISO dash")
    assert length == pytest.approx(15.0)
    # DXF convention: positive = dash, negative = gap.
    assert dashes == pytest.approx([12.0, -3.0])
    assert {"GAS_LINE", "TRACKS", "ZIGZAG", "BATTING"} <= set(by_name)
    # Every record has its own handle.
    assert len({handle for handle, *_ in rows}) == len(rows)


@pytest.mark.parametrize("path", LINE_SAMPLES, ids=lambda path: path.stem)
def test_linetype_and_layer_tables_are_read_in_every_version(path: Path) -> None:
    linetypes = ezdwg.raw.decode_linetypes(str(path))
    assert [name.upper() for _handle, name, *_ in linetypes] == [
        "BYBLOCK",
        "BYLAYER",
        "CONTINUOUS",
    ]
    continuous = next(handle for handle, name, *_ in linetypes if name.upper() == "CONTINUOUS")

    # The only layer is "0" and it uses the continuous linetype.
    ((layer_handle, layer_name),) = ezdwg.raw.decode_layer_names(str(path))
    assert layer_name == "0"
    assert ezdwg.raw.decode_layer_linetypes(str(path)) == [(layer_handle, continuous)]

    # The LINE refers to that layer and takes its linetype from it.
    doc = ezdwg.read(str(path))
    (line,) = doc.modelspace().query("LINE")
    assert line.dxf["layer_handle"] == layer_handle
    assert line.dxf["linetype"] == "BYLAYER"
    assert line.dxf["linetype_handle"] is None
    assert line.dxf["linetype_scale"] == 1.0
    rows = {row[0]: row for row in ezdwg.raw.decode_entity_linetypes(str(path))}
    assert rows[line.handle] == (line.handle, layer_handle, 0, None, 1.0)


def test_r2000_layer_table_has_names_and_colors() -> None:
    # R2000 has no "XDic Missing Flag" in front of the entry name.
    path = str(EXAMPLES / "line_2000.dwg")

    ((handle, name),) = ezdwg.raw.decode_layer_names(path)
    assert name == "0"
    assert ezdwg.raw.decode_layer_colors(path) == [(handle, 7, None)]
    block_names = {name for _handle, name in ezdwg.raw.decode_block_header_names(path)}
    assert {"*Model_Space", "*Paper_Space"} <= block_names


def test_r2007_layer_names_come_from_the_string_stream() -> None:
    names = [name for _handle, name in ezdwg.raw.decode_layer_names(str(ACADSHARP_SAMPLES[1]))]

    assert names[0] == "0"
    assert {"Layer1", "Layer_Off", "Layer_Freeze", "Layer_Lock", "Layer_NoPlot"} <= set(names)


@pytest.mark.parametrize("path", ACADSHARP_SAMPLES, ids=lambda path: path.stem)
def test_entity_linetypes_resolve_to_table_names(path: Path) -> None:
    doc = ezdwg.read(str(path))
    entities = list(doc.entities().query())
    linetypes = doc.linetypes()

    counts = Counter(entity.dxf.get("linetype") for entity in entities)
    assert counts["BYLAYER"] > 200
    assert counts["BYBLOCK"] > 0
    assert counts["ACAD_ISO02W100"] == 8
    assert counts[None] == 0
    for entity in entities:
        name = entity.dxf["linetype"]
        if name in ("BYLAYER", "BYBLOCK", "CONTINUOUS"):
            continue
        # A named linetype is an entry of the linetype table.
        assert linetypes[name]["handle"] == entity.dxf["linetype_handle"]
        assert entity.dxf["linetype_scale"] > 0


def test_document_linetypes_and_layers() -> None:
    doc = ezdwg.read(str(EXAMPLES / "mechanical_example-imperial.dwg"))

    linetypes = doc.linetypes()
    assert list(linetypes) == ["ByBlock", "ByLayer", "Continuous", "CENTER"]
    center = linetypes["CENTER"]
    assert center["dashes"] == pytest.approx([1.25, -0.25, 0.25, -0.25])
    assert center["pattern_length"] == pytest.approx(2.0)
    assert center["description"].startswith("Center")

    layers = doc.layers()
    assert layers["Center"]["linetype"] == "CENTER"
    assert layers["Object"]["linetype"] == "Continuous"
    assert layers["0"]["handle"] == 0x10
    # Entities on the "Center" layer are BYLAYER and get the chain line through it.
    center_lines = [
        entity
        for entity in doc.modelspace().query("LINE")
        if entity.dxf["layer_handle"] == layers["Center"]["handle"]
    ]
    assert center_lines
    assert {entity.dxf["linetype"] for entity in center_lines} == {"BYLAYER"}


def test_layer_linetypes_cover_every_layer() -> None:
    for path in ACADSHARP_SAMPLES:
        layers = dict(ezdwg.raw.decode_layer_names(str(path)))
        linetype_handles = {handle for handle, *_ in ezdwg.raw.decode_linetypes(str(path))}
        layer_linetypes = dict(ezdwg.raw.decode_layer_linetypes(str(path)))

        assert set(layer_linetypes) == set(layers), path.name
        assert set(layer_linetypes.values()) <= linetype_handles, path.name


@pytest.mark.parametrize("path", ACADSHARP_SAMPLES, ids=lambda path: path.stem)
def test_hatch_pattern_definition_lines(path: Path) -> None:
    doc = ezdwg.read(str(path))
    hatches = list(doc.entities().query("HATCH"))
    patterns = [hatch for hatch in hatches if not hatch.dxf["solid_fill"]]

    assert len(hatches) == 8 and len(patterns) == 6
    assert all("pattern_lines" not in hatch.dxf for hatch in hatches if hatch.dxf["solid_fill"])
    # ANSI31: one family of 45 degree lines, 0.125 apart, without dashes.
    ansi31 = patterns[0].dxf
    assert ansi31["pattern_angle"] == pytest.approx(0.0)
    assert ansi31["pattern_scale"] == pytest.approx(1.0)
    assert ansi31["pattern_double"] is False
    (line,) = ansi31["pattern_lines"]
    assert line["angle"] == pytest.approx(45.0)
    assert line["dashes"] == []
    direction = math.radians(line["angle"])
    spacing = line["offset"][1] * math.cos(direction) - line["offset"][0] * math.sin(direction)
    assert spacing == pytest.approx(0.125)
    # AR-PARQ1: 14 families of dashed lines.
    parquet = patterns[1].dxf["pattern_lines"]
    assert len(parquet) == 14
    assert parquet[0]["dashes"] == pytest.approx([12.0, -12.0])

    # The raw rows keep the stored radians.
    raw_rows = ezdwg.raw.decode_hatch_patterns(str(path))
    assert [row[0] for row in raw_rows] == [hatch.handle for hatch in patterns]
    assert raw_rows[0][4][0][0] == pytest.approx(math.radians(45.0))


def test_written_r2000_entities_keep_their_layer(tmp_path: Path) -> None:
    # The R2000 writer stores the xdictionary handle that the format requires,
    # so the layer handle is read back from where a R2000 reader expects it.
    source = EXAMPLES / "line_2000.dwg"
    output = tmp_path / "roundtrip.dwg"
    ezdwg.read(str(source)).export_dwg(str(output))

    (line,) = ezdwg.read(str(output)).modelspace().query("LINE")
    # The writer puts every entity on its one layer, handle 2.
    assert line.dxf["layer_handle"] == 2
    assert line.dxf["linetype"] == "BYLAYER"
    assert line.dxf["linetype_scale"] == 1.0
