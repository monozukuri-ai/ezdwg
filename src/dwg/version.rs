use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DwgVersion {
    R2000,
    R2004,
    R2007,
    R2010,
    R2013,
    R2018,
    Unknown(String),
}

impl DwgVersion {
    pub fn as_str(&self) -> &str {
        match self {
            Self::R2000 => "AC1015",
            Self::R2004 => "AC1018",
            Self::R2007 => "AC1021",
            Self::R2010 => "AC1024",
            Self::R2013 => "AC1027",
            Self::R2018 => "AC1032",
            Self::Unknown(value) => value.as_str(),
        }
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
        "AC1021" => DwgVersion::R2007,
        "AC1024" => DwgVersion::R2010,
        "AC1027" => DwgVersion::R2013,
        "AC1032" => DwgVersion::R2018,
        other => DwgVersion::Unknown(other.to_string()),
    };
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::{detect_version, DwgVersion};

    #[test]
    fn detects_known_versions() {
        assert_eq!(detect_version(b"AC1015xxxx").unwrap(), DwgVersion::R2000);
        assert_eq!(detect_version(b"AC1018xxxx").unwrap(), DwgVersion::R2004);
        assert_eq!(detect_version(b"AC1021xxxx").unwrap(), DwgVersion::R2007);
        assert_eq!(detect_version(b"AC1024xxxx").unwrap(), DwgVersion::R2010);
        assert_eq!(detect_version(b"AC1027xxxx").unwrap(), DwgVersion::R2013);
        assert_eq!(detect_version(b"AC1032xxxx").unwrap(), DwgVersion::R2018);
    }
}
