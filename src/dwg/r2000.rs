use crate::container::section_directory;
use crate::container::section_loader;
use crate::core::config::ParseConfig;
use crate::core::result::Result;
use crate::objects;
use crate::objects::{ObjectIndex, ObjectRecord};
use crate::{container::SectionDirectory, container::SectionSlice};

pub fn parse_section_directory(bytes: &[u8], config: &ParseConfig) -> Result<SectionDirectory> {
    section_directory::parse_with_config(bytes, config)
}

pub fn load_section_by_index<'a>(
    bytes: &'a [u8],
    directory: &SectionDirectory,
    index: usize,
    config: &ParseConfig,
) -> Result<SectionSlice<'a>> {
    section_loader::load_section_by_index(bytes, directory, index, config)
}

pub fn build_object_index(bytes: &[u8], config: &ParseConfig) -> Result<ObjectIndex> {
    objects::build_object_index(bytes, config)
}

pub fn parse_object_record<'a>(bytes: &'a [u8], offset: u32) -> Result<ObjectRecord<'a>> {
    objects::parse_object_record(bytes, offset)
}
