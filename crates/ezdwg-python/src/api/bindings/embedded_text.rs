// Embedded-text recovery primitives (see ezdwg_core::embedded_text). These are
// the hot loops behind `ezdwg._embedded_text`; the Python module keeps the
// orchestration and calls into these for the byte/character scanning.

type EmbeddedTextRunRow = (usize, String);
type EmbeddedTextFragmentRow = (usize, String, i64);
type EmbeddedTextShiftedFragmentRow = (u32, usize, String, i64);

#[pyfunction]
pub fn embedded_text_shift_bits_bytes(py: Python<'_>, data: &[u8], shift: u32) -> PyObject {
    let shifted = embedded_text::shift_bits_bytes(data, shift);
    pyo3::types::PyBytes::new_bound(py, &shifted).into()
}

#[pyfunction(signature = (data, min_chars=3))]
pub fn embedded_text_utf16_runs(data: &[u8], min_chars: usize) -> Vec<EmbeddedTextRunRow> {
    embedded_text::utf16_runs_any_alignment(data, min_chars)
}

#[pyfunction]
pub fn embedded_text_is_plausible_char(ch: &str) -> PyResult<bool> {
    let mut chars = ch.chars();
    match (chars.next(), chars.next()) {
        (Some(single), None) => Ok(embedded_text::is_plausible_embedded_text_char(single)),
        _ => Err(PyValueError::new_err("expected a single character")),
    }
}

#[pyfunction]
pub fn embedded_text_score_fragment(text: &str) -> i64 {
    embedded_text::score_embedded_text_fragment(text)
}

#[pyfunction]
pub fn embedded_text_normalize_fragment(text: &str) -> String {
    embedded_text::normalize_embedded_text_fragment(text)
}

#[pyfunction]
pub fn embedded_text_extract_plausible_fragment(text: &str) -> Option<String> {
    embedded_text::extract_plausible_embedded_text_fragment(text)
}

#[pyfunction]
pub fn embedded_text_normalize_direct_custom_fragment(text: &str) -> Option<String> {
    embedded_text::normalize_direct_custom_text_fragment(text)
}

#[pyfunction(signature = (data, min_score=16))]
pub fn embedded_text_visible_fragments(
    data: &[u8],
    min_score: i64,
) -> Vec<EmbeddedTextFragmentRow> {
    embedded_text::visible_embedded_text_fragments(data, min_score)
}

#[pyfunction(signature = (data, shifts, min_score=16))]
pub fn embedded_text_shifted_visible_fragments(
    data: &[u8],
    shifts: Vec<u32>,
    min_score: i64,
) -> Vec<EmbeddedTextShiftedFragmentRow> {
    embedded_text::shifted_visible_embedded_text_fragments(data, &shifts, min_score)
}

#[pyfunction(signature = (data, shifts, min_score=8))]
pub fn embedded_text_shifted_short_direct_fragments(
    data: &[u8],
    shifts: Vec<u32>,
    min_score: i64,
) -> Vec<EmbeddedTextShiftedFragmentRow> {
    embedded_text::shifted_short_direct_custom_text_fragments(data, &shifts, min_score)
}
