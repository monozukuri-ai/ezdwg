//! Native AC1032 writer. Shares geometry and database allocation with R2000;
//! version-dependent record layouts and the paged container are encoded separately.
mod container;
mod header;
pub(crate) mod streams;

pub fn write_document(
    doc: &super::WriterDocument,
    config: &super::WriterConfig,
) -> crate::core::result::Result<Vec<u8>> {
    if doc.version != crate::dwg::version::DwgVersion::R2018 {
        return Err(crate::core::error::DwgError::new(
            crate::core::error::ErrorKind::Unsupported,
            "r2018 writer requires AC1032",
        ));
    }
    super::r2000::write_document_versioned(doc, config)
}

pub(crate) use container::write_container;
pub(crate) use header::encode_header;
