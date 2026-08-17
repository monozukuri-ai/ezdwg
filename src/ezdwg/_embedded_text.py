"""Recovery of text embedded in undecoded (custom / proxy) object payloads.

The byte/character scanning primitives live in the Rust core
(``ezdwg._core.embedded_text_*``); this module keeps the layout heuristics and
the per-record orchestration. ``tests/_embedded_text_reference.py`` holds the
original pure-Python primitives for equivalence testing.
"""

from __future__ import annotations

import math
import struct
from typing import Any, Callable

from ezdwg import _core


def shift_bits_bytes(data: bytes, shift: int) -> bytes:
    """Shift the byte string right by ``shift`` bits (0..7) with carry across bytes."""
    if shift <= 0:
        return data
    return _core.embedded_text_shift_bits_bytes(bytes(data), shift)


def iter_utf16_runs_any_alignment(data: bytes, *, min_chars: int = 3) -> list[tuple[int, str]]:
    return _core.embedded_text_utf16_runs(bytes(data), min_chars)


def is_plausible_embedded_text_char(ch: str) -> bool:
    return _core.embedded_text_is_plausible_char(ch)


def score_embedded_text_fragment(text: str) -> int:
    return _core.embedded_text_score_fragment(text)


def normalize_embedded_text_fragment(text: str) -> str:
    return _core.embedded_text_normalize_fragment(text)


def extract_plausible_embedded_text_fragment(text: str) -> str | None:
    return _core.embedded_text_extract_plausible_fragment(text)


def normalize_direct_custom_text_fragment(text: str) -> str | None:
    return _core.embedded_text_normalize_direct_custom_fragment(text)


def iter_visible_embedded_text_fragments(
    data: bytes,
    *,
    min_score: int = 16,
) -> list[tuple[int, str, int]]:
    return _core.embedded_text_visible_fragments(bytes(data), min_score)


def iter_shifted_visible_embedded_text_fragments(
    data: bytes,
    *,
    shifts: range | tuple[int, ...] = range(8),
    min_score: int = 16,
) -> list[tuple[int, int, str, int]]:
    return _core.embedded_text_shifted_visible_fragments(bytes(data), list(shifts), min_score)


def iter_shifted_short_direct_custom_text_fragments(
    data: bytes,
    *,
    shifts: range | tuple[int, ...] = (4,),
    min_score: int = 8,
) -> list[tuple[int, int, str, int]]:
    return _core.embedded_text_shifted_short_direct_fragments(bytes(data), list(shifts), min_score)


def read_f64_le(data: bytes, offset: int) -> float | None:
    if offset < 0 or offset + 8 > len(data):
        return None
    try:
        value = struct.unpack_from("<d", data, offset)[0]
    except Exception:
        return None
    if not math.isfinite(value):
        return None
    return float(value)


def is_plausible_direct_custom_text_position(x: float | None, y: float | None) -> bool:
    if x is None or y is None:
        return False
    if abs(x) <= 1e-9 or abs(x) > 100_000.0:
        return False
    if abs(x) < 1_000.0:
        return False
    if y < 0.0 or y > 10_000.0:
        return False
    return True


def is_plausible_direct_custom_text_height(value: float | None) -> bool:
    if value is None:
        return False
    return 5.0 <= value <= 500.0


def extract_direct_custom_text_entity_layout(
    shifted_buffers: dict[int, bytes],
    fragment_shift: int,
    fragment_offset: int,
) -> tuple[float, float, float] | None:
    if fragment_shift == 4:
        x = read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 41)
        y = read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 33)
        height = read_f64_le(shifted_buffers.get(0, b""), fragment_offset - 17)
    elif fragment_shift == 3:
        x = read_f64_le(shifted_buffers.get(2, b""), fragment_offset - 37)
        y = read_f64_le(shifted_buffers.get(0, b""), fragment_offset - 29)
        height = read_f64_le(shifted_buffers.get(4, b""), fragment_offset - 18)
        if not is_plausible_direct_custom_text_height(height):
            height = read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 7)
    elif fragment_shift == 2:
        x = read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 46)
        y = read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 38)
        height = read_f64_le(shifted_buffers.get(0, b""), fragment_offset - 14)
    else:
        return None

    if not is_plausible_direct_custom_text_position(x, y):
        return None
    if not is_plausible_direct_custom_text_height(height):
        return None
    return (float(x), float(y), float(height))


def select_direct_custom_text_height_hint(heights: list[float]) -> float | None:
    if not heights:
        return None
    counts: dict[int, tuple[int, float]] = {}
    for value in heights:
        rounded = int(round(float(value) * 1000.0))
        count, _ = counts.get(rounded, (0, float(value)))
        counts[rounded] = (count + 1, float(value))
    _key, (_, best_value) = max(counts.items(), key=lambda item: (item[1][0], -abs(item[0])))
    return best_value


def extract_short_direct_custom_text_entity_layout(
    shifted_buffers: dict[int, bytes],
    fragment_shift: int,
    fragment_offset: int,
    *,
    height_hint: float | None = None,
) -> tuple[float, float, float] | None:
    layout = extract_direct_custom_text_entity_layout(
        shifted_buffers,
        fragment_shift,
        fragment_offset,
    )
    if layout is not None:
        return layout
    if fragment_shift != 2:
        return None

    candidates = [
        (
            read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 42),
            read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 34),
            read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 16),
        ),
        (
            read_f64_le(shifted_buffers.get(4, b""), fragment_offset - 41),
            read_f64_le(shifted_buffers.get(4, b""), fragment_offset - 33),
            read_f64_le(shifted_buffers.get(6, b""), fragment_offset - 16),
        ),
    ]
    for x, y, height in candidates:
        if not is_plausible_direct_custom_text_position(x, y):
            continue
        chosen_height = height if is_plausible_direct_custom_text_height(height) else height_hint
        if not is_plausible_direct_custom_text_height(chosen_height):
            continue
        return (float(x), float(y), float(chosen_height))
    return None


def extract_shifted_embedded_text_height(shifted_six: bytes, marker_offset: int) -> float | None:
    best_value: float | None = None
    best_score = -10_000
    for offset in range(marker_offset + 48, min(len(shifted_six) - 8, marker_offset + 72)):
        value = read_f64_le(shifted_six, offset)
        if value is None or value < 5.0 or value > 500.0:
            continue
        score = 0
        if value >= 40.0:
            score += 20
        if abs(value - round(value)) < 1e-6:
            score += 12
        elif abs(value * 4.0 - round(value * 4.0)) < 1e-6:
            score += 8
        elif abs(value * 8.0 - round(value * 8.0)) < 1e-6:
            score += 4
        if value >= 80.0:
            score += 6
        if score > best_score:
            best_score = score
            best_value = value
    if best_score <= 0:
        return None
    return best_value


def extract_shifted_embedded_text_position(
    shifted_four: bytes, marker_offset: int
) -> tuple[float, float] | None:
    best: tuple[int, int, float, float] | None = None
    for rel_x in range(21, 34):
        rel_y = rel_x + 8
        x = read_f64_le(shifted_four, marker_offset + rel_x)
        y = read_f64_le(shifted_four, marker_offset + rel_y)
        if x is None or y is None:
            continue
        if abs(x) <= 1e-12 or abs(y) <= 1e-12:
            continue
        if abs(x) > 100_000 or abs(y) > 100_000:
            continue
        score = 0
        if 1_000.0 <= abs(x) <= 50_000.0:
            score += 12
        if 0.0 <= y <= 10_000.0:
            score += 8
        if rel_x in {25, 29, 21}:
            score += 4
        elif rel_x in {23, 27, 31}:
            score += 2
        candidate = (score, rel_x, float(x), float(y))
        if best is None or candidate > best:
            best = candidate
    if best is None:
        return None
    return (best[2], best[3])


def select_nearby_embedded_text_fragment(
    fragments: list[tuple[int, str, int]], marker_offset: int
) -> str | None:
    nearby = [
        fragment
        for fragment in fragments
        if marker_offset + 36 <= fragment[0] <= marker_offset + 120
    ]
    if not nearby:
        return None
    best = max(
        nearby,
        key=lambda item: (item[2], -abs(item[0] - (marker_offset + 64)), len(item[1])),
    )
    return best[1]


def select_nearby_shifted_value_fragment(
    fragments: list[tuple[int, int, str, int]],
    marker_offset: int,
) -> str | None:
    def _naturalness_bonus(text: str) -> int:
        bonus = 0
        if any(ch.isascii() and ch.isalnum() for ch in text):
            bonus += 24
        if any(ch in ".-/" for ch in text):
            bonus += 8
        if any(ch in {" ", "\u3000", "・", "（", "）", "／", "－", "："} for ch in text):
            bonus += 4
        if any(0x3040 <= ord(ch) <= 0x30FF for ch in text):
            bonus += 6
        if len(text) >= 8:
            bonus += 4
        return bonus

    pool = []
    for shift, offset, text, score in fragments:
        if shift in {4, 6}:
            continue
        delta = offset - marker_offset
        if delta < 40 or delta > 112:
            continue
        pool.append((shift, offset, text, score + _naturalness_bonus(text), delta))
    if not pool:
        return None
    best = max(
        pool,
        key=lambda item: (
            item[3],
            -abs(item[4] - 64),
            len(item[2]),
            -item[0],
        ),
    )
    return best[2]


def select_attdef_embedded_text_fragment(fragments: list[tuple[int, str, int]]) -> str | None:
    if not fragments:
        return None
    preferred = [
        fragment
        for fragment in fragments
        if any(ch.isascii() and ch.isalnum() for ch in fragment[1])
        and any(ch in ".-/" for ch in fragment[1])
    ]
    pool = preferred or fragments
    best = max(pool, key=lambda item: (item[2], len(item[1])))
    return best[1]


def select_attdef_embedded_text_fragment_any_shift(data: bytes) -> str | None:
    best: tuple[int, int, str] | None = None
    for shift, _offset, text, score in iter_shifted_visible_embedded_text_fragments(data):
        if not any(ch.isascii() and ch.isalnum() for ch in text):
            continue
        if not any(ch in ".-/" for ch in text):
            continue
        candidate = (score, -shift, text)
        if best is None or candidate > best:
            best = candidate
    if best is None:
        return None
    return best[2]


def extract_attdef_direct_text_position(shifted_four: bytes) -> tuple[float, float] | None:
    x = read_f64_le(shifted_four, 42)
    y = read_f64_le(shifted_four, 50)
    if x is None or y is None:
        return None
    if abs(x) <= 1e-12 or abs(y) <= 1e-12:
        return None
    if abs(x) > 100_000 or abs(y) > 100_000:
        return None
    return (float(x), float(y))


def extract_attdef_direct_text_height(shifted_six: bytes) -> float | None:
    best_value: float | None = None
    best_score = -10_000
    for offset in range(72, min(len(shifted_six) - 8, 84)):
        value = read_f64_le(shifted_six, offset)
        if value is None or value < 5.0 or value > 500.0:
            continue
        score = 0
        if 20.0 <= value <= 200.0:
            score += 20
        if abs(value - round(value)) < 1e-6:
            score += 8
        if score > best_score:
            best_score = score
            best_value = value
    if best_score <= 0:
        return None
    return best_value


def _read_candidate_records(
    path: str,
    handles: list[int],
    read_records_by_handle: Callable[[str, list[int]], list[tuple[Any, ...]]],
) -> dict[int, tuple[Any, ...]]:
    """Read all candidate records with one raw call.

    The raw reader rebuilds the object index for every call, so reading the
    candidates one handle at a time cost ~20ms x N. If the batch call fails
    (one unreadable record aborts the whole call), fall back to per-handle
    reads so a single bad record does not hide the others.
    """
    if not handles:
        return {}
    try:
        rows = list(read_records_by_handle(path, list(handles)))
    except Exception:
        rows = []
        for handle in handles:
            try:
                rows.extend(read_records_by_handle(path, [handle]))
            except Exception:
                continue
    out: dict[int, tuple[Any, ...]] = {}
    for row in rows:
        if not isinstance(row, tuple) or not row:
            continue
        try:
            handle = int(row[0])
        except Exception:
            continue
        out.setdefault(handle, row)
    return out


def collect_unknown_embedded_text_entities(
    path: str,
    list_headers_with_type: Callable[[str], list[tuple[Any, ...]]],
    read_records_by_handle: Callable[[str, list[int]], list[tuple[Any, ...]]],
    *,
    modelspace_owner_handle: int | None = None,
    limit: int | None = None,
) -> tuple[
    tuple[int, str, tuple[float, float, float], float, float, int | None, str | None, str | None],
    ...,
]:
    try:
        header_rows = list_headers_with_type(path)
    except Exception:
        return ()

    candidate_rows: dict[int, tuple[int, int]] = {}
    for row in header_rows:
        if not isinstance(row, tuple) or len(row) < 6:
            continue
        try:
            handle = int(row[0])
            offset = int(row[1])
            size = int(row[2])
            type_code = int(row[3])
        except Exception:
            continue
        type_name = str(row[4]).strip().upper()
        type_class = str(row[5]).strip().upper()
        if type_class in {"O", "OBJECT"}:
            continue
        if size < 512:
            continue
        if not (type_name.startswith("UNKNOWN(") or type_name in {"ATTRIB", "ATTDEF", "SEQEND"}):
            continue
        previous = candidate_rows.get(handle)
        if previous is None or offset > previous[0]:
            candidate_rows[handle] = (offset, type_code)

    out: list[
        tuple[int, str, tuple[float, float, float], float, float, int | None, str | None, str | None]
    ] = []
    seen: set[tuple[str, int, int, int]] = set()

    ordered_candidates = sorted(candidate_rows.items())
    rows_by_handle = _read_candidate_records(
        path,
        [handle for handle, _ in ordered_candidates],
        read_records_by_handle,
    )
    for source_handle, (_offset, expected_type_code) in ordered_candidates:
        row = rows_by_handle.get(source_handle)
        if row is None:
            continue
        if not isinstance(row, tuple) or len(row) < 5:
            continue
        try:
            actual_type_code = int(row[3])
        except Exception:
            actual_type_code = expected_type_code
        record_bytes = bytes(row[4]) if row[4] is not None else b""
        if not record_bytes:
            continue
        visible_fragments = iter_visible_embedded_text_fragments(record_bytes)
        direct_fragments = (
            iter_shifted_visible_embedded_text_fragments(
                record_bytes,
                shifts=(2, 3, 4),
                min_score=14,
            )
            if actual_type_code != 3
            else []
        )
        short_direct_fragment_shifts = (2, 3, 4) if len(record_bytes) <= 16384 else (4,)
        short_direct_fragments = (
            iter_shifted_short_direct_custom_text_fragments(
                record_bytes,
                shifts=short_direct_fragment_shifts,
                min_score=8,
            )
            if actual_type_code != 3
            else []
        )
        if not visible_fragments and not direct_fragments and not short_direct_fragments and actual_type_code != 3:
            continue
        shifted_buffers = {
            0: record_bytes,
            2: shift_bits_bytes(record_bytes, 2),
            3: shift_bits_bytes(record_bytes, 3),
            4: shift_bits_bytes(record_bytes, 4),
            6: shift_bits_bytes(record_bytes, 6),
        }
        shifted_four = shifted_buffers[4]
        shifted_six = shifted_buffers[6]
        direct_text_heights: list[float] = []
        emitted_direct_attdef = False
        if actual_type_code == 3:
            direct_text = select_attdef_embedded_text_fragment(visible_fragments)
            if direct_text is None:
                direct_text = select_attdef_embedded_text_fragment_any_shift(record_bytes)
            direct_position = extract_attdef_direct_text_position(shifted_four)
            direct_height = extract_attdef_direct_text_height(shifted_six)
            if direct_text is not None and direct_position is not None and direct_height is not None:
                direct_x, direct_y = direct_position
                key = (
                    direct_text,
                    int(round(direct_x * 1000.0)),
                    int(round(direct_y * 1000.0)),
                    int(round(direct_height * 1000.0)),
                )
                if key not in seen:
                    seen.add(key)
                    out.append(
                        (
                            int(source_handle),
                            direct_text,
                            (direct_x, direct_y, 0.0),
                            direct_height,
                            0.0,
                            modelspace_owner_handle,
                            "TEXT",
                            "direct_attdef",
                        )
                    )
                    emitted_direct_attdef = True
                    if limit is not None and len(out) >= limit:
                        break
        if actual_type_code == 3 and emitted_direct_attdef:
            continue

        marker = "TEXT".encode("utf-16le")
        shifted_fragments = (
            iter_shifted_visible_embedded_text_fragments(record_bytes, shifts=(0, 2, 3))
            if actual_type_code != 3
            else []
        )
        marker_offset = 0
        while True:
            marker_offset = shifted_four.find(marker, marker_offset)
            if marker_offset < 0:
                break
            position = extract_shifted_embedded_text_position(shifted_four, marker_offset)
            height = extract_shifted_embedded_text_height(shifted_six, marker_offset)
            if position is None or height is None:
                marker_offset += 2
                continue
            x, y = position
            text = None
            if shifted_fragments:
                text = select_nearby_shifted_value_fragment(shifted_fragments, marker_offset)
            if text is None:
                text = select_nearby_embedded_text_fragment(visible_fragments, marker_offset)
            if text is None and actual_type_code == 3:
                text = select_attdef_embedded_text_fragment(visible_fragments)
            if text is None:
                marker_offset += 2
                continue
            key = (text, int(round(x * 1000.0)), int(round(y * 1000.0)), int(round(height * 1000.0)))
            if key in seen:
                marker_offset += 2
                continue
            seen.add(key)
            out.append(
                (
                    int(source_handle),
                    text,
                    (x, y, 0.0),
                    height,
                    0.0,
                    modelspace_owner_handle,
                    "TEXT",
                    "marker",
                )
            )
            if limit is not None and len(out) >= limit:
                break
            marker_offset += 2
        if limit is not None and len(out) >= limit:
            break
        if actual_type_code != 3:
            for fragment_shift, fragment_offset, text, score in direct_fragments:
                if fragment_shift == 2 and score < 18:
                    continue
                layout = extract_direct_custom_text_entity_layout(
                    shifted_buffers,
                    fragment_shift,
                    fragment_offset,
                )
                if layout is None:
                    continue
                x, y, height = layout
                key = (
                    text,
                    int(round(x * 1000.0)),
                    int(round(y * 1000.0)),
                    int(round(height * 1000.0)),
                )
                if key in seen:
                    continue
                seen.add(key)
                direct_text_heights.append(height)
                out.append(
                    (
                        int(source_handle),
                        text,
                        (x, y, 0.0),
                        height,
                        0.0,
                        modelspace_owner_handle,
                        "MTEXT" if fragment_shift == 3 else "TEXT",
                        "direct_custom_mtext" if fragment_shift == 3 else "direct_custom_text",
                    )
                )
                if limit is not None and len(out) >= limit:
                    break
            if limit is None or len(out) < limit:
                height_hint = select_direct_custom_text_height_hint(direct_text_heights)
                for fragment_shift, fragment_offset, text, score in short_direct_fragments:
                    layout = extract_short_direct_custom_text_entity_layout(
                        shifted_buffers,
                        fragment_shift,
                        fragment_offset,
                        height_hint=height_hint,
                    )
                    if layout is None:
                        continue
                    x, y, height = layout
                    key = (
                        text,
                        int(round(x * 1000.0)),
                        int(round(y * 1000.0)),
                        int(round(height * 1000.0)),
                    )
                    if key in seen:
                        continue
                    seen.add(key)
                    out.append(
                        (
                            int(source_handle),
                            text,
                            (x, y, 0.0),
                            height,
                            0.0,
                            modelspace_owner_handle,
                            "TEXT",
                            "short_direct_custom_text",
                        )
                    )
                    if limit is not None and len(out) >= limit:
                        break
        if limit is not None and len(out) >= limit:
            break

    return tuple(out)
