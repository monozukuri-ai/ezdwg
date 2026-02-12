use crate::container::{SectionDirectory, SectionSlice};
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::dwg::r2000;
use crate::dwg::r2004;
use crate::dwg::r2007;
use crate::dwg::version::{detect_version, DwgVersion};
use crate::objects::{ObjectIndex, ObjectRecord};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Decoder<'a> {
    bytes: &'a [u8],
    version: DwgVersion,
    config: ParseConfig,
}

impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8], config: ParseConfig) -> Result<Self> {
        let version = detect_version(bytes)?;
        Ok(Self {
            bytes,
            version,
            config,
        })
    }

    pub fn version(&self) -> &DwgVersion {
        &self.version
    }

    pub fn ensure_supported(&self) -> Result<()> {
        match self.version {
            DwgVersion::R2000
            | DwgVersion::R2004
            | DwgVersion::R2007
            | DwgVersion::R2010
            | DwgVersion::R2013
            | DwgVersion::R2018 => Ok(()),
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }

    pub fn section_directory(&self) -> Result<SectionDirectory> {
        match self.version {
            DwgVersion::R2000 => r2000::parse_section_directory(self.bytes, &self.config),
            DwgVersion::R2004 | DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018 => {
                r2004::parse_section_directory(self.bytes, &self.config)
            }
            DwgVersion::R2007 => r2007::parse_section_directory(self.bytes, &self.config),
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }

    pub fn load_section_by_index(
        &self,
        directory: &SectionDirectory,
        index: usize,
    ) -> Result<SectionSlice<'a>> {
        match self.version {
            DwgVersion::R2000 => {
                r2000::load_section_by_index(self.bytes, directory, index, &self.config)
            }
            DwgVersion::R2004 | DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018 => {
                r2004::load_section_by_index(self.bytes, directory, index, &self.config)
            }
            DwgVersion::R2007 => {
                r2007::load_section_by_index(self.bytes, directory, index, &self.config)
            }
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }

    pub fn build_object_index(&self) -> Result<ObjectIndex> {
        match self.version {
            DwgVersion::R2000 => r2000::build_object_index(self.bytes, &self.config),
            DwgVersion::R2004 | DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018 => {
                r2004::build_object_index(self.bytes, &self.config)
            }
            DwgVersion::R2007 => r2007::build_object_index(self.bytes, &self.config),
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }

    pub fn parse_object_record(&self, offset: u32) -> Result<ObjectRecord<'a>> {
        match self.version {
            DwgVersion::R2000 => r2000::parse_object_record(self.bytes, offset),
            DwgVersion::R2004 | DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018 => {
                r2004::parse_object_record(self.bytes, offset, &self.config)
            }
            DwgVersion::R2007 => r2007::parse_object_record(self.bytes, offset, &self.config),
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }

    pub fn dynamic_type_map(&self) -> Result<HashMap<u16, String>> {
        match self.version {
            DwgVersion::R2000 => Ok(HashMap::new()),
            DwgVersion::R2004 => r2004::load_dynamic_type_map(self.bytes, &self.config),
            DwgVersion::R2010 => Ok(HashMap::new()),
            DwgVersion::R2007 => r2007::load_dynamic_type_map(self.bytes, &self.config),
            DwgVersion::R2013 | DwgVersion::R2018 => Ok(HashMap::new()),
            DwgVersion::Unknown(_) => Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("unsupported DWG version: {}", self.version.as_str()),
            )),
        }
    }
}
