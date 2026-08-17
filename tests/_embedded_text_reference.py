"""Pure-Python reference implementations of the embedded-text scanning
primitives, kept verbatim from the original ``ezdwg._embedded_text`` module.

The production module now delegates these to the Rust core; the tests compare
both implementations on random and fixture-derived byte strings so the
recovery behaviour is provably unchanged.
"""

from __future__ import annotations


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


def normalize_direct_custom_text_fragment(text: str) -> str | None:
    normalized = normalize_embedded_text_fragment(text)
    if not normalized:
        return None
    trimmed = normalized.rstrip(" :*;,-")
    trimmed = trimmed.rstrip("：＊；，")
    if trimmed and score_embedded_text_fragment(trimmed) >= score_embedded_text_fragment(normalized):
        normalized = trimmed
    if not normalized:
        return None
    return normalized


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


def iter_shifted_short_direct_custom_text_fragments(
    data: bytes,
    *,
    shifts: range | tuple[int, ...] = (4,),
    min_score: int = 8,
) -> list[tuple[int, int, str, int]]:
    out: list[tuple[int, int, str, int]] = []
    for shift in shifts:
        shifted = shift_bits_bytes(data, shift) if shift else data
        for offset, run in iter_utf16_runs_any_alignment(shifted, min_chars=1):
            fragment = extract_plausible_embedded_text_fragment(run)
            if not fragment:
                continue
            normalized = normalize_direct_custom_text_fragment(fragment)
            if not normalized:
                continue
            if len(normalized) > 8:
                continue
            if normalized == fragment and "\u3000" not in normalized and not any(ch in fragment for ch in ":*"):
                continue
            score = score_embedded_text_fragment(normalized)
            if score < min_score:
                continue
            out.append((shift, offset, normalized, score))
    return out
