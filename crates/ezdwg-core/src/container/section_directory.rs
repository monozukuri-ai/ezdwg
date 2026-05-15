use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::io::ByteReader;

const SECTION_LOCATOR_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];
const MAX_SECTION_RECORDS: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    HeaderVariables,
    Classes,
    ObjectMap,
    Unknown3,
    Measurement,
    Unknown(u8),
}

impl SectionKind {
    pub fn from_record_no(record_no: u8) -> Self {
        match record_no {
            0 => Self::HeaderVariables,
            1 => Self::Classes,
            2 => Self::ObjectMap,
            3 => Self::Unknown3,
            4 => Self::Measurement,
            other => Self::Unknown(other),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::HeaderVariables => "HeaderVariables".to_string(),
            Self::Classes => "Classes".to_string(),
            Self::ObjectMap => "ObjectMap".to_string(),
            Self::Unknown3 => "Unknown3".to_string(),
            Self::Measurement => "Measurement".to_string(),
            Self::Unknown(value) => format!("Unknown({value})"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectionLocatorRecord {
    pub record_no: u8,
    pub offset: u32,
    pub size: u32,
    pub name: Option<String>,
}

impl SectionLocatorRecord {
    pub fn kind(&self) -> SectionKind {
        SectionKind::from_record_no(self.record_no)
    }
}

#[derive(Debug, Clone)]
pub struct SectionDirectory {
    pub record_count: u32,
    pub records: Vec<SectionLocatorRecord>,
    pub crc: u16,
    pub sentinel_ok: bool,
}

pub fn parse(bytes: &[u8]) -> Result<SectionDirectory> {
    parse_with_config(bytes, &ParseConfig::default())
}

pub fn parse_with_config(bytes: &[u8], config: &ParseConfig) -> Result<SectionDirectory> {
    if bytes.len() < 0x15 + 4 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "file too small for section directory",
        ));
    }

    let mut reader = ByteReader::new(bytes);
    reader.seek(0x15)?;
    let record_count = reader.read_u32_le()?;
    if record_count > MAX_SECTION_RECORDS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section directory record count {record_count} exceeds limit {MAX_SECTION_RECORDS}"
            ),
        ));
    }

    let mut records = Vec::with_capacity(record_count as usize);
    for _ in 0..record_count {
        let record_no = reader.read_u8()?;
        let offset = reader.read_u32_le()?;
        let size = reader.read_u32_le()?;
        records.push(SectionLocatorRecord {
            record_no,
            offset,
            size,
            name: None,
        });
    }

    let crc = reader.read_u16_le()?;
    let sentinel = reader.read_bytes(SECTION_LOCATOR_SENTINEL.len())?;
    let sentinel_ok = sentinel == SECTION_LOCATOR_SENTINEL;

    if config.strict && !sentinel_ok {
        return Err(
            DwgError::new(ErrorKind::Format, "section directory sentinel mismatch")
                .with_offset(reader.tell()),
        );
    }

    Ok(SectionDirectory {
        record_count,
        records,
        crc,
        sentinel_ok,
    })
}
