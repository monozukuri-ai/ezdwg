use std::collections::HashMap;

use crate::bit::{BitReader, Endian};
use crate::container::{SectionDirectory, SectionLocatorRecord, SectionSlice};
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::io::ByteReader;
use crate::objects::{Handle, ObjectIndex, ObjectRecord, ObjectRef};

const STREAM_BASE_OFFSET: u64 = 0x480;
const SECOND_HEADER_OFFSET: usize = 0x80;
const SECOND_HEADER_RS_SIZE: usize = 0x3D8;
const SECOND_HEADER_PAYLOAD_OFFSET: usize = 0x20;
const SECOND_HEADER_BODY_SIZE: usize = 0x110;

const SYSTEM_PAGE_RS_DATA_SIZE: u64 = 239;
const SYSTEM_PAGE_RS_CODEWORD_SIZE: u64 = 255;
const SYSTEM_PAGE_CRC_BLOCK_SIZE: u64 = 8;
const SYSTEM_PAGE_ALIGN_SIZE: u64 = 0x20;

const SECTION_ENTRY_SIZE: usize = 8 * 8;
const SECTION_PAGE_INFO_SIZE: usize = 7 * 8;
const SENTINEL_CLASSES_BEFORE: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const SENTINEL_CLASSES_AFTER: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

#[derive(Debug, Clone)]
struct ClassEntry {
    dxf_name: String,
}

#[derive(Debug, Clone)]
struct HeaderData {
    pages_map_offset: u64,
    pages_map_size_compressed: u64,
    pages_map_size_uncompressed: u64,
    pages_map_correction_factor: u64,
    sections_map_id: u64,
    sections_map_size_compressed: u64,
    sections_map_size_uncompressed: u64,
    sections_map_correction_factor: u64,
    sections_amount: u64,
}

#[derive(Debug, Clone)]
struct PageMapEntry {
    id: i64,
    size: u64,
    address: u64,
}

#[derive(Debug, Clone)]
struct SectionEntry {
    size: u64,
    encoded: u64,
    name: String,
    pages: Vec<SectionPageInfo>,
}

#[derive(Debug, Clone)]
struct SectionPageInfo {
    offset: u64,
    id: u64,
    size_uncompressed: u64,
    size_compressed: u64,
}

#[derive(Debug, Clone)]
struct ContainerMetadata {
    page_map: Vec<PageMapEntry>,
    sections: Vec<SectionEntry>,
}

pub fn parse_section_directory(bytes: &[u8], _config: &ParseConfig) -> Result<SectionDirectory> {
    let metadata = parse_container_metadata(bytes)?;
    let mut records = Vec::with_capacity(metadata.sections.len());

    for section in metadata.sections {
        let record_no = record_no_for_name(&section.name);
        let offset = section
            .pages
            .first()
            .and_then(|page| {
                metadata
                    .page_map
                    .iter()
                    .find(|entry| entry.id == page.id as i64)
                    .map(|entry| entry.address)
            })
            .unwrap_or(0);

        records.push(SectionLocatorRecord {
            record_no,
            offset: offset.min(u32::MAX as u64) as u32,
            size: section.size.min(u32::MAX as u64) as u32,
            name: Some(section.name),
        });
    }

    Ok(SectionDirectory {
        record_count: records.len() as u32,
        records,
        crc: 0,
        sentinel_ok: true,
    })
}

pub fn load_section_by_index<'a>(
    bytes: &'a [u8],
    directory: &SectionDirectory,
    index: usize,
    config: &ParseConfig,
) -> Result<SectionSlice<'a>> {
    let metadata = parse_container_metadata(bytes)?;
    let section = metadata
        .sections
        .get(index)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section index out of range"))?;
    let data = load_section_data(bytes, section, &metadata.page_map, config)?;

    let record = directory
        .records
        .get(index)
        .cloned()
        .unwrap_or_else(|| SectionLocatorRecord {
            record_no: record_no_for_name(&section.name),
            offset: section
                .pages
                .first()
                .and_then(|page| {
                    metadata
                        .page_map
                        .iter()
                        .find(|entry| entry.id == page.id as i64)
                        .map(|entry| entry.address as u32)
                })
                .unwrap_or(0),
            size: section.size.min(u32::MAX as u64) as u32,
            name: Some(section.name.clone()),
        });

    Ok(SectionSlice {
        record,
        data: std::borrow::Cow::Owned(data),
    })
}

pub fn build_object_index(bytes: &[u8], config: &ParseConfig) -> Result<ObjectIndex> {
    let handles_data = load_named_section_data(bytes, config, "AcDb:Handles")?;
    let objects_data = load_named_section_data(bytes, config, "AcDb:AcDbObjects")?;
    let index = parse_object_map_handles(&handles_data, config)?;

    let mut valid_objects = Vec::with_capacity(index.objects.len());
    for object in index.objects {
        if crate::objects::object_record::parse_object_record_owned(&objects_data, object.offset)
            .is_ok()
        {
            valid_objects.push(object);
        }
    }

    Ok(ObjectIndex::from_objects(valid_objects))
}

pub fn parse_object_record<'a>(
    bytes: &'a [u8],
    offset: u32,
    config: &ParseConfig,
) -> Result<ObjectRecord<'a>> {
    let data = load_named_section_data(bytes, config, "AcDb:AcDbObjects")?;
    crate::objects::object_record::parse_object_record_owned(&data, offset)
}

pub fn load_dynamic_type_map(bytes: &[u8], config: &ParseConfig) -> Result<HashMap<u16, String>> {
    let data = load_named_section_data(bytes, config, "AcDb:Classes")?;
    let classes = parse_classes_section(&data)?;
    let mut map = HashMap::with_capacity(classes.len());
    for (idx, class) in classes.iter().enumerate() {
        let code = 500usize + idx;
        if code > u16::MAX as usize {
            break;
        }
        if !class.dxf_name.is_empty() {
            map.insert(code as u16, class.dxf_name.to_ascii_uppercase());
        }
    }
    Ok(map)
}

fn parse_container_metadata(bytes: &[u8]) -> Result<ContainerMetadata> {
    let header = read_header_data(bytes)?;
    let page_map = read_page_map(bytes, &header)?;
    let sections = read_section_map(bytes, &header, &page_map)?;
    Ok(ContainerMetadata { page_map, sections })
}

fn load_named_section_data(bytes: &[u8], config: &ParseConfig, name: &str) -> Result<Vec<u8>> {
    let metadata = parse_container_metadata(bytes)?;
    let section = metadata
        .sections
        .iter()
        .find(|section| section.name == name)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, format!("section not found: {name}")))?;
    load_section_data(bytes, section, &metadata.page_map, config)
}

fn load_section_data(
    bytes: &[u8],
    section: &SectionEntry,
    page_map: &[PageMapEntry],
    config: &ParseConfig,
) -> Result<Vec<u8>> {
    if section.size > config.max_section_bytes {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section size {} exceeds limit {}",
                section.size, config.max_section_bytes
            ),
        ));
    }

    let total_size = to_usize(section.size, "R2007 section size")?;
    let mut output = vec![0u8; total_size];
    if total_size == 0 {
        return Ok(output);
    }

    for page in &section.pages {
        let entry = page_map
            .iter()
            .find(|entry| entry.id == page.id as i64)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 section page not found"))?;

        let page_data = read_data_page(
            bytes,
            entry,
            section.encoded,
            page.size_compressed,
            page.size_uncompressed,
        )?;

        let start = to_usize(page.offset, "R2007 section page offset")?;
        if start >= output.len() {
            continue;
        }
        let end = (start + page_data.len()).min(output.len());
        output[start..end].copy_from_slice(&page_data[..end - start]);
    }

    Ok(output)
}

fn read_data_page(
    bytes: &[u8],
    page_entry: &PageMapEntry,
    encoded: u64,
    size_compressed: u64,
    size_uncompressed: u64,
) -> Result<Vec<u8>> {
    const DATA_PAGE_RS_DATA_SIZE: u64 = 251;

    let block_count_u64 = div_ceil(size_compressed, DATA_PAGE_RS_DATA_SIZE);
    let min_page_size = DATA_PAGE_RS_DATA_SIZE
        .checked_mul(block_count_u64)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 data page size overflow"))?;
    let read_size = page_entry.size.max(min_page_size);

    let address = to_usize(page_entry.address, "R2007 data page address")?;
    let read_size = to_usize(read_size, "R2007 data page size")?;
    let end = address
        .checked_add(read_size)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 data page range overflow"))?;
    if end > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 data page out of file range",
        ));
    }

    let page_buf = &bytes[address..end];
    let block_count = to_usize(block_count_u64, "R2007 data page RS block count")?;
    let encoded_method = u8::try_from(encoded)
        .map_err(|_| DwgError::new(ErrorKind::Format, "R2007 encoded flag exceeds u8"))?;
    let decoded = match encoded_method {
        0 => page_buf.to_vec(),
        1 | 4 => decode_reed_solomon(page_buf, 251, block_count, encoded_method)?,
        _ => {
            return Err(DwgError::not_implemented(
                "unsupported R2007 data page encoding method",
            ))
        }
    };

    if size_compressed < size_uncompressed {
        let compressed_size = to_usize(size_compressed, "R2007 compressed data page size")?;
        let uncompressed_size = to_usize(size_uncompressed, "R2007 uncompressed data page size")?;
        if compressed_size > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 compressed data page exceeds decoded buffer",
            ));
        }
        decompress_r21(&decoded[..compressed_size], uncompressed_size)
    } else {
        let size = to_usize(size_uncompressed, "R2007 data page size")?;
        if size > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 data page exceeds decoded buffer",
            ));
        }
        Ok(decoded[..size].to_vec())
    }
}

fn parse_classes_section(data: &[u8]) -> Result<Vec<ClassEntry>> {
    let mut reader = BitReader::new(data);

    let sentinel_before = reader.read_rcs(SENTINEL_CLASSES_BEFORE.len())?;
    if sentinel_before.as_slice() != SENTINEL_CLASSES_BEFORE {
        return Err(DwgError::new(
            ErrorKind::Format,
            "AcDb:Classes sentinel(before) mismatch",
        ));
    }

    let size = reader.read_rl(Endian::Little)? as usize;
    let end_bit = reader.read_rl(Endian::Little)?;
    let max_class_number = reader.read_bs()?;
    let _zero0 = reader.read_rc()?;
    let _zero1 = reader.read_rc()?;
    let _bit_flag = reader.read_b()?;

    let (saved_byte, saved_bit) = reader.get_pos();
    let string_stream_present = if end_bit > 0 {
        let base_offset_bits = 20u32 * 8;
        reader.set_bit_pos(base_offset_bits + end_bit - 1);
        let flag = reader.read_b()? != 0;
        reader.set_pos(saved_byte, saved_bit);
        flag
    } else {
        false
    };

    let mut classes = Vec::new();
    while reader.get_pos().0 <= size {
        let class_number = reader.read_bs()?;
        let _proxy_flags = reader.read_bs()?;
        let dxf_name = String::new();
        let _was_a_zombie = reader.read_b()?;
        let _item_class_id = reader.read_bs()?;
        let _number_of_objects = reader.read_bl()?;
        let _dwg_version = reader.read_bl()?;
        let _maintenance_version = reader.read_bl()?;
        let _unknown0 = reader.read_bl()?;
        let _unknown1 = reader.read_bl()?;

        classes.push(ClassEntry { dxf_name });

        if class_number == max_class_number {
            break;
        }
    }

    if string_stream_present {
        for class in &mut classes {
            let _app_name = read_tu(&mut reader)?;
            let _cpp_name = read_tu(&mut reader)?;
            class.dxf_name = read_tu(&mut reader)?;
        }
        let base_offset_bits = 20u32 * 8;
        reader.set_bit_pos(base_offset_bits + end_bit);
    }

    let _crc = reader.read_crc()?;
    let sentinel_after = reader.read_rcs(SENTINEL_CLASSES_AFTER.len())?;
    if sentinel_after.as_slice() != SENTINEL_CLASSES_AFTER {
        return Err(DwgError::new(
            ErrorKind::Format,
            "AcDb:Classes sentinel(after) mismatch",
        ));
    }

    Ok(classes)
}

fn read_tu(reader: &mut BitReader<'_>) -> Result<String> {
    let length = reader.read_bs()? as usize;
    let mut units = Vec::with_capacity(length);
    for _ in 0..length {
        units.push(reader.read_rs(Endian::Little)?);
    }
    Ok(String::from_utf16_lossy(&units))
}

fn parse_object_map_handles(bytes: &[u8], config: &ParseConfig) -> Result<ObjectIndex> {
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
                format!("invalid AcDb:Handles block size {section_size}"),
            ));
        }
        if reader.remaining() < section_size - 2 {
            return Err(DwgError::new(
                ErrorKind::Format,
                "AcDb:Handles block exceeds remaining bytes",
            )
            .with_offset(reader.tell()));
        }

        let start = reader.tell();
        let mut last_handle: i64 = 0;
        let mut last_offset: i64 = 0;

        while (reader.tell() - start) < (section_size as u64 - 2) {
            last_handle += read_modular_char(&mut reader)?;
            last_offset += read_modular_char(&mut reader)?;

            if last_handle < 0 || last_offset < 0 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "AcDb:Handles contains negative handle or offset",
                )
                .with_offset(reader.tell()));
            }
            if last_offset > u32::MAX as i64 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "AcDb:Handles offset exceeds u32 range",
                )
                .with_offset(reader.tell()));
            }

            objects.push(ObjectRef {
                handle: Handle(last_handle as u64),
                offset: last_offset as u32,
            });

            if objects.len() as u32 > config.max_objects {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    format!("object count exceeds limit {}", config.max_objects),
                ));
            }
        }

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

fn read_header_data(bytes: &[u8]) -> Result<HeaderData> {
    if bytes.len() < SECOND_HEADER_OFFSET + SECOND_HEADER_RS_SIZE {
        return Err(DwgError::new(
            ErrorKind::Format,
            "file too small for R2007 second header",
        ));
    }

    let encoded = &bytes[SECOND_HEADER_OFFSET..SECOND_HEADER_OFFSET + SECOND_HEADER_RS_SIZE];
    let decoded = decode_reed_solomon(encoded, 239, 3, 4)?;
    if decoded.len() < SECOND_HEADER_PAYLOAD_OFFSET {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 second header decode is truncated",
        ));
    }

    let mut head_reader = ByteReader::new(&decoded);
    let _crc = head_reader.read_u64_le()?;
    let _key = head_reader.read_u64_le()?;
    let _compressed_data_crc = head_reader.read_u64_le()?;
    let compressed_size = head_reader.read_i32_le()?;
    let _length2 = head_reader.read_i32_le()?;

    let body = if compressed_size < 0 {
        let size = compressed_size.unsigned_abs() as usize;
        let end = SECOND_HEADER_PAYLOAD_OFFSET
            .checked_add(size)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "second header size overflow"))?;
        if end > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 second header body out of range",
            ));
        }
        decoded[SECOND_HEADER_PAYLOAD_OFFSET..end].to_vec()
    } else if compressed_size > 0 {
        let compressed_size = compressed_size as usize;
        let end = SECOND_HEADER_PAYLOAD_OFFSET
            .checked_add(compressed_size)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "second header size overflow"))?;
        if end > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 compressed second header body out of range",
            ));
        }
        decompress_r21(
            &decoded[SECOND_HEADER_PAYLOAD_OFFSET..end],
            SECOND_HEADER_BODY_SIZE,
        )?
    } else {
        return Err(DwgError::new(
            ErrorKind::Format,
            "invalid R2007 second header compressed size: 0",
        ));
    };

    if body.len() < SECOND_HEADER_BODY_SIZE {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 second header body is truncated",
        ));
    }

    let mut body_reader = ByteReader::new(&body[..SECOND_HEADER_BODY_SIZE]);
    let mut fields = Vec::with_capacity(34);
    for _ in 0..34 {
        fields.push(body_reader.read_u64_le()?);
    }

    Ok(HeaderData {
        pages_map_offset: fields[7],
        pages_map_size_compressed: fields[10],
        pages_map_size_uncompressed: fields[11],
        pages_map_correction_factor: fields[3],
        sections_map_id: fields[24],
        sections_map_size_compressed: fields[22],
        sections_map_size_uncompressed: fields[25],
        sections_map_correction_factor: fields[27],
        sections_amount: fields[20],
    })
}

fn read_page_map(bytes: &[u8], header: &HeaderData) -> Result<Vec<PageMapEntry>> {
    let address = STREAM_BASE_OFFSET
        .checked_add(header.pages_map_offset)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 page map address overflow"))?;
    let data = read_system_page(
        bytes,
        address,
        header.pages_map_size_compressed,
        header.pages_map_size_uncompressed,
        header.pages_map_correction_factor,
    )?;

    let mut reader = ByteReader::new(&data);
    let mut entries = Vec::new();
    let mut current_address = STREAM_BASE_OFFSET;
    while reader.remaining() >= 16 {
        let size = reader.read_u64_le()? as i64;
        let id = reader.read_u64_le()? as i64;
        if size == 0 && id == 0 {
            break;
        }
        if size <= 0 {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 page map entry has invalid size",
            ));
        }

        entries.push(PageMapEntry {
            id,
            size: size as u64,
            address: current_address,
        });
        current_address = current_address
            .checked_add(size as u64)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 page map address overflow"))?;
    }

    if entries.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 page map has no entries",
        ));
    }
    Ok(entries)
}

fn read_section_map(
    bytes: &[u8],
    header: &HeaderData,
    page_map: &[PageMapEntry],
) -> Result<Vec<SectionEntry>> {
    let section_map_page = page_map
        .iter()
        .find(|entry| entry.id == header.sections_map_id as i64)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 section map page not found"))?;

    let data = read_system_page(
        bytes,
        section_map_page.address,
        header.sections_map_size_compressed,
        header.sections_map_size_uncompressed,
        header.sections_map_correction_factor,
    )?;

    let mut reader = ByteReader::new(&data);
    let mut sections = Vec::new();
    let max_sections = if header.sections_amount > 0 {
        header.sections_amount.saturating_sub(1) as usize
    } else {
        usize::MAX
    };

    while reader.remaining() >= SECTION_ENTRY_SIZE && sections.len() < max_sections {
        let size = reader.read_u64_le()?;
        let _max_size = reader.read_u64_le()?;
        let encrypted = reader.read_u64_le()?;
        let _hash_code = reader.read_u64_le()?;
        let name_length = reader.read_u64_le()?;
        let _unknown = reader.read_u64_le()?;
        let encoded = reader.read_u64_le()?;
        let page_count = reader.read_u64_le()?;

        if size == 0 && page_count == 0 && name_length == 0 {
            break;
        }

        if encrypted == 1 {
            return Err(DwgError::not_implemented(
                "encrypted R2007 sections are not supported",
            ));
        }

        let name_length = to_usize(name_length, "R2007 section name length")?;
        if reader.remaining() < name_length {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 section name exceeds section map bounds",
            ));
        }
        let name = decode_utf16_string(reader.read_bytes(name_length)?)?;

        let page_count = to_usize(page_count, "R2007 section page count")?;
        let mut pages = Vec::with_capacity(page_count);
        for _ in 0..page_count {
            if reader.remaining() < SECTION_PAGE_INFO_SIZE {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "R2007 section page info is truncated",
                ));
            }
            let offset = reader.read_u64_le()?;
            let _size = reader.read_u64_le()?;
            let id = reader.read_u64_le()?;
            let size_uncompressed = reader.read_u64_le()?;
            let size_compressed = reader.read_u64_le()?;
            let _checksum = reader.read_u64_le()?;
            let _crc = reader.read_u64_le()?;
            pages.push(SectionPageInfo {
                offset,
                id,
                size_uncompressed,
                size_compressed,
            });
        }

        sections.push(SectionEntry {
            size,
            encoded,
            name,
            pages,
        });
    }

    if sections.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 section map has no entries",
        ));
    }
    Ok(sections)
}

fn read_system_page(
    bytes: &[u8],
    address: u64,
    size_compressed: u64,
    size_uncompressed: u64,
    correction_factor: u64,
) -> Result<Vec<u8>> {
    let compressed_padded = align_up(size_compressed, SYSTEM_PAGE_CRC_BLOCK_SIZE)?;
    let rs_pre_encoded_size = compressed_padded
        .checked_mul(correction_factor)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 system page size overflow"))?;
    let block_count = div_ceil(rs_pre_encoded_size, SYSTEM_PAGE_RS_DATA_SIZE);
    let page_size = align_up(
        block_count
            .checked_mul(SYSTEM_PAGE_RS_CODEWORD_SIZE)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 system page size overflow"))?,
        SYSTEM_PAGE_ALIGN_SIZE,
    )?;

    let address = to_usize(address, "R2007 system page address")?;
    let page_size = to_usize(page_size, "R2007 system page size")?;
    let end = address
        .checked_add(page_size)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "R2007 system page range overflow"))?;
    if end > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 system page out of file range",
        ));
    }

    let block_count = to_usize(block_count, "R2007 RS block count")?;
    let decoded = decode_reed_solomon(&bytes[address..end], 239, block_count, 4)?;

    if size_compressed < size_uncompressed {
        let compressed_size = to_usize(size_compressed, "R2007 compressed page size")?;
        let uncompressed_size = to_usize(size_uncompressed, "R2007 uncompressed page size")?;
        if compressed_size > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 compressed system page data out of range",
            ));
        }
        decompress_r21(&decoded[..compressed_size], uncompressed_size)
    } else {
        let size = to_usize(size_uncompressed, "R2007 page size")?;
        if size > decoded.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "R2007 system page data out of range",
            ));
        }
        Ok(decoded[..size].to_vec())
    }
}

fn decode_reed_solomon(src: &[u8], k: usize, block_count: usize, method: u8) -> Result<Vec<u8>> {
    let output_size = k
        .checked_mul(block_count)
        .ok_or_else(|| DwgError::new(ErrorKind::Decode, "RS output size overflow"))?;
    if output_size == 0 {
        return Ok(Vec::new());
    }
    if src.len() < output_size {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "R2007 RS input is smaller than required output layout",
        ));
    }

    match method {
        4 => {
            let mut out = vec![0u8; output_size];
            let mut dst = 0usize;
            for bc in 0..block_count {
                for idx in 0..k {
                    out[dst] = src[block_count * idx + bc];
                    dst += 1;
                }
            }
            Ok(out)
        }
        1 => Ok(src[..output_size].to_vec()),
        _ => Err(DwgError::not_implemented(
            "unsupported Reed-Solomon method for R2007",
        )),
    }
}

fn decompress_r21(src: &[u8], dst_size: usize) -> Result<Vec<u8>> {
    if dst_size == 0 {
        return Ok(Vec::new());
    }
    if src.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "R2007 compressed stream is empty",
        ));
    }

    let mut dst = vec![0u8; dst_size];
    let src_size = src.len();

    let mut src_idx = 0usize;
    let mut dst_idx = 0usize;
    let mut length = 0usize;

    let mut opcode = read_u8(src, &mut src_idx)? as usize;
    if (opcode & 0xF0) == 0x20 {
        src_idx = src_idx
            .checked_add(2)
            .ok_or_else(|| DwgError::new(ErrorKind::Decode, "R2007 opcode index overflow"))?;
        if src_idx >= src_size {
            return Err(DwgError::new(
                ErrorKind::Decode,
                "R2007 opcode bootstrap exceeds input",
            ));
        }
        length = (src[src_idx] & 0x07) as usize;
        src_idx += 1;
    }

    while src_idx < src_size {
        if length == 0 {
            (length, src_idx) = read_literal_length(src, src_idx, opcode)?;
        }

        if dst_idx + length > dst_size {
            break;
        }

        copy_compressed_chunk(src, src_idx, length, &mut dst, dst_idx)?;
        dst_idx += length;
        src_idx += length;

        if src_idx >= src_size {
            break;
        }

        let (next_opcode, next_length, next_src_idx, next_dst_idx) =
            copy_decompressed_chunks(src, src_idx, &mut dst, dst_idx)?;
        opcode = next_opcode;
        length = next_length;
        src_idx = next_src_idx;
        dst_idx = next_dst_idx;
    }

    Ok(dst)
}

fn read_literal_length(src: &[u8], mut src_idx: usize, opcode: usize) -> Result<(usize, usize)> {
    let mut length = opcode + 8;
    if length == 0x17 {
        let mut n = *src.get(src_idx).ok_or_else(|| {
            DwgError::new(
                ErrorKind::Decode,
                "R2007 literal length read exceeds compressed data",
            )
        })? as usize;
        src_idx += 1;
        length += n;
        if n == 0xFF {
            loop {
                let lo = *src.get(src_idx).ok_or_else(|| {
                    DwgError::new(
                        ErrorKind::Decode,
                        "R2007 literal extension exceeds compressed data",
                    )
                })? as usize;
                let hi = *src.get(src_idx + 1).ok_or_else(|| {
                    DwgError::new(
                        ErrorKind::Decode,
                        "R2007 literal extension exceeds compressed data",
                    )
                })? as usize;
                src_idx += 2;
                n = lo | (hi << 8);
                length += n;
                if n != 0xFFFF {
                    break;
                }
            }
        }
    }
    Ok((length, src_idx))
}

fn copy_compressed_chunk(
    src: &[u8],
    mut src_idx: usize,
    mut length: usize,
    dst: &mut [u8],
    dst_idx: usize,
) -> Result<()> {
    let mut out = dst_idx;

    while length >= 32 {
        copy_16b(src, src_idx + 16, dst, &mut out)?;
        copy_16b(src, src_idx, dst, &mut out)?;
        src_idx += 32;
        length -= 32;
    }

    match length {
        0 => {}
        1 => copy_1b(src, src_idx, dst, &mut out)?,
        2 => copy_2b(src, src_idx, dst, &mut out)?,
        3 => copy_3b(src, src_idx, dst, &mut out)?,
        4 => copy_4b(src, src_idx, dst, &mut out)?,
        5 => {
            copy_1b(src, src_idx + 4, dst, &mut out)?;
            copy_4b(src, src_idx, dst, &mut out)?;
        }
        6 => {
            copy_1b(src, src_idx + 5, dst, &mut out)?;
            copy_4b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        7 => {
            copy_2b(src, src_idx + 5, dst, &mut out)?;
            copy_4b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        8 => {
            copy_4b(src, src_idx, dst, &mut out)?;
            copy_4b(src, src_idx + 4, dst, &mut out)?;
        }
        9 => {
            copy_1b(src, src_idx + 8, dst, &mut out)?;
            copy_8b(src, src_idx, dst, &mut out)?;
        }
        10 => {
            copy_1b(src, src_idx + 9, dst, &mut out)?;
            copy_8b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        11 => {
            copy_2b(src, src_idx + 9, dst, &mut out)?;
            copy_8b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        12 => {
            copy_4b(src, src_idx + 8, dst, &mut out)?;
            copy_8b(src, src_idx, dst, &mut out)?;
        }
        13 => {
            copy_1b(src, src_idx + 12, dst, &mut out)?;
            copy_4b(src, src_idx + 8, dst, &mut out)?;
            copy_8b(src, src_idx, dst, &mut out)?;
        }
        14 => {
            copy_1b(src, src_idx + 13, dst, &mut out)?;
            copy_4b(src, src_idx + 9, dst, &mut out)?;
            copy_8b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        15 => {
            copy_2b(src, src_idx + 13, dst, &mut out)?;
            copy_4b(src, src_idx + 9, dst, &mut out)?;
            copy_8b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        16 => copy_16b(src, src_idx, dst, &mut out)?,
        17 => {
            copy_8b(src, src_idx + 9, dst, &mut out)?;
            copy_1b(src, src_idx + 8, dst, &mut out)?;
            copy_8b(src, src_idx, dst, &mut out)?;
        }
        18 => {
            copy_1b(src, src_idx + 17, dst, &mut out)?;
            copy_16b(src, src_idx + 1, dst, &mut out)?;
            copy_1b(src, src_idx, dst, &mut out)?;
        }
        19 => {
            copy_3b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        20 => {
            copy_4b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        21 => {
            copy_1b(src, src_idx + 20, dst, &mut out)?;
            copy_4b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        22 => {
            copy_2b(src, src_idx + 20, dst, &mut out)?;
            copy_4b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        23 => {
            copy_3b(src, src_idx + 20, dst, &mut out)?;
            copy_4b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        24 => {
            copy_8b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        25 => {
            copy_8b(src, src_idx + 17, dst, &mut out)?;
            copy_1b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        26 => {
            copy_1b(src, src_idx + 25, dst, &mut out)?;
            copy_8b(src, src_idx + 17, dst, &mut out)?;
            copy_1b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        27 => {
            copy_2b(src, src_idx + 25, dst, &mut out)?;
            copy_8b(src, src_idx + 17, dst, &mut out)?;
            copy_1b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        28 => {
            copy_4b(src, src_idx + 24, dst, &mut out)?;
            copy_8b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        29 => {
            copy_1b(src, src_idx + 28, dst, &mut out)?;
            copy_4b(src, src_idx + 24, dst, &mut out)?;
            copy_8b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        30 => {
            copy_2b(src, src_idx + 28, dst, &mut out)?;
            copy_4b(src, src_idx + 24, dst, &mut out)?;
            copy_8b(src, src_idx + 16, dst, &mut out)?;
            copy_16b(src, src_idx, dst, &mut out)?;
        }
        31 => {
            copy_1b(src, src_idx + 30, dst, &mut out)?;
            copy_4b(src, src_idx + 26, dst, &mut out)?;
            copy_8b(src, src_idx + 18, dst, &mut out)?;
            copy_16b(src, src_idx + 2, dst, &mut out)?;
            copy_2b(src, src_idx, dst, &mut out)?;
        }
        _ => {
            return Err(DwgError::new(
                ErrorKind::Decode,
                "R2007 invalid compressed chunk length",
            ));
        }
    }

    Ok(())
}

fn copy_decompressed_chunks(
    src: &[u8],
    mut src_idx: usize,
    dst: &mut [u8],
    mut dst_idx: usize,
) -> Result<(usize, usize, usize, usize)> {
    let src_size = src.len();
    let mut opcode = read_u8(src, &mut src_idx)? as usize;
    let (mut opcode_next, mut offset, mut length, mut src_idx_next) =
        read_instructions(src, src_idx, opcode)?;
    opcode = opcode_next;
    src_idx = src_idx_next;

    loop {
        dst_idx = copy_from_output(dst, dst_idx, offset, length)?;

        length = opcode & 0x07;
        if length != 0 || src_idx >= src_size {
            break;
        }

        opcode = read_u8(src, &mut src_idx)? as usize;
        if (opcode >> 4) == 0 {
            break;
        }
        if (opcode >> 4) == 15 {
            opcode &= 0x0F;
        }

        (opcode_next, offset, length, src_idx_next) = read_instructions(src, src_idx, opcode)?;
        opcode = opcode_next;
        src_idx = src_idx_next;
    }

    Ok((opcode, length, src_idx, dst_idx))
}

fn read_instructions(
    src: &[u8],
    mut src_idx: usize,
    mut opcode: usize,
) -> Result<(usize, usize, usize, usize)> {
    let mut length;
    let offset;
    match opcode >> 4 {
        0 => {
            length = (opcode & 0x0F) + 0x13;
            offset = read_u8(src, &mut src_idx)? as usize;
            opcode = read_u8(src, &mut src_idx)? as usize;
            length += (opcode >> 3) & 0x10;
            let offset = ((opcode & 0x78) << 5) + 1 + offset;
            Ok((opcode, offset, length, src_idx))
        }
        1 => {
            length = (opcode & 0x0F) + 0x03;
            offset = read_u8(src, &mut src_idx)? as usize;
            opcode = read_u8(src, &mut src_idx)? as usize;
            let offset = ((opcode & 0xF8) << 5) + 1 + offset;
            Ok((opcode, offset, length, src_idx))
        }
        2 => {
            let mut offset = read_u8(src, &mut src_idx)? as usize;
            offset |= ((read_u8(src, &mut src_idx)? as usize) << 8) & 0xFF00;
            length = opcode & 0x07;
            if (opcode & 0x08) == 0 {
                opcode = read_u8(src, &mut src_idx)? as usize;
                length += opcode & 0xF8;
            } else {
                offset += 1;
                length += (read_u8(src, &mut src_idx)? as usize) << 3;
                opcode = read_u8(src, &mut src_idx)? as usize;
                length = (((opcode & 0xF8) << 8) + length) + 0x100;
            }
            Ok((opcode, offset, length, src_idx))
        }
        _ => {
            length = opcode >> 4;
            let mut offset = opcode & 0x0F;
            opcode = read_u8(src, &mut src_idx)? as usize;
            offset = ((opcode & 0xF8) << 1) + offset + 1;
            Ok((opcode, offset, length, src_idx))
        }
    }
}

fn copy_from_output(dst: &mut [u8], dst_idx: usize, offset: usize, length: usize) -> Result<usize> {
    let src_idx = dst_idx.checked_sub(offset).ok_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "R2007 back-reference offset exceeds decompressed prefix",
        )
    })?;
    let end = dst_idx
        .checked_add(length)
        .ok_or_else(|| DwgError::new(ErrorKind::Decode, "R2007 decompressed write overflow"))?;
    if end > dst.len() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "R2007 decompressed write exceeds output buffer",
        ));
    }
    for i in 0..length {
        let src_pos = src_idx + i;
        let dst_pos = dst_idx + i;
        if src_pos >= dst.len() {
            return Err(DwgError::new(
                ErrorKind::Decode,
                "R2007 decompressed read exceeds output buffer",
            ));
        }
        dst[dst_pos] = dst[src_pos];
    }
    Ok(end)
}

fn read_u8(src: &[u8], src_idx: &mut usize) -> Result<u8> {
    let value = *src.get(*src_idx).ok_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "R2007 compressed stream read exceeds buffer",
        )
    })?;
    *src_idx += 1;
    Ok(value)
}

fn copy_1b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    if src_idx >= src.len() || *dst_idx >= dst.len() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "R2007 copy_1b out of range",
        ));
    }
    dst[*dst_idx] = src[src_idx];
    *dst_idx += 1;
    Ok(())
}

fn copy_2b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    copy_1b(src, src_idx + 1, dst, dst_idx)?;
    copy_1b(src, src_idx, dst, dst_idx)
}

fn copy_3b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    copy_1b(src, src_idx + 2, dst, dst_idx)?;
    copy_1b(src, src_idx + 1, dst, dst_idx)?;
    copy_1b(src, src_idx, dst, dst_idx)
}

fn copy_4b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    copy_bytes_direct(src, src_idx, 4, dst, dst_idx)
}

fn copy_8b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    copy_bytes_direct(src, src_idx, 8, dst, dst_idx)
}

fn copy_16b(src: &[u8], src_idx: usize, dst: &mut [u8], dst_idx: &mut usize) -> Result<()> {
    copy_8b(src, src_idx + 8, dst, dst_idx)?;
    copy_8b(src, src_idx, dst, dst_idx)
}

fn copy_bytes_direct(
    src: &[u8],
    src_idx: usize,
    length: usize,
    dst: &mut [u8],
    dst_idx: &mut usize,
) -> Result<()> {
    let src_end = src_idx
        .checked_add(length)
        .ok_or_else(|| DwgError::new(ErrorKind::Decode, "R2007 source range overflow"))?;
    let dst_end = (*dst_idx)
        .checked_add(length)
        .ok_or_else(|| DwgError::new(ErrorKind::Decode, "R2007 destination range overflow"))?;
    if src_end > src.len() || dst_end > dst.len() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "R2007 direct copy out of range",
        ));
    }
    dst[*dst_idx..dst_end].copy_from_slice(&src[src_idx..src_end]);
    *dst_idx = dst_end;
    Ok(())
}

fn decode_utf16_string(bytes: &[u8]) -> Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R2007 UTF-16 section name has odd byte length",
        ));
    }
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    let decoded = String::from_utf16_lossy(&units);
    Ok(decoded.trim_end_matches('\0').to_string())
}

fn record_no_for_name(name: &str) -> u8 {
    match name {
        "AcDb:Header" | "AcDb:Headers" => 0,
        "AcDb:Classes" => 1,
        "AcDb:Handles" => 2,
        "AcDb:Template" => 4,
        _ => 255,
    }
}

fn align_up(value: u64, align: u64) -> Result<u64> {
    if align == 0 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "alignment must be non-zero",
        ));
    }
    let adjusted = value
        .checked_add(align - 1)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "alignment overflow"))?;
    Ok((adjusted / align) * align)
}

fn div_ceil(value: u64, divisor: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    value.div_ceil(divisor)
}

fn to_usize(value: u64, label: &str) -> Result<usize> {
    usize::try_from(value)
        .map_err(|_| DwgError::new(ErrorKind::Format, format!("{label} exceeds usize range")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dwg::decoder::Decoder;
    use crate::dwg::version::{detect_version, DwgVersion};

    #[test]
    fn detects_ac1021_from_sample() {
        let bytes = std::fs::read("dwg_samples/line_2007.dwg").expect("sample file");
        let version = detect_version(&bytes).expect("version tag");
        assert_eq!(version, DwgVersion::R2007);
    }

    #[test]
    fn ensure_supported_accepts_r2007() {
        let bytes = std::fs::read("dwg_samples/line_2007.dwg").expect("sample file");
        let decoder = Decoder::new(&bytes, Default::default()).expect("decoder");
        decoder
            .ensure_supported()
            .expect("R2007 should be supported");
    }

    #[test]
    fn parses_section_directory_and_core_names_for_ac1021() {
        let bytes = std::fs::read("dwg_samples/line_2007.dwg").expect("sample file");
        let dir = parse_section_directory(&bytes, &Default::default()).expect("section directory");
        assert!(dir.record_count > 0);
        assert!(dir
            .records
            .iter()
            .any(|record| record.name.as_deref() == Some("AcDb:Classes")));
        assert!(dir
            .records
            .iter()
            .any(|record| record.name.as_deref() == Some("AcDb:Handles")));
        assert!(dir
            .records
            .iter()
            .any(|record| record.name.as_deref() == Some("AcDb:AcDbObjects")));
        assert!(dir.records.iter().any(|record| {
            record.name.as_deref() == Some("AcDb:AcDbObjects")
                && record.offset > 0
                && record.size > 0
        }));
    }

    #[test]
    fn loads_sections_and_builds_object_core_for_ac1021() {
        let bytes = std::fs::read("dwg_samples/line_2007.dwg").expect("sample file");
        let dir = parse_section_directory(&bytes, &Default::default()).expect("section directory");

        let index = dir
            .records
            .iter()
            .position(|record| record.name.as_deref() == Some("AcDb:Handles"))
            .expect("handles section");
        let section = load_section_by_index(&bytes, &dir, index, &Default::default())
            .expect("handles section data");
        assert!(!section.data.is_empty());

        let object_index = build_object_index(&bytes, &Default::default()).expect("object index");
        assert!(!object_index.objects.is_empty());

        let first = object_index.objects[0];
        let record =
            parse_object_record(&bytes, first.offset, &Default::default()).expect("object record");
        assert!(record.size > 0);

        let dynamic_map = load_dynamic_type_map(&bytes, &Default::default()).expect("dynamic map");
        assert!(!dynamic_map.is_empty());
    }

    #[test]
    fn builds_object_index_for_core_ac1021_samples() {
        let cases = [
            ("dwg_samples/line_2007.dwg", 0x13u16),
            ("dwg_samples/arc_2007.dwg", 0x11u16),
            ("dwg_samples/polyline2d_line_2007.dwg", 0x4Du16),
        ];

        for (path, expected_type) in cases {
            let bytes = std::fs::read(path).expect("sample file");
            let index = build_object_index(&bytes, &Default::default()).expect("object index");
            assert!(!index.objects.is_empty(), "empty object index for {path}");

            let mut found = false;
            for object in &index.objects {
                let record = parse_object_record(&bytes, object.offset, &Default::default())
                    .expect("object record");
                let header = crate::objects::object_header_r2000::parse_from_record(&record)
                    .expect("object header");
                if header.type_code == expected_type {
                    found = true;
                    break;
                }
            }

            assert!(
                found,
                "expected object type {expected_type:#x} not found in {path}"
            );
        }
    }

    #[test]
    fn decodes_line_entity_geometry_from_ac1021_sample() {
        let bytes = std::fs::read("dwg_samples/line_2007.dwg").expect("sample file");
        let index = build_object_index(&bytes, &Default::default()).expect("object index");

        let mut decoded_count = 0usize;
        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &Default::default()).expect("record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x13 {
                continue;
            }
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_line_r2007(&mut reader).expect("line");

            assert!((entity.start.0 - 50.0).abs() < 1e-9);
            assert!((entity.start.1 - 50.0).abs() < 1e-9);
            assert!((entity.start.2 - 0.0).abs() < 1e-9);
            assert!((entity.end.0 - 100.0).abs() < 1e-9);
            assert!((entity.end.1 - 100.0).abs() < 1e-9);
            assert!((entity.end.2 - 0.0).abs() < 1e-9);
            decoded_count += 1;
        }

        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn decodes_lwpolyline_vertices_from_ac1021_sample() {
        let bytes = std::fs::read("dwg_samples/polyline2d_line_2007.dwg").expect("sample file");
        let index = build_object_index(&bytes, &Default::default()).expect("object index");

        let mut decoded_count = 0usize;
        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &Default::default()).expect("record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x4D {
                continue;
            }
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_lwpolyline_r2007(&mut reader).expect("lwpolyline");

            assert_eq!(entity.vertices.len(), 3);
            assert!((entity.vertices[0].0 - 50.0).abs() < 1e-9);
            assert!((entity.vertices[0].1 - 50.0).abs() < 1e-9);
            assert!((entity.vertices[1].0 - 100.0).abs() < 1e-9);
            assert!((entity.vertices[1].1 - 100.0).abs() < 1e-9);
            assert!((entity.vertices[2].0 - 150.0).abs() < 1e-9);
            assert!((entity.vertices[2].1 - 50.0).abs() < 1e-9);
            decoded_count += 1;
        }

        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn decodes_arc_entity_geometry_from_ac1021_sample() {
        let bytes = std::fs::read("dwg_samples/arc_2007.dwg").expect("sample file");
        let index = build_object_index(&bytes, &Default::default()).expect("object index");

        let mut decoded_count = 0usize;
        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &Default::default()).expect("record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x11 {
                continue;
            }
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_arc_r2007(&mut reader).expect("arc");

            assert!((entity.center.0 - 75.0).abs() < 1e-9);
            assert!((entity.center.1 - 50.0).abs() < 1e-9);
            assert!((entity.center.2 - 0.0).abs() < 1e-9);
            assert!((entity.radius - 25.0).abs() < 1e-9);
            assert!((entity.angle_start - 0.0).abs() < 1e-9);
            assert!((entity.angle_end - std::f64::consts::PI).abs() < 1e-9);
            decoded_count += 1;
        }

        assert_eq!(decoded_count, 1);
    }
}
