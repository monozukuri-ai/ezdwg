"""The Rust embedded-text primitives must behave exactly like the original
pure-Python implementations (kept in tests/_embedded_text_reference.py)."""

from __future__ import annotations

import random
from pathlib import Path

import pytest

import ezdwg._embedded_text as fast
import ezdwg.raw as raw_module

from tests._embedded_text_reference import (
    extract_plausible_embedded_text_fragment as ref_extract,
    is_plausible_embedded_text_char as ref_is_plausible,
    iter_shifted_short_direct_custom_text_fragments as ref_shifted_short,
    iter_shifted_visible_embedded_text_fragments as ref_shifted_visible,
    iter_utf16_runs_any_alignment as ref_runs,
    iter_visible_embedded_text_fragments as ref_visible,
    normalize_direct_custom_text_fragment as ref_normalize_direct,
    normalize_embedded_text_fragment as ref_normalize,
    score_embedded_text_fragment as ref_score,
    shift_bits_bytes as ref_shift,
)

REPO = Path(__file__).resolve().parents[1]
SAMPLE_DWGS = [
    path
    for path in [
        REPO / "test_dwg" / "acadsharp" / "sample_AC1027.dwg",
        REPO / "test_dwg" / "acadsharp" / "sample_AC1032.dwg",
        REPO / "SE167-2_0014.dwg",
        REPO / "SS101・102Y.dwg",
    ]
    if path.exists()
]

_INTERESTING_UNITS = (
    [0x0000, 0x0009, 0x001C, 0x001F, 0x0020, 0x0021, 0x002A, 0x003A, 0x0041, 0x0061, 0x007A, 0x0085]
    + [0x00A0, 0x0100, 0x2000, 0x2028, 0x3000, 0x3001, 0x3040, 0x30FF, 0x4E00, 0x9FFF, 0xA000, 0xFF08]
    + [0xFF0D, 0xFF1A, 0xD800, 0xFFFD, 0xFFFF]
)


def _random_blob(rng: random.Random, size: int) -> bytes:
    kind = rng.random()
    if kind < 0.35:
        return bytes(rng.getrandbits(8) for _ in range(size))
    units: list[int] = []
    while len(units) * 2 < size:
        pick = rng.random()
        if pick < 0.5:
            units.append(rng.choice(_INTERESTING_UNITS))
        elif pick < 0.8:
            units.append(rng.randint(0x20, 0x9FFF))
        else:
            units.append(rng.getrandbits(16))
    data = b"".join(u.to_bytes(2, "little") for u in units)
    if rng.random() < 0.5:
        data = b"\x00" + data  # odd alignment
    return data[:size]


# Text inputs are always produced by the UTF-16 run scanner (code units in
# 0x20..0x9FFF), so lone surrogates never reach the string helpers.
_TEXT_UNITS = [unit for unit in _INTERESTING_UNITS if not 0xD800 <= unit <= 0xDFFF]


def _random_text(rng: random.Random, size: int) -> str:
    chars = []
    for _ in range(size):
        pick = rng.random()
        if pick < 0.6:
            chars.append(chr(rng.choice(_TEXT_UNITS) or 0x20))
        else:
            chars.append(chr(rng.randint(0x20, 0x9FFF)))
    return "".join(chars)


@pytest.mark.parametrize("seed", range(6))
def test_primitives_match_reference_on_random_bytes(seed: int) -> None:
    rng = random.Random(seed)
    for _ in range(60):
        data = _random_blob(rng, rng.randint(0, 200))
        for shift in range(8):
            assert fast.shift_bits_bytes(data, shift) == ref_shift(data, shift)
        for min_chars in (1, 3):
            assert fast.iter_utf16_runs_any_alignment(data, min_chars=min_chars) == ref_runs(
                data, min_chars=min_chars
            )
        for min_score in (8, 14, 16):
            assert fast.iter_visible_embedded_text_fragments(data, min_score=min_score) == ref_visible(
                data, min_score=min_score
            )
        assert fast.iter_shifted_visible_embedded_text_fragments(data) == ref_shifted_visible(data)
        assert fast.iter_shifted_visible_embedded_text_fragments(
            data, shifts=(2, 3, 4), min_score=14
        ) == ref_shifted_visible(data, shifts=(2, 3, 4), min_score=14)
        assert fast.iter_shifted_short_direct_custom_text_fragments(
            data, shifts=(2, 3, 4), min_score=8
        ) == ref_shifted_short(data, shifts=(2, 3, 4), min_score=8)


@pytest.mark.parametrize("seed", range(4))
def test_string_helpers_match_reference_on_random_text(seed: int) -> None:
    rng = random.Random(100 + seed)
    for _ in range(300):
        text = _random_text(rng, rng.randint(0, 24))
        assert fast.score_embedded_text_fragment(text) == ref_score(text)
        assert fast.normalize_embedded_text_fragment(text) == ref_normalize(text)
        assert fast.extract_plausible_embedded_text_fragment(text) == ref_extract(text)
        assert fast.normalize_direct_custom_text_fragment(text) == ref_normalize_direct(text)
        for ch in text[:4]:
            assert fast.is_plausible_embedded_text_char(ch) == ref_is_plausible(ch)


def test_python_whitespace_semantics_in_normalize() -> None:
    # str.strip() removes U+001C..U+001F and U+3000 but not U+200B; Rust must agree
    for pad in ("\x1c", "\x1f", "　", " ", "\x85", "\xa0"):
        text = f"{pad}Plan A{pad}"
        assert fast.normalize_embedded_text_fragment(text) == ref_normalize(text) == "Plan A"
    zwsp = "​Plan​"
    assert fast.normalize_embedded_text_fragment(zwsp) == ref_normalize(zwsp) == zwsp


@pytest.mark.skipif(not SAMPLE_DWGS, reason="local DWG samples not present")
def test_primitives_match_reference_on_real_unknown_records() -> None:
    checked = 0
    for path in SAMPLE_DWGS:
        headers = raw_module.list_object_headers_with_type(str(path))
        candidates = [
            int(row[0])
            for row in headers
            if len(row) >= 6
            and str(row[5]).strip().upper() not in {"O", "OBJECT"}
            and int(row[2]) >= 512
            and (
                str(row[4]).strip().upper().startswith("UNKNOWN(")
                or str(row[4]).strip().upper() in {"ATTRIB", "ATTDEF", "SEQEND"}
            )
        ]
        for handle in candidates[:40]:
            try:
                row = raw_module.read_object_records_by_handle(str(path), [handle])[0]
            except Exception:
                continue
            data = bytes(row[4])
            assert fast.iter_visible_embedded_text_fragments(data) == ref_visible(data)
            assert fast.iter_shifted_visible_embedded_text_fragments(
                data, shifts=(2, 3, 4), min_score=14
            ) == ref_shifted_visible(data, shifts=(2, 3, 4), min_score=14)
            assert fast.iter_shifted_short_direct_custom_text_fragments(
                data, shifts=(4,), min_score=8
            ) == ref_shifted_short(data, shifts=(4,), min_score=8)
            checked += 1
    if checked == 0:
        pytest.skip("no large UNKNOWN entity records in the local samples")
