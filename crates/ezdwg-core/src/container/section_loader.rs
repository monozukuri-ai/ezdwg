use crate::container::section_directory::{SectionDirectory, SectionLocatorRecord};
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct SectionSlice<'a> {
    pub record: SectionLocatorRecord,
    pub data: Cow<'a, [u8]>,
}

pub fn load_section<'a>(
    bytes: &'a [u8],
    record: SectionLocatorRecord,
    config: &ParseConfig,
) -> Result<SectionSlice<'a>> {
    let size = record.size as u64;
    if size > config.max_section_bytes {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section size {} exceeds limit {}",
                size, config.max_section_bytes
            ),
        ));
    }

    let offset = record.offset as usize;
    let size = record.size as usize;
    let end = offset
        .checked_add(size)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section range overflow"))?;

    if end > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section out of range: offset {offset} size {size} (file size {})",
                bytes.len()
            ),
        ));
    }

    let data = Cow::Borrowed(&bytes[offset..end]);
    Ok(SectionSlice { record, data })
}

pub fn load_section_by_index<'a>(
    bytes: &'a [u8],
    directory: &SectionDirectory,
    index: usize,
    config: &ParseConfig,
) -> Result<SectionSlice<'a>> {
    let record = directory
        .records
        .get(index)
        .cloned()
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section index out of range"))?;
    load_section(bytes, record, config)
}

pub fn load_all_sections<'a>(
    bytes: &'a [u8],
    directory: &SectionDirectory,
    config: &ParseConfig,
) -> Result<Vec<SectionSlice<'a>>> {
    let mut sections = Vec::with_capacity(directory.records.len());
    for record in &directory.records {
        sections.push(load_section(bytes, record.clone(), config)?);
    }
    Ok(sections)
}
