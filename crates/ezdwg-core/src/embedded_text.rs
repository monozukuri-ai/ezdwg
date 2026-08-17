//! Byte-level heuristics for recovering text embedded in undecoded (custom /
//! proxy) object payloads.
//!
//! These primitives were originally written in Python
//! (`ezdwg/_embedded_text.py`). They are hot: every UNKNOWN record of a
//! drawing is scanned at several bit shifts, and the pure-Python version took
//! minutes on real-world drawings with a few hundred large custom records.
//! The semantics here intentionally mirror the Python reference
//! implementations exactly (including Python's definition of whitespace for
//! `str.strip()`), so the recovery results are unchanged.

/// Shift the whole byte string right by `shift` bits (0..=7), carrying bits
/// across byte boundaries. `shift == 0` returns a copy of the input.
pub fn shift_bits_bytes(data: &[u8], shift: u32) -> Vec<u8> {
    if shift == 0 || shift > 7 {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut carry: u8 = 0;
    for &value in data {
        out.push((value >> shift) | carry);
        carry = value.wrapping_shl(8 - shift);
    }
    out
}

fn is_run_code_unit(code: u16) -> bool {
    // U+3000 is inside 0x20..=0x9FFF, kept explicit to mirror the reference.
    code == 0x3000 || (0x20..=0x9FFF).contains(&code)
}

/// Find runs of at least `min_chars` UTF-16LE code units in the printable
/// range 0x20..=0x9FFF, at both byte alignments. Returns `(byte_offset, text)`
/// sorted by offset.
pub fn utf16_runs_any_alignment(data: &[u8], min_chars: usize) -> Vec<(usize, String)> {
    let mut runs: Vec<(usize, String)> = Vec::new();
    let len = data.len();
    // Python: `while index < len(data) - min_chars * 2` (may be negative → no loop)
    let limit = len as i64 - (min_chars as i64) * 2;
    for parity in 0..2usize {
        let mut index = parity;
        while (index as i64) < limit {
            let mut cursor = index;
            let mut chars = String::new();
            let mut count = 0usize;
            while cursor + 1 < len {
                let code = data[cursor] as u16 | ((data[cursor + 1] as u16) << 8);
                if code == 0 {
                    break;
                }
                if is_run_code_unit(code) {
                    // 0x20..=0x9FFF never contains surrogates, so this is a valid char.
                    chars.push(char::from_u32(code as u32).unwrap_or('\u{FFFD}'));
                    count += 1;
                    cursor += 2;
                    continue;
                }
                break;
            }
            if count >= min_chars {
                runs.push((index, chars));
                index = cursor;
            } else {
                index += 2;
            }
        }
    }
    runs.sort_by_key(|(offset, _)| *offset);
    runs
}

fn is_ascii_alnum(ch: char) -> bool {
    ch.is_ascii_digit() || ch.is_ascii_uppercase() || ch.is_ascii_lowercase()
}

fn is_kana(ch: char) -> bool {
    ('\u{3040}'..='\u{30FF}').contains(&ch)
}

fn is_cjk(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

/// Mirrors `is_plausible_embedded_text_char` in the Python reference.
pub fn is_plausible_embedded_text_char(ch: char) -> bool {
    if " .,:;/_-()[]{}&+*#%'\"".contains(ch) {
        return true;
    }
    if matches!(
        ch,
        ' ' | '\u{3000}' | '・' | '（' | '）' | '／' | '－' | '：'
    ) {
        return true;
    }
    is_ascii_alnum(ch) || is_kana(ch) || is_cjk(ch)
}

/// Mirrors `score_embedded_text_fragment` in the Python reference.
pub fn score_embedded_text_fragment(text: &str) -> i64 {
    if text.is_empty() {
        return -10_000;
    }
    let mut score: i64 = 0;
    for ch in text.chars() {
        if is_ascii_alnum(ch) {
            score += 2;
        } else if matches!(
            ch,
            ' ' | '\u{3000}' | '-' | '_' | '.' | '/' | '(' | ')' | '・' | '（' | '）' | '：'
        ) {
            score += 1;
        } else if is_kana(ch) {
            score += 3;
        } else if is_cjk(ch) {
            score += 4;
        } else {
            score -= 8;
        }
    }
    score
}

/// Python's `str.isspace()` definition (bidirectional WS/B/S or category Zs),
/// which is what `str.strip()` removes. Differs from `char::is_whitespace`
/// (e.g. U+001C..U+001F).
pub fn is_python_space(ch: char) -> bool {
    matches!(
        ch as u32,
        0x09..=0x0D
            | 0x1C..=0x20
            | 0x85
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
    )
}

fn py_strip(text: &str) -> &str {
    text.trim_matches(is_python_space)
}

fn py_rstrip(text: &str) -> &str {
    text.trim_end_matches(is_python_space)
}

/// Mirrors `normalize_embedded_text_fragment` in the Python reference.
pub fn normalize_embedded_text_fragment(text: &str) -> String {
    let mut text = py_strip(text);
    if text.is_empty() {
        return String::new();
    }
    let has_cjk = text.chars().any(is_cjk);
    if has_cjk {
        if let Some(last) = text.chars().next_back() {
            if last.is_ascii() && last.is_ascii_alphabetic() {
                text = py_rstrip(&text[..text.len() - last.len_utf8()]);
            }
        }
    }
    text.to_string()
}

fn has_visible_char(text: &str) -> bool {
    text.chars()
        .any(|ch| is_ascii_alnum(ch) || is_kana(ch) || is_cjk(ch))
}

/// Mirrors `extract_plausible_embedded_text_fragment` in the Python reference:
/// split on implausible characters, keep the best-scoring normalized segment.
pub fn extract_plausible_embedded_text_fragment(text: &str) -> Option<String> {
    let mut best = String::new();
    let mut best_score = score_embedded_text_fragment("");
    let mut current = String::new();
    let consider = |current: &mut String, best: &mut String, best_score: &mut i64| {
        let candidate = normalize_embedded_text_fragment(current);
        let score = score_embedded_text_fragment(&candidate);
        if score > *best_score {
            *best = candidate;
            *best_score = score;
        }
        current.clear();
    };
    for ch in text.chars() {
        if is_plausible_embedded_text_char(ch) {
            current.push(ch);
            continue;
        }
        consider(&mut current, &mut best, &mut best_score);
    }
    consider(&mut current, &mut best, &mut best_score);
    if best.is_empty() || !has_visible_char(&best) {
        return None;
    }
    Some(best)
}

/// Mirrors `normalize_direct_custom_text_fragment` in the Python reference.
pub fn normalize_direct_custom_text_fragment(text: &str) -> Option<String> {
    let normalized = normalize_embedded_text_fragment(text);
    if normalized.is_empty() {
        return None;
    }
    let trimmed = normalized
        .trim_end_matches(|ch| " :*;,-".contains(ch))
        .trim_end_matches(|ch| "：＊；，".contains(ch));
    if !trimmed.is_empty()
        && score_embedded_text_fragment(trimmed) >= score_embedded_text_fragment(&normalized)
    {
        return Some(trimmed.to_string());
    }
    Some(normalized)
}

/// Mirrors `iter_visible_embedded_text_fragments`: `(offset, fragment, score)`.
pub fn visible_embedded_text_fragments(data: &[u8], min_score: i64) -> Vec<(usize, String, i64)> {
    let mut fragments = Vec::new();
    for (offset, run) in utf16_runs_any_alignment(data, 3) {
        let Some(fragment) = extract_plausible_embedded_text_fragment(&run) else {
            continue;
        };
        let score = score_embedded_text_fragment(&fragment);
        if score < min_score {
            continue;
        }
        fragments.push((offset, fragment, score));
    }
    fragments
}

/// Mirrors `iter_shifted_visible_embedded_text_fragments`:
/// `(shift, offset, text, score)` for each shift in order.
pub fn shifted_visible_embedded_text_fragments(
    data: &[u8],
    shifts: &[u32],
    min_score: i64,
) -> Vec<(u32, usize, String, i64)> {
    let mut out = Vec::new();
    for &shift in shifts {
        let shifted;
        let view: &[u8] = if shift == 0 {
            data
        } else {
            shifted = shift_bits_bytes(data, shift);
            &shifted
        };
        for (offset, text, score) in visible_embedded_text_fragments(view, min_score) {
            out.push((shift, offset, text, score));
        }
    }
    out
}

/// Mirrors `iter_shifted_short_direct_custom_text_fragments`:
/// short (<= 8 chars) label-like fragments that survive direct-custom
/// normalization, `(shift, offset, normalized, score)`.
pub fn shifted_short_direct_custom_text_fragments(
    data: &[u8],
    shifts: &[u32],
    min_score: i64,
) -> Vec<(u32, usize, String, i64)> {
    let mut out = Vec::new();
    for &shift in shifts {
        let shifted;
        let view: &[u8] = if shift == 0 {
            data
        } else {
            shifted = shift_bits_bytes(data, shift);
            &shifted
        };
        for (offset, run) in utf16_runs_any_alignment(view, 1) {
            let Some(fragment) = extract_plausible_embedded_text_fragment(&run) else {
                continue;
            };
            let Some(normalized) = normalize_direct_custom_text_fragment(&fragment) else {
                continue;
            };
            if normalized.chars().count() > 8 {
                continue;
            }
            if normalized == fragment
                && !normalized.contains('\u{3000}')
                && !fragment.chars().any(|ch| ch == ':' || ch == '*')
            {
                continue;
            }
            let score = score_embedded_text_fragment(&normalized);
            if score < min_score {
                continue;
            }
            out.push((shift, offset, normalized, score));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    #[test]
    fn shift_bits_carries_across_bytes() {
        assert_eq!(
            shift_bits_bytes(&[0b1111_0000, 0b0000_1111], 4),
            vec![0b0000_1111, 0b0000_0000]
        );
        assert_eq!(shift_bits_bytes(&[0xAB, 0xCD], 0), vec![0xAB, 0xCD]);
        assert_eq!(shift_bits_bytes(&[0x01, 0x00], 1), vec![0x00, 0x80]);
    }

    #[test]
    fn utf16_runs_find_both_alignments_and_respect_min_chars() {
        let mut data = vec![0x00u8];
        data.extend(utf16le("AB"));
        data.extend([0x00, 0x00]);
        data.extend(utf16le("Hello"));
        data.extend([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let runs = utf16_runs_any_alignment(&data, 3);
        // The odd alignment also yields a (junk) run, exactly like the reference.
        assert_eq!(
            runs,
            vec![(6, "䠀攀氀氀漀".to_string()), (7, "Hello".to_string())]
        );
        // A run occupying exactly the last 6 bytes is skipped, like the reference.
        let tail = utf16le("XYZ");
        assert!(utf16_runs_any_alignment(&tail, 3).is_empty());
        let mut padded = utf16le("XYZ");
        padded.extend([0x00, 0x00]);
        assert_eq!(
            utf16_runs_any_alignment(&padded, 3),
            vec![(0, "XYZ".to_string())]
        );
    }

    #[test]
    fn scoring_and_normalization_match_reference_rules() {
        assert_eq!(score_embedded_text_fragment(""), -10_000);
        assert_eq!(score_embedded_text_fragment("A1 "), 2 + 2 + 1);
        assert_eq!(score_embedded_text_fragment("図面"), 8);
        assert_eq!(score_embedded_text_fragment("あ!"), 3 - 8);
        assert_eq!(normalize_embedded_text_fragment("  図面 X "), "図面");
        assert_eq!(
            normalize_embedded_text_fragment("\u{3000}Plan A\u{1c}"),
            "Plan A"
        );
        assert_eq!(normalize_embedded_text_fragment("Plan A"), "Plan A");
        assert_eq!(
            normalize_direct_custom_text_fragment("Title:*"),
            Some("Title".to_string())
        );
        assert_eq!(normalize_direct_custom_text_fragment(" "), None);
    }

    #[test]
    fn extract_keeps_best_plausible_segment() {
        assert_eq!(
            extract_plausible_embedded_text_fragment("!!図面番号!!A-1"),
            Some("図面番号".to_string())
        );
        assert_eq!(extract_plausible_embedded_text_fragment("--- ///"), None);
        assert_eq!(extract_plausible_embedded_text_fragment(""), None);
    }

    #[test]
    fn visible_fragments_pipeline() {
        let mut data = vec![0x00u8; 4];
        data.extend(utf16le("Project-A/01"));
        data.extend(vec![0x00u8; 8]);
        let fragments = visible_embedded_text_fragments(&data, 16);
        // Reference output: the odd-alignment junk run scores higher (7 CJK-range
        // code units) and is kept alongside the real text.
        assert_eq!(
            fragments,
            vec![
                (3, "倀爀漀樀攀挀琀".to_string(), 28),
                (4, "Project-A/01".to_string(), 22)
            ]
        );
        let shifted = shifted_visible_embedded_text_fragments(&data, &[0, 2], 16);
        assert_eq!(
            shifted,
            vec![
                (0, 3, "倀爀漀樀攀挀琀".to_string(), 28),
                (0, 4, "Project-A/01".to_string(), 22)
            ]
        );
    }
}
