//! Legacy (code page) text encoding shared by the R2000 writer and the Python
//! preflight, so that the preflight replaces exactly the characters the writer
//! would reject.

use encoding_rs::Encoding;

use crate::bit::bit_reader::dwg_codepage_encoding;

fn encode_with(encoding: &'static Encoding, text: &str) -> Option<Vec<u8>> {
    if text.contains('\0') {
        return None;
    }
    let (bytes, _, errors) = encoding.encode(text);
    if errors {
        return None;
    }
    // Compare without BOM sniffing: a cp1252 string may start with "ÿþ" (FF FE),
    // which `Encoding::decode` would otherwise read as a UTF-16LE byte order mark.
    let (decoded, _) = encoding.decode_without_bom_handling(&bytes);
    (decoded == text).then(|| bytes.into_owned())
}

/// The bytes of `text` in the DWG code page (`$DWGCODEPAGE` numbering), or `None`
/// when the code page is unknown, a character has no mapping, the text contains
/// NUL or the bytes do not decode back to `text`.
pub fn encode_legacy_text(codepage: u16, text: &str) -> Option<Vec<u8>> {
    encode_with(dwg_codepage_encoding(Some(codepage))?, text)
}

/// Whether the writer stores `text` unchanged in the code page; `None` for an
/// unknown code page.
pub fn is_representable(codepage: u16, text: &str) -> Option<bool> {
    let encoding = dwg_codepage_encoding(Some(codepage))?;
    Some(encode_with(encoding, text).is_some())
}

/// `text` with every character the code page cannot store replaced by
/// `replacement`, and the number of replaced characters; `None` for an unknown
/// code page.
pub fn replace_unrepresentable(
    codepage: u16,
    text: &str,
    replacement: &str,
) -> Option<(String, usize)> {
    let encoding = dwg_codepage_encoding(Some(codepage))?;
    if encode_with(encoding, text).is_some() {
        return Some((text.to_owned(), 0));
    }
    let mut out = String::with_capacity(text.len());
    let mut replaced = 0usize;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        if encode_with(encoding, ch.encode_utf8(&mut buf)).is_some() {
            out.push(ch);
        } else {
            out.push_str(replacement);
            replaced += 1;
        }
    }
    Some((out, replaced))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_order_mark_lookalikes_are_ordinary_cp1252_text() {
        assert_eq!(
            encode_legacy_text(30, "ÿþ café"),
            Some(b"\xff\xfe caf\xe9".to_vec())
        );
        assert_eq!(is_representable(30, "ï»¿x"), Some(true));
        assert_eq!(is_representable(30, "þÿ"), Some(true));
    }

    #[test]
    fn characters_without_a_mapping_are_replaced_one_by_one() {
        // Python's cp932 maps byte A0 to U+F8F0; the Shift_JIS table has no such character
        assert_eq!(is_representable(38, "L\u{f8f0}990"), Some(false));
        assert_eq!(
            replace_unrepresentable(38, "L\u{f8f0}990", "?"),
            Some(("L?990".to_owned(), 1))
        );
        assert_eq!(
            replace_unrepresentable(38, "図面", "?"),
            Some(("図面".to_owned(), 0))
        );
        assert_eq!(is_representable(30, "a\0b"), Some(false));
        assert_eq!(is_representable(999, "x"), None);
        assert_eq!(replace_unrepresentable(999, "x", "?"), None);
    }
}
