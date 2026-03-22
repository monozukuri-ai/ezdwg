from __future__ import annotations

import math
import struct
from typing import Any, Callable


def shift_bits_bytes(data: bytes, shift: int) -> bytes:
    if shift <= 0:
        return data
    out = bytearray(len(data))
    carry = 0
    for index, value in enumerate(data):
        out[index] = ((value >> shift) | carry) & 0xFF
        carry = (value << (8 - shift)) & 0xFF
    return bytes(out)


def iter_utf16_runs_any_alignment(data: bytes, *, min_chars: int = 3) -> list[tuple[int, str]]:
    runs: list[tuple[int, str]] = []
    for parity in (0, 1):
        index = parity
        while index < len(data) - min_chars * 2:
            cursor = index
            chars: list[str] = []
            while cursor + 1 < len(data):
                code = data[cursor] | (data[cursor + 1] << 8)
                if code == 0:
                    break
                if code == 0x3000 or 32 <= code <= 0x9FFF:
                    chars.append(chr(code))
                    cursor += 2
                    continue
                break
            if len(chars) >= min_chars:
                runs.append((index, "".join(chars)))
                index = cursor
            else:
                index += 2
    runs.sort(key=lambda item: item[0])
    return runs


def is_plausible_embedded_text_char(ch: str) -> bool:
    code = ord(ch)
    if ch in " .,:;/_-()[]{}&+*#%'\"":
        return True
    if ch in {" ", "\u3000", "・", "（", "）", "／", "－", "："}:
        return True
    if "0" <= ch <= "9" or "A" <= ch <= "Z" or "a" <= ch <= "z":
        return True
    if 0x3040 <= code <= 0x30FF:
        return True
    if 0x4E00 <= code <= 0x9FFF:
        return True
    return False


def score_embedded_text_fragment(text: str) -> int:
    if not text:
        return -10_000
    score = 0
    for ch in text:
        code = ord(ch)
        if "0" <= ch <= "9" or "A" <= ch <= "Z" or "a" <= ch <= "z":
            score += 2
        elif ch in {" ", "\u3000", "-", "_", ".", "/", "(", ")", "・", "（", "）", "："}:
            score += 1
        elif 0x3040 <= code <= 0x30FF:
            score += 3
        elif 0x4E00 <= code <= 0x9FFF:
            score += 4
        else:
            score -= 8
    return score


def normalize_embedded_text_fragment(text: str) -> str:
    text = text.strip()
    if not text:
        return text
    has_cjk = any(0x4E00 <= ord(ch) <= 0x9FFF for ch in text)
    if has_cjk and text[-1].isascii() and text[-1].isalpha():
        text = text[:-1].rstrip()
    return text


def extract_plausible_embedded_text_fragment(text: str) -> str | None:
    best = ""
    current: list[str] = []
    for ch in text:
        if is_plausible_embedded_text_char(ch):
            current.append(ch)
            continue
        candidate = normalize_embedded_text_fragment("".join(current))
        if score_embedded_text_fragment(candidate) > score_embedded_text_fragment(best):
            best = candidate
        current.clear()
    candidate = normalize_embedded_text_fragment("".join(current))
    if score_embedded_text_fragment(candidate) > score_embedded_text_fragment(best):
        best = candidate
    if not best:
        return None
    has_visible = any(
        ("0" <= ch <= "9")
        or ("A" <= ch <= "Z")
        or ("a" <= ch <= "z")
        or (0x3040 <= ord(ch) <= 0x30FF)
        or (0x4E00 <= ord(ch) <= 0x9FFF)
        for ch in best
    )
    if not has_visible:
        return None
    return best


def iter_visible_embedded_text_fragments(
    data: bytes,
    *,
    min_score: int = 16,
) -> list[tuple[int, str, int]]:
    fragments: list[tuple[int, str, int]] = []
    for offset, run in iter_utf16_runs_any_alignment(data, min_chars=3):
        fragment = extract_plausible_embedded_text_fragment(run)
        if not fragment:
            continue
        score = score_embedded_text_fragment(fragment)
        if score < min_score:
            continue
        fragments.append((offset, fragment, score))
    return fragments


def iter_shifted_visible_embedded_text_fragments(
    data: bytes,
    *,
    shifts: range | tuple[int, ...] = range(8),
    min_score: int = 16,
) -> list[tuple[int, int, str, int]]:
    out: list[tuple[int, int, str, int]] = []
    for shift in shifts:
        shifted = shift_bits_bytes(data, shift) if shift else data
        for offset, text, score in iter_visible_embedded_text_fragments(shifted, min_score=min_score):
            out.append((shift, offset, text, score))
    return out


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


def collect_unknown_embedded_text_entities(
    path: str,
    list_headers_with_type: Callable[[str], list[tuple[Any, ...]]],
    read_records_by_handle: Callable[[str, list[int]], list[tuple[Any, ...]]],
    *,
    modelspace_owner_handle: int | None = None,
    limit: int | None = None,
) -> tuple[tuple[int, str, tuple[float, float, float], float, float, int | None], ...]:
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

    out: list[tuple[int, str, tuple[float, float, float], float, float, int | None]] = []
    seen: set[tuple[str, int, int, int]] = set()

    for source_handle, (_offset, expected_type_code) in sorted(candidate_rows.items()):
        try:
            row = read_records_by_handle(path, [source_handle])[0]
        except Exception:
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
        if not visible_fragments and actual_type_code != 3:
            continue
        shifted_four = shift_bits_bytes(record_bytes, 4)
        shifted_six = shift_bits_bytes(record_bytes, 6)
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
                )
            )
            if limit is not None and len(out) >= limit:
                break
            marker_offset += 2
        if limit is not None and len(out) >= limit:
            break

    return tuple(out)
