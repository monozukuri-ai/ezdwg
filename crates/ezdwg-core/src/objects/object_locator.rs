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
            let should_replace = match by_handle.get(&obj.handle).copied() {
                Some(prev_idx) => {
                    let prev_offset = objects
                        .get(prev_idx)
                        .map(|prev: &ObjectRef| prev.offset)
                        .unwrap_or(0);
                    obj.offset > prev_offset
                }
                None => true,
            };
            if should_replace {
                by_handle.insert(obj.handle, idx);
            }
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
        .find(|record| record.kind() == SectionKind::ObjectMap)
        .cloned()
}

fn parse_object_map(bytes: &[u8], _config: &ParseConfig) -> Result<ObjectIndex> {
    let mut reader = ByteReader::new(bytes);
    let mut objects = Vec::new();

    let mut last_handle: i64 = 0;
    let mut last_offset: i64 = 0;
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
        if !_config.strict {
            last_handle = 0;
            last_offset = 0;
        }

        while (reader.tell() - start) < (section_size as u64 - 2) {
            let delta_handle = read_modular_char(&mut reader)?;
            let delta_offset = read_modular_char(&mut reader)?;
            last_handle = last_handle.checked_add(delta_handle).ok_or_else(|| {
                DwgError::new(ErrorKind::Format, "object map handle overflow")
                    .with_offset(reader.tell())
            })?;
            last_offset = last_offset.checked_add(delta_offset).ok_or_else(|| {
                DwgError::new(ErrorKind::Format, "object map offset overflow")
                    .with_offset(reader.tell())
            })?;
            if last_handle < 0 || last_offset < 0 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "object map contains negative handle/offset",
                )
                .with_offset(reader.tell()));
            }
            if last_offset > u32::MAX as i64 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "object map offset exceeds u32 range",
                )
                .with_offset(reader.tell()));
            }
            objects.push(ObjectRef {
                handle: Handle(last_handle as u64),
                offset: last_offset as u32,
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

fn read_modular_char(reader: &mut ByteReader<'_>) -> Result<i64> {
    let mut value: i64 = 0;
    let mut shift = 0;

    for _ in 0..4 {
        let mut byte = reader.read_u8()?;
        if (byte & 0x80) == 0 {
            let negative = (byte & 0x40) != 0;
            if negative {
                byte &= 0xBF;
            }
            value |= (byte as i64) << shift;
            if negative {
                value = -value;
            }
            return Ok(value);
        }
        byte &= 0x7F;
        value |= (byte as i64) << shift;
        shift += 7;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{parse_object_map, ObjectIndex};
    use crate::core::config::ParseConfig;
    use crate::objects::{Handle, ObjectRef};

    #[test]
    fn object_index_prefers_highest_offset_for_duplicate_handles() {
        let index = ObjectIndex::from_objects(vec![
            ObjectRef {
                handle: Handle(237),
                offset: 527_255,
            },
            ObjectRef {
                handle: Handle(237),
                offset: 12_157,
            },
            ObjectRef {
                handle: Handle(237),
                offset: 204_892,
            },
        ]);

        let resolved = index.get(Handle(237)).expect("duplicate-handle entry");
        assert_eq!(resolved.offset, 527_255);
    }

    #[test]
    fn parse_multiblock_object_map_keeps_running_deltas() {
        let bytes = vec![
            0x00, 0x06, // block 1: 2-byte header + 4-byte payload
            0x01, 0x0A, // +1, +10
            0x02, 0x04, // +2, +4
            0x00, 0x00, // crc
            0x00, 0x06, // block 2: continue from previous handle/offset
            0x07, 0x08, // +7, +8
            0x02, 0x03, // +2, +3
            0x00, 0x00, // crc
            0x00, 0x02, // terminator block
        ];
        let mut config = ParseConfig::default();
        config.strict = true;
        let index = parse_object_map(&bytes, &config).expect("index");
        let refs: Vec<(u64, u32)> = index
            .objects
            .iter()
            .map(|obj| (obj.handle.0, obj.offset))
            .collect();
        assert_eq!(refs, vec![(1, 10), (3, 14), (10, 22), (12, 25)]);
    }
}
