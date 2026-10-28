// Legacy (code page) text checks shared with the R2000 writer
// (ezdwg_core::bit::legacy_text), so `_write_preflight` replaces exactly the
// characters the writer would reject instead of trusting Python's codecs alone.

#[pyfunction]
pub fn legacy_text_is_representable(codepage: u16, text: &str) -> PyResult<bool> {
    crate::bit::legacy_text::is_representable(codepage, text)
        .ok_or_else(|| PyValueError::new_err(format!("unsupported DWG codepage {codepage}")))
}

#[pyfunction(signature = (codepage, text, replacement="?"))]
pub fn legacy_text_replace_unrepresentable(
    codepage: u16,
    text: &str,
    replacement: &str,
) -> PyResult<(String, usize)> {
    crate::bit::legacy_text::replace_unrepresentable(codepage, text, replacement)
        .ok_or_else(|| PyValueError::new_err(format!("unsupported DWG codepage {codepage}")))
}
