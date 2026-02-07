use std::collections::HashMap;

use crate::container::section_directory::{SectionDirectory, SectionKind, SectionLocatorRecord};
use crate::container::section_loader;
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::io::ByteReader;
use crate::objects::{Handle, ObjectRef};

#[derive(Debug, Clone)]
pub struct ObjectIndex {
    pub objects: Vec<ObjectRef>,
    by_handle: HashMap<Handle, usize>,
}

impl ObjectIndex {
    pub fn get(&self, handle: Handle) -> Option<&ObjectRef> {
        self.by_handle.get(&handle).map(|idx| &self.objects[*idx])
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn from_objects(objects: Vec<ObjectRef>) -> Self {
        let mut by_handle = HashMap::with_capacity(objects.len());
        for (idx, obj) in objects.iter().enumerate() {
            by_handle.insert(obj.handle, idx);
        }
        Self { objects, by_handle }
    }
}

pub fn build_object_index(bytes: &[u8], config: &ParseConfig) -> Result<ObjectIndex> {
    let directory = crate::container::section_directory::parse_with_config(bytes, config)?;
    build_object_index_from_directory(bytes, &directory, config)
}

pub fn build_object_index_from_directory(
    bytes: &[u8],
    directory: &SectionDirectory,
    config: &ParseConfig,
) -> Result<ObjectIndex> {
    let record = find_object_map_record(directory).ok_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "object map section not found in section directory",
        )
    })?;
    let section = section_loader::load_section(bytes, record, config)?;
    parse_object_map(section.data.as_ref(), config)
}

fn find_object_map_record(directory: &SectionDirectory) -> Option<SectionLocatorRecord> {
    directory
        .records
        .iter()
        .cloned()
        .find(|record| record.kind() == SectionKind::ObjectMap)
}

fn parse_object_map(bytes: &[u8], _config: &ParseConfig) -> Result<ObjectIndex> {
    let mut reader = ByteReader::new(bytes);
    let mut objects = Vec::new();

    loop {
        if reader.remaining() < 2 {
            break;
        }
        let section_size = read_u16_be(&mut reader)? as usize;
        if section_size == 2 {
            break;
        }
        if section_size < 2 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("invalid object map block size {section_size}"),
            ));
        }
        if reader.remaining() < section_size - 2 {
            return Err(DwgError::new(
                ErrorKind::Format,
                "object map block exceeds remaining data",
            )
            .with_offset(reader.tell()));
        }

        let start = reader.tell();
        let mut last_handle: u64 = 0;
        let mut last_offset: u64 = 0;

        while (reader.tell() - start) < (section_size as u64 - 2) {
            let delta_handle = read_modular_char_unsigned(&mut reader)?;
            let delta_offset = read_modular_char_unsigned(&mut reader)?;
            last_handle = last_handle.checked_add(delta_handle).ok_or_else(|| {
                DwgError::new(ErrorKind::Format, "object map handle overflow")
                    .with_offset(reader.tell())
            })?;
            last_offset = last_offset.checked_add(delta_offset).ok_or_else(|| {
                DwgError::new(ErrorKind::Format, "object map offset overflow")
                    .with_offset(reader.tell())
            })?;
            let offset = u32::try_from(last_offset).map_err(|_| {
                DwgError::new(ErrorKind::Format, "object map offset exceeds u32 range")
                    .with_offset(reader.tell())
            })?;
            objects.push(ObjectRef {
                handle: Handle(last_handle),
                offset,
            });
        }

        // CRC (big-endian) - currently ignored
        if reader.remaining() < 2 {
            break;
        }
        let _crc = read_u16_be(&mut reader)?;
    }

    Ok(ObjectIndex::from_objects(objects))
}

fn read_u16_be(reader: &mut ByteReader<'_>) -> Result<u16> {
    let hi = reader.read_u8()? as u16;
    let lo = reader.read_u8()? as u16;
    Ok((hi << 8) | lo)
}

fn read_modular_char_unsigned(reader: &mut ByteReader<'_>) -> Result<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;

    for _ in 0..5 {
        let byte = reader.read_u8()?;
        value |= u64::from(byte & 0x7F) << shift;
        if (byte & 0x80) == 0 {
            return Ok(value);
        }
        shift += 7;
    }

    Ok(value)
}
