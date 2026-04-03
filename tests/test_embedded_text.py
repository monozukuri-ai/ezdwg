from __future__ import annotations

import struct

import ezdwg._embedded_text as embedded_text_module
import ezdwg.document as document_module
import ezdwg.raw as raw_module


def _clear_document_caches() -> None:
    document_module._present_supported_types.cache_clear()
    document_module._entity_style_map.cache_clear()
    document_module._layer_color_map.cache_clear()
    document_module._layer_names_by_handle.cache_clear()
    document_module._geometry_layer_hint_rows.cache_clear()
    document_module._modelspace_block_handles.cache_clear()
    document_module._unknown_shifted_text_entities.cache_clear()


def _prefix_bits(data: bytes, shift: int) -> bytes:
    bit_string = ("0" * shift) + "".join(f"{value:08b}" for value in data)
    pad = (-len(bit_string)) % 8
    bit_string += "0" * pad
    return bytes(int(bit_string[index : index + 8], 2) for index in range(0, len(bit_string), 8))


def test_select_attdef_embedded_text_fragment_any_shift_recovers_shifted_utf16() -> None:
    desired = "Project-A/01".encode("utf-16le") + b"\x00\x00"
    shifted = _prefix_bits(desired, 5)

    assert embedded_text_module.select_attdef_embedded_text_fragment_any_shift(shifted) == "Project-A/01"


def test_collect_unknown_embedded_text_entities_emits_direct_attdef(monkeypatch) -> None:
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [(24, "Project-A/01", 32)],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: b"shift4" if shift == 4 else b"shift6",
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_attdef_direct_text_position",
        lambda shifted: (120.0, 240.0) if shifted == b"shift4" else None,
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_attdef_direct_text_height",
        lambda shifted: 35.0 if shifted == b"shift6" else None,
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(10, 100, 640, 3, "ATTDEF", "Entity")],
        lambda _path, _handles: [(10, 100, 640, 3, b"record-bytes")],
        modelspace_owner_handle=77,
    )

    assert rows == (
        (10, "Project-A/01", (120.0, 240.0, 0.0), 35.0, 0.0, 77, "TEXT", "direct_attdef"),
    )


def test_collect_unknown_embedded_text_entities_emits_marker_based_unknown(monkeypatch) -> None:
    marker = "TEXT".encode("utf-16le")
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [(96, "工事名 Project", 24)],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: (b"aaaa" + marker + b"bbbb") if shift == 4 else b"shift6",
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_shifted_embedded_text_position",
        lambda shifted, marker_offset: (3200.0, 480.0)
        if shifted.startswith(b"aaaa") and marker_offset == 4
        else None,
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_shifted_embedded_text_height",
        lambda shifted, marker_offset: 45.0 if shifted == b"shift6" and marker_offset == 4 else None,
    )
    monkeypatch.setattr(
        embedded_text_module,
        "select_nearby_embedded_text_fragment",
        lambda fragments, marker_offset: "工事名 Project"
        if fragments and marker_offset == 4
        else None,
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(11, 120, 768, 694, "UNKNOWN(0x2B6)", "Entity")],
        lambda _path, _handles: [(11, 120, 768, 694, b"record-bytes")],
        modelspace_owner_handle=88,
    )

    assert rows == (
        (11, "工事名 Project", (3200.0, 480.0, 0.0), 45.0, 0.0, 88, "TEXT", "marker"),
    )


def test_select_nearby_shifted_value_fragment_prefers_value_side_candidates() -> None:
    fragments = [
        (4, 111, "設計担当者名", 28),
        (2, 164, "Project-A Main Tower", 40),
        (0, 52, "Old Value", 22),
    ]

    text = embedded_text_module.select_nearby_shifted_value_fragment(fragments, 100)

    assert text == "Project-A Main Tower"


def test_select_nearby_shifted_value_fragment_prefers_ascii_value_over_gibberish() -> None:
    fragments = [
        (7, 175, "娀怀怀樀渀搀渀爀栀怀戀娀怀戀", 56),
        (0, 153, "DOC-001", 32),
    ]

    text = embedded_text_module.select_nearby_shifted_value_fragment(fragments, 100)

    assert text == "DOC-001"


def test_collect_unknown_embedded_text_entities_prefers_shifted_value_fragment(monkeypatch) -> None:
    marker = "TEXT".encode("utf-16le")
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [(111, "設計担当者名", 28)],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_visible_embedded_text_fragments",
        lambda _data, shifts=range(8), min_score=16: [
            (4, 111, "設計担当者名", 28),
            (0, 68, "Project-A Main Tower", 40),
        ],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: (b"aaaa" + marker + b"bbbb") if shift == 4 else b"shift6",
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_shifted_embedded_text_position",
        lambda shifted, marker_offset: (3200.0, 480.0)
        if shifted.startswith(b"aaaa") and marker_offset == 4
        else None,
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_shifted_embedded_text_height",
        lambda shifted, marker_offset: 45.0 if shifted == b"shift6" and marker_offset == 4 else None,
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(11, 120, 768, 694, "UNKNOWN(0x2B6)", "Entity")],
        lambda _path, _handles: [(11, 120, 768, 694, b"record-bytes")],
        modelspace_owner_handle=88,
    )

    assert rows == (
        (11, "Project-A Main Tower", (3200.0, 480.0, 0.0), 45.0, 0.0, 88, "TEXT", "marker"),
    )


def test_extract_direct_custom_text_entity_layout_recovers_shift4_label_pattern() -> None:
    offset = 100
    shifted_buffers = {shift: bytearray(256) for shift in (0, 2, 4, 6)}
    struct.pack_into("<d", shifted_buffers[6], offset - 41, 2400.0)
    struct.pack_into("<d", shifted_buffers[6], offset - 33, 320.0)
    struct.pack_into("<d", shifted_buffers[0], offset - 17, 55.0)

    layout = embedded_text_module.extract_direct_custom_text_entity_layout(
        {shift: bytes(buffer) for shift, buffer in shifted_buffers.items()},
        4,
        offset,
    )

    assert layout == (2400.0, 320.0, 55.0)


def test_extract_direct_custom_text_entity_layout_recovers_shift3_fragment_pattern() -> None:
    offset = 120
    shifted_buffers = {shift: bytearray(256) for shift in (0, 2, 4, 6)}
    struct.pack_into("<d", shifted_buffers[2], offset - 37, 4200.0)
    struct.pack_into("<d", shifted_buffers[0], offset - 29, 180.0)
    struct.pack_into("<d", shifted_buffers[4], offset - 18, 36.0)

    layout = embedded_text_module.extract_direct_custom_text_entity_layout(
        {shift: bytes(buffer) for shift, buffer in shifted_buffers.items()},
        3,
        offset,
    )

    assert layout == (4200.0, 180.0, 36.0)


def test_extract_direct_custom_text_entity_layout_recovers_shift2_value_pattern() -> None:
    offset = 140
    shifted_buffers = {shift: bytearray(256) for shift in (0, 2, 4, 6)}
    struct.pack_into("<d", shifted_buffers[6], offset - 46, 3150.5)
    struct.pack_into("<d", shifted_buffers[6], offset - 38, 410.25)
    struct.pack_into("<d", shifted_buffers[0], offset - 14, 48.0)

    layout = embedded_text_module.extract_direct_custom_text_entity_layout(
        {shift: bytes(buffer) for shift, buffer in shifted_buffers.items()},
        2,
        offset,
    )

    assert layout == (3150.5, 410.25, 48.0)


def test_extract_short_direct_custom_text_entity_layout_recovers_shift2_alt_xy_with_height_hint() -> None:
    offset = 220
    shifted_buffers = {shift: bytearray(320) for shift in (0, 2, 4, 6)}
    struct.pack_into("<d", shifted_buffers[6], offset - 42, 2300.4)
    struct.pack_into("<d", shifted_buffers[6], offset - 34, 275.5)

    layout = embedded_text_module.extract_short_direct_custom_text_entity_layout(
        {shift: bytes(buffer) for shift, buffer in shifted_buffers.items()},
        2,
        offset,
        height_hint=42.0,
    )

    assert layout == (2300.4, 275.5, 42.0)


def test_extract_short_direct_custom_text_entity_layout_recovers_shift2_alt_xy_with_direct_height() -> None:
    offset = 260
    shifted_buffers = {shift: bytearray(320) for shift in (0, 2, 4, 6)}
    struct.pack_into("<d", shifted_buffers[4], offset - 41, 6200.0)
    struct.pack_into("<d", shifted_buffers[4], offset - 33, 515.5)
    struct.pack_into("<d", shifted_buffers[6], offset - 16, 42.0)

    layout = embedded_text_module.extract_short_direct_custom_text_entity_layout(
        {shift: bytes(buffer) for shift, buffer in shifted_buffers.items()},
        2,
        offset,
        height_hint=None,
    )

    assert layout == (6200.0, 515.5, 42.0)


def test_collect_unknown_embedded_text_entities_emits_direct_custom_fragments(monkeypatch) -> None:
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_visible_embedded_text_fragments",
        lambda _data, shifts=range(8), min_score=16: (
            [(3, 100, "Rev-A", 30), (4, 180, "Project Name", 18)]
            if tuple(shifts) == (2, 3, 4)
            else []
        ),
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: f"shift{shift}".encode("ascii"),
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_direct_custom_text_entity_layout",
        lambda shifted_buffers, fragment_shift, fragment_offset: (
            (4200.0, 180.0, 36.0)
            if fragment_shift == 3 and fragment_offset == 100 and shifted_buffers[2] == b"shift2"
            else (2400.0, 320.0, 55.0)
            if fragment_shift == 4 and fragment_offset == 180 and shifted_buffers[6] == b"shift6"
            else None
        ),
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(11, 120, 768, 694, "UNKNOWN(0x2B6)", "Entity")],
        lambda _path, _handles: [(11, 120, 768, 694, b"record-bytes")],
        modelspace_owner_handle=88,
    )

    assert rows == (
        (11, "Rev-A", (4200.0, 180.0, 0.0), 36.0, 0.0, 88, "MTEXT", "direct_custom_mtext"),
        (11, "Project Name", (2400.0, 320.0, 0.0), 55.0, 0.0, 88, "TEXT", "direct_custom_text"),
    )


def test_iter_shifted_short_direct_custom_text_fragments_trims_trailing_noise() -> None:
    shifted = "Label*".encode("utf-16le") + b"\x00\x00"

    rows = embedded_text_module.iter_shifted_short_direct_custom_text_fragments(
        shifted,
        shifts=(0,),
        min_score=8,
    )

    assert rows == [(0, 0, "Label", 10)]


def test_collect_unknown_embedded_text_entities_emits_short_direct_custom_fragment(monkeypatch) -> None:
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_visible_embedded_text_fragments",
        lambda _data, shifts=range(8), min_score=16: [],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_short_direct_custom_text_fragments",
        lambda _data, shifts=(4,), min_score=8: [(4, 180, "Label", 10)],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: f"shift{shift}".encode("ascii"),
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_direct_custom_text_entity_layout",
        lambda shifted_buffers, fragment_shift, fragment_offset: (
            (2810.0, 375.0, 42.0)
            if fragment_shift == 4 and fragment_offset == 180 and shifted_buffers[6] == b"shift6"
            else None
        ),
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(11, 120, 768, 694, "UNKNOWN(0x2B6)", "Entity")],
        lambda _path, _handles: [(11, 120, 768, 694, b"record-bytes")],
        modelspace_owner_handle=88,
    )

    assert rows == (
        (11, "Label", (2810.0, 375.0, 0.0), 42.0, 0.0, 88, "TEXT", "short_direct_custom_text"),
    )


def test_collect_unknown_embedded_text_entities_uses_height_hint_for_shift2_short_fragment(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        embedded_text_module,
        "iter_visible_embedded_text_fragments",
        lambda _data, min_score=16: [],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_visible_embedded_text_fragments",
        lambda _data, shifts=range(8), min_score=16: [(4, 180, "Header Label", 18)]
        if tuple(shifts) == (2, 3, 4)
        else [],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "iter_shifted_short_direct_custom_text_fragments",
        lambda _data, shifts=(2, 3, 4), min_score=8: [(2, 220, "Issue Date", 14)],
    )
    monkeypatch.setattr(
        embedded_text_module,
        "shift_bits_bytes",
        lambda _data, shift: f"shift{shift}".encode("ascii"),
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_direct_custom_text_entity_layout",
        lambda shifted_buffers, fragment_shift, fragment_offset: (
            (2400.0, 320.0, 42.0)
            if fragment_shift == 4 and fragment_offset == 180
            else None
        ),
    )
    monkeypatch.setattr(
        embedded_text_module,
        "extract_short_direct_custom_text_entity_layout",
        lambda shifted_buffers, fragment_shift, fragment_offset, *, height_hint=None: (
            (2300.4, 275.5, height_hint)
            if fragment_shift == 2 and fragment_offset == 220 and height_hint == 42.0
            else None
        ),
    )

    rows = embedded_text_module.collect_unknown_embedded_text_entities(
        "dummy.dwg",
        lambda _path: [(11, 120, 768, 694, "UNKNOWN(0x2B6)", "Entity")],
        lambda _path, _handles: [(11, 120, 768, 694, b"record-bytes")],
        modelspace_owner_handle=88,
    )

    assert rows == (
        (11, "Header Label", (2400.0, 320.0, 0.0), 42.0, 0.0, 88, "TEXT", "direct_custom_text"),
        (11, "Issue Date", (2300.4, 275.5, 0.0), 42.0, 0.0, 88, "TEXT", "short_direct_custom_text"),
    )


def test_raw_decode_unknown_embedded_text_entities_wraps_shared_helper(monkeypatch) -> None:
    raw_module._decode_unknown_embedded_text_entities_cached.cache_clear()
    monkeypatch.setattr(
        raw_module,
        "collect_unknown_embedded_text_entities",
        lambda path, list_headers_with_type, read_records_by_handle, *, modelspace_owner_handle=None, limit=None: (
            (
                1,
                "Project-A/01",
                (10.0, 20.0, 0.0),
                30.0,
                0.0,
                modelspace_owner_handle,
                "TEXT",
                "marker",
            ),
        ),
    )

    rows = raw_module.decode_unknown_embedded_text_entities(
        "dummy.dwg",
        modelspace_owner_handle=42,
        limit=5,
    )

    assert rows == [(1, "Project-A/01", (10.0, 20.0, 0.0), 30.0, 0.0, 42, "TEXT", "marker")]


def test_document_unknown_shifted_text_entities_delegates_to_raw(monkeypatch) -> None:
    _clear_document_caches()
    monkeypatch.setattr(document_module, "_modelspace_block_handles", lambda _path: (55,))
    monkeypatch.setattr(
        document_module.raw,
        "decode_unknown_embedded_text_entities",
        lambda path, modelspace_owner_handle=None: [
            (
                2,
                "Project-A/01",
                (40.0, 60.0, 0.0),
                25.0,
                0.0,
                modelspace_owner_handle,
                "TEXT",
                "marker",
            )
        ],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_object_entity_layer_handles",
        lambda _path, _handles, limit=None: [(2, 130)],
    )

    rows = document_module._unknown_shifted_text_entities("dummy.dwg")

    assert rows == ((2, "Project-A/01", (40.0, 60.0, 0.0), 25.0, 0.0, 55, 130, "TEXT", "marker"),)


def test_attach_entity_color_uses_seeded_layer_handle_when_style_missing() -> None:
    dxf = document_module._attach_entity_color(
        999,
        {"layer_handle": 130},
        {},
        {130: (5, None)},
    )

    assert dxf["layer_handle"] == 130
    assert dxf["resolved_color_index"] == 5


def test_infer_nearby_layer_handle_prefers_dominant_geometry(monkeypatch) -> None:
    monkeypatch.setattr(
        document_module,
        "_geometry_layer_hint_rows",
        lambda _path: (
            (130, 0.0, 100.0, 0.0, 100.0, 4),
            (130, 25.0, 75.0, 10.0, 90.0, 3),
            (140, 400.0, 450.0, 0.0, 50.0, 4),
        ),
    )

    layer_handle = document_module._infer_nearby_layer_handle("dummy.dwg", (50.0, 50.0, 0.0))

    assert layer_handle == 130


def test_geometry_layer_hint_rows_skip_absurd_extents(monkeypatch) -> None:
    document_module._geometry_layer_hint_rows.cache_clear()
    monkeypatch.setattr(
        document_module,
        "_entity_style_map",
        lambda _path: {
            1: (None, None, 130, None, None),
            2: (None, None, 133, None, None),
        },
    )
    monkeypatch.setattr(
        document_module,
        "_line_arc_circle_rows",
        lambda _path: (
            (),
            (
                (1, 0.0, 0.0, 0.0, 3.089337021186409e157, 0.0, 0.0),
                (2, 150.0, 250.0, 0.0, 25.0, 0.0, 0.0),
            ),
            (),
        ),
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_lwpolyline_entities",
        lambda _path: [],
    )

    rows = document_module._geometry_layer_hint_rows("dummy_absurd_geometry.dwg")

    assert rows == ((133, 125.0, 175.0, 225.0, 275.0, 2),)


def test_document_query_text_uses_inferred_layer_for_synthetic_unknown_text(monkeypatch) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x01, "TEXT", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_text_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [(130, 5, None)])
    monkeypatch.setattr(
        document_module,
        "_unknown_shifted_text_entities",
        lambda _path: ((2, "Project-A/01", (40.0, 60.0, 0.0), 25.0, 0.0, None, None, "TEXT", None),),
    )
    monkeypatch.setattr(document_module, "_infer_nearby_layer_handle", lambda _path, _insert: 130)

    doc = document_module.Document(path="dummy_unknown_text.dwg", version="AC1021")
    entities = list(doc.modelspace().query("TEXT"))

    assert len(entities) == 1
    assert entities[0].dxf["layer_handle"] == 130
    assert entities[0].dxf["resolved_color_index"] == 5


def test_document_query_text_prefers_intrinsic_source_layer_for_synthetic_unknown_text(
    monkeypatch,
) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x01, "TEXT", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_text_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(130, 5, None), (133, 3, None)],
    )
    monkeypatch.setattr(
        document_module,
        "_unknown_shifted_text_entities",
        lambda _path: ((2, "Project-A/01", (40.0, 60.0, 0.0), 25.0, 0.0, None, 133, "TEXT", None),),
    )
    monkeypatch.setattr(
        document_module,
        "_is_plausible_nearby_layer_handle",
        lambda _path, layer_handle, _insert: layer_handle == 133,
    )
    monkeypatch.setattr(document_module, "_infer_nearby_layer_handle", lambda _path, _insert: 130)

    doc = document_module.Document(path="dummy_unknown_text_source_layer.dwg", version="AC1021")
    entities = list(doc.modelspace().query("TEXT"))

    assert len(entities) == 1
    assert entities[0].dxf["layer_handle"] == 133
    assert entities[0].dxf["resolved_color_index"] == 3


def test_document_query_text_ignores_intrinsic_source_layer_when_not_nearby(
    monkeypatch,
) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x01, "TEXT", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_text_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(130, 5, None), (160, 2, None)],
    )
    monkeypatch.setattr(
        document_module,
        "_unknown_shifted_text_entities",
        lambda _path: ((2, "Project-A/01", (40.0, 60.0, 0.0), 25.0, 0.0, None, 160, "TEXT", None),),
    )
    monkeypatch.setattr(
        document_module,
        "_is_plausible_nearby_layer_handle",
        lambda _path, _layer_handle, _insert: False,
    )
    monkeypatch.setattr(document_module, "_infer_nearby_layer_handle", lambda _path, _insert: 130)

    doc = document_module.Document(path="dummy_unknown_text_source_layer_fallback.dwg", version="AC1021")
    entities = list(doc.modelspace().query("TEXT"))

    assert len(entities) == 1
    assert entities[0].dxf["layer_handle"] == 130
    assert entities[0].dxf["resolved_color_index"] == 5


def test_document_query_mtext_uses_unknown_shifted_text_mtext_hint(monkeypatch) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x02, "MTEXT", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_mtext_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_layer_colors", lambda _path: [(130, 5, None)])
    monkeypatch.setattr(
        document_module,
        "_unknown_shifted_text_entities",
        lambda _path: ((2, "Rev-A", (40.0, 60.0, 0.0), 25.0, 0.0, None, 130, "MTEXT", "direct_custom_mtext"),),
    )
    monkeypatch.setattr(
        document_module,
        "_is_plausible_nearby_layer_handle",
        lambda _path, layer_handle, _insert: layer_handle == 130,
    )

    doc = document_module.Document(path="dummy_unknown_mtext_hint.dwg", version="AC1021")
    entities = list(doc.modelspace().query("MTEXT"))

    assert len(entities) == 1
    assert entities[0].dxftype == "MTEXT"
    assert entities[0].dxf["text"] == "Rev-A"
    assert entities[0].dxf["layer_handle"] == 130


def test_infer_adjacent_recovered_text_layer_handle_prefers_base_or_text_pair(monkeypatch) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_names",
        lambda _path: [
            (128, 'bad\x01name'),
            (132, "TITLE_LAYER"),
            (133, "TITLE_LAYER_TEXT"),
            (200, "OTHER"),
            (201, "OTHER_TEXT"),
        ],
    )

    assert (
        document_module._infer_adjacent_recovered_text_layer_handle(
            "dummy_pair_layers.dwg",
            128,
            "direct_custom_text",
        )
        == 132
    )
    document_module._layer_names_by_handle.cache_clear()
    assert (
        document_module._infer_adjacent_recovered_text_layer_handle(
            "dummy_pair_layers.dwg",
            128,
            "marker",
        )
        == 133
    )


def test_document_query_text_uses_adjacent_layer_pair_for_recovered_unknown_text(monkeypatch) -> None:
    _clear_document_caches()
    monkeypatch.setattr(
        document_module.raw,
        "list_object_headers_with_type",
        lambda _path: [(100, 0, 0, 0x01, "TEXT", "Entity")],
    )
    monkeypatch.setattr(document_module.raw, "decode_text_entities", lambda _path: [])
    monkeypatch.setattr(document_module.raw, "decode_entity_styles", lambda _path: [])
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_colors",
        lambda _path: [(130, 5, None), (132, 3, None), (133, 1, None)],
    )
    monkeypatch.setattr(
        document_module.raw,
        "decode_layer_names",
        lambda _path: [
            (128, 'bad\x01name'),
            (132, "TITLE_LAYER"),
            (133, "TITLE_LAYER_TEXT"),
        ],
    )
    monkeypatch.setattr(
        document_module,
        "_unknown_shifted_text_entities",
        lambda _path: (
            (2, "Section Label", (2400.0, 320.0, 0.0), 42.0, 0.0, None, 128, "TEXT", "direct_custom_text"),
            (3, "Project Value", (2550.0, 375.0, 0.0), 42.0, 0.0, None, 128, "TEXT", "marker"),
        ),
    )
    monkeypatch.setattr(
        document_module,
        "_is_plausible_nearby_layer_handle",
        lambda _path, _layer_handle, _insert: False,
    )
    monkeypatch.setattr(document_module, "_infer_nearby_layer_handle", lambda _path, _insert: 130)

    doc = document_module.Document(path="dummy_unknown_text_adjacent_pair.dwg", version="AC1021")
    entities = list(doc.modelspace().query("TEXT"))

    assert len(entities) == 2
    assert entities[0].dxf["layer_handle"] == 132
    assert entities[0].dxf["resolved_color_index"] == 3
    assert entities[1].dxf["layer_handle"] == 133
    assert entities[1].dxf["resolved_color_index"] == 1
