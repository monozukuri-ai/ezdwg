use crate::bit::{BitReader, Endian};
use crate::container::section_directory;
use crate::container::section_loader;
use crate::container::SectionKind;
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects;
use crate::objects::{ObjectIndex, ObjectRecord};
use crate::{container::SectionDirectory, container::SectionSlice};
use std::collections::HashMap;

const SENTINEL_CLASSES_BEFORE: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const SENTINEL_CLASSES_AFTER: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

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

pub fn load_dynamic_type_map(bytes: &[u8], config: &ParseConfig) -> Result<HashMap<u16, String>> {
    let directory = parse_section_directory(bytes, config)?;
    let classes_index = directory
        .records
        .iter()
        .position(|record| record.kind() == SectionKind::Classes)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section not found: AcDb:Classes"))?;
    let section = load_section_by_index(bytes, &directory, classes_index, config)?;
    let classes = parse_classes_section_r13_r15(&section.data)?;

    let mut map = HashMap::with_capacity(classes.len());
    let has_explicit_codes = classes.iter().any(|entry| entry.class_number >= 500);

    for (idx, class) in classes.iter().enumerate() {
        let code = if has_explicit_codes {
            class.class_number as usize
        } else {
            500usize + idx
        };
        if code > u16::MAX as usize || class.dxf_name.is_empty() {
            continue;
        }
        map.insert(code as u16, class.dxf_name.to_ascii_uppercase());
    }

    if std::env::var("EZDWG_DEBUG_R2000_CLASSES")
        .ok()
        .is_some_and(|value| value != "0")
    {
        eprintln!(
            "[r2000-classes] parsed_entries={} mapped_entries={}",
            classes.len(),
            map.len()
        );
        for (idx, class) in classes.iter().take(48).enumerate() {
            eprintln!(
                "[r2000-classes] idx={} class_number={} dxf_name={}",
                idx, class.class_number, class.dxf_name
            );
        }
    }

    Ok(map)
}

#[derive(Debug, Clone)]
struct ClassEntry {
    class_number: u16,
    dxf_name: String,
}

fn parse_classes_section_r13_r15(data: &[u8]) -> Result<Vec<ClassEntry>> {
    let mut reader = BitReader::new(data);

    let sentinel_before = reader.read_rcs(SENTINEL_CLASSES_BEFORE.len())?;
    if sentinel_before.as_slice() != SENTINEL_CLASSES_BEFORE {
        return Err(DwgError::new(
            ErrorKind::Format,
            "AcDb:Classes sentinel(before) mismatch",
        ));
    }

    let class_data_size_bytes = reader.read_rl(Endian::Little)? as u64;
    let class_data_start = reader.tell_bits();
    let class_data_end = class_data_start.saturating_add(class_data_size_bytes.saturating_mul(8));

    let mut classes = Vec::new();
    while reader.tell_bits() < class_data_end {
        let class_entry = (|| -> Result<ClassEntry> {
            let class_number = reader.read_bs()?;
            let _proxy_flags_or_version = reader.read_bs()?;
            let _app_name = reader.read_tv()?;
            let _cpp_name = reader.read_tv()?;
            let dxf_name = reader.read_tv()?;
            let _was_a_zombie = reader.read_b()?;
            let _item_class_id = reader.read_bs()?;
            Ok(ClassEntry {
                class_number,
                dxf_name,
            })
        })();

        match class_entry {
            Ok(entry) => classes.push(entry),
            Err(err)
                if matches!(
                    err.kind,
                    ErrorKind::Io | ErrorKind::Format | ErrorKind::Decode
                ) =>
            {
                break;
            }
            Err(err) => return Err(err),
        }
    }

    if let Ok(class_data_end_u32) = u32::try_from(class_data_end) {
        reader.set_bit_pos(class_data_end_u32);
        let _ = reader.read_crc();
        if let Ok(sentinel_after) = reader.read_rcs(SENTINEL_CLASSES_AFTER.len()) {
            if sentinel_after.as_slice() != SENTINEL_CLASSES_AFTER {
                // Keep best-effort parsed classes when trailing markers are not readable.
            }
        }
    }

    Ok(classes)
}
