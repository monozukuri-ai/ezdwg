use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgVersion {
    R2000,
    R2004,
    Unknown(String),
}

impl DwgVersion {
    pub fn as_str(&self) -> &str {
        match self {
            Self::R2000 => "AC1015",
            Self::R2004 => "AC1018",
            Self::Unknown(value) => value.as_str(),
        }
    }

    pub fn is_supported(&self) -> bool {
        matches!(self, Self::R2000)
    }
}

pub fn detect_version(bytes: &[u8]) -> Result<DwgVersion> {
    if bytes.len() < 6 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "file too small to contain DWG version",
        ));
    }
    let tag = std::str::from_utf8(&bytes[..6]).unwrap_or("");
    let version = match tag {
        "AC1015" => DwgVersion::R2000,
        "AC1018" => DwgVersion::R2004,
        other => DwgVersion::Unknown(other.to_string()),
    };
    Ok(version)
}
