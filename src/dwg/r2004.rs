use std::borrow::Cow;
use std::collections::HashMap;

use crate::bit::{BitReader, Endian};
use crate::container::{SectionDirectory, SectionLocatorRecord, SectionSlice};
use crate::core::config::ParseConfig;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::io::ByteReader;
use crate::objects::object_record::parse_object_record_owned;
use crate::objects::{Handle, ObjectIndex, ObjectRecord, ObjectRef};

const HEADER_OFFSET: usize = 0x80;
const HEADER_SIZE: usize = 0x6c;
const SECTION_PAGE_MAP_MAGIC: u32 = 0x41630E3B;
const SECTION_MAP_MAGIC: u32 = 0x4163003B;
const DATA_SECTION_MAGIC: u32 = 0x4163043B;
const SENTINEL_CLASSES_BEFORE: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const SENTINEL_CLASSES_AFTER: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

#[derive(Debug, Clone)]
struct HeaderData {
    section_page_map_address: u64,
    section_map_id: u32,
}

#[derive(Debug, Clone)]
struct SystemSectionHeader {
    signature: u32,
    decompressed_size: u32,
    compressed_size: u32,
    compressed_type: u32,
}

#[derive(Debug, Clone)]
struct PageMapEntry {
    id: i32,
    address: u64,
}

#[derive(Debug, Clone)]
struct SectionMapHeader {
    section_entry_count: u32,
}

#[derive(Debug, Clone)]
struct SectionEntry {
    size: u64,
    max_decompressed_size: u32,
    compressed: u32,
    encrypted: u32,
    name: String,
    pages: Vec<SectionPageInfo>,
}

#[derive(Debug, Clone)]
struct SectionPageInfo {
    page_id: u32,
}

#[derive(Debug, Clone)]
struct DataSectionHeader {
    signature: u32,
    compressed_size: u32,
}

#[derive(Debug, Clone)]
struct ClassEntry {
    dxf_name: String,
}

pub fn parse_section_directory(bytes: &[u8], _config: &ParseConfig) -> Result<SectionDirectory> {
    let header = read_header_data(bytes)?;
    let page_map = read_page_map(bytes, &header)?;
    let section_map = read_section_map(bytes, &header, &page_map)?;

    let mut page_lookup = HashMap::with_capacity(page_map.len());
    for entry in page_map {
        if entry.id > 0 {
            page_lookup.insert(entry.id as u32, entry);
        }
    }

    let mut records = Vec::with_capacity(section_map.len());
    for section in section_map {
        let record_no = record_no_for_name(&section.name);
        let size = section
            .size
            .min(u32::MAX as u64)
            .try_into()
            .unwrap_or(u32::MAX);
        let offset = section
            .pages
            .first()
            .and_then(|page| page_lookup.get(&page.page_id))
            .map(|entry| entry.address as u32)
            .unwrap_or(0);

        records.push(SectionLocatorRecord {
            record_no,
            offset,
            size,
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
    let header = read_header_data(bytes)?;
    let page_map = read_page_map(bytes, &header)?;
    let section_map = read_section_map(bytes, &header, &page_map)?;
    let section = section_map
        .get(index)
        .cloned()
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section index out of range"))?;

    let mut page_lookup = HashMap::with_capacity(page_map.len());
    for entry in page_map {
        if entry.id > 0 {
            page_lookup.insert(entry.id as u32, entry);
        }
    }

    let data = load_section_data(bytes, &section, &page_lookup, config)?;

    let record = directory
        .records
        .get(index)
        .cloned()
        .unwrap_or(SectionLocatorRecord {
            record_no: record_no_for_name(&section.name),
            offset: section
                .pages
                .first()
                .and_then(|page| page_lookup.get(&page.page_id))
                .map(|entry| entry.address as u32)
                .unwrap_or(0),
            size: section
                .size
                .min(u32::MAX as u64)
                .try_into()
                .unwrap_or(u32::MAX),
            name: Some(section.name),
        });

    Ok(SectionSlice {
        record,
        data: Cow::Owned(data),
    })
}

pub fn build_object_index(bytes: &[u8], config: &ParseConfig) -> Result<ObjectIndex> {
    let handles_data = load_named_section_data(bytes, config, "AcDb:Handles")?;
    let objects_data = load_objects_section_data(bytes, config)?;
    let index = parse_object_map_handles(&handles_data, config)?;

    if config.strict {
        let mut valid_objects = Vec::with_capacity(index.objects.len());
        for object in index.objects {
            if parse_object_record_owned(&objects_data, object.offset).is_ok() {
                valid_objects.push(object);
            }
        }
        return Ok(ObjectIndex::from_objects(valid_objects));
    }

    // Performance path for permissive mode: keep object-index construction linear
    // and avoid eagerly reparsing every record here.
    let max_offset = objects_data.len();
    let objects = index
        .objects
        .into_iter()
        .filter(|object| (object.offset as usize) < max_offset)
        .collect();
    Ok(ObjectIndex::from_objects(objects))
}

pub fn load_objects_section_data(bytes: &[u8], config: &ParseConfig) -> Result<Vec<u8>> {
    load_named_section_data(bytes, config, "AcDb:AcDbObjects")
}

pub fn parse_object_record_from_section_data(
    data: &[u8],
    offset: u32,
) -> Result<ObjectRecord<'static>> {
    parse_object_record_owned(data, offset)
}

pub fn parse_object_record<'a>(
    bytes: &'a [u8],
    offset: u32,
    config: &ParseConfig,
) -> Result<ObjectRecord<'a>> {
    let data = load_objects_section_data(bytes, config)?;
    parse_object_record_from_section_data(&data, offset)
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
        map.insert(code as u16, class.dxf_name.to_ascii_uppercase());
    }
    Ok(map)
}

fn load_named_section_data(bytes: &[u8], config: &ParseConfig, name: &str) -> Result<Vec<u8>> {
    let header = read_header_data(bytes)?;
    let page_map = read_page_map(bytes, &header)?;
    let section_map = read_section_map(bytes, &header, &page_map)?;

    let mut page_lookup = HashMap::with_capacity(page_map.len());
    for entry in page_map {
        if entry.id > 0 {
            page_lookup.insert(entry.id as u32, entry);
        }
    }

    let section = section_map
        .iter()
        .find(|section| section.name == name)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, format!("section not found: {name}")))?;

    load_section_data(bytes, section, &page_lookup, config)
}

fn read_header_data(bytes: &[u8]) -> Result<HeaderData> {
    if bytes.len() < HEADER_OFFSET + HEADER_SIZE {
        return Err(DwgError::new(
            ErrorKind::Format,
            "file too small for R2004 header data",
        ));
    }
    let encrypted = &bytes[HEADER_OFFSET..HEADER_OFFSET + HEADER_SIZE];
    let magic = magic_sequence();
    let mut decrypted = vec![0u8; HEADER_SIZE];
    for (idx, out) in decrypted.iter_mut().enumerate() {
        *out = encrypted[idx] ^ magic[idx];
    }

    let mut reader = ByteReader::new(&decrypted);
    reader.seek(0x50)?;
    let _section_page_map_id = reader.read_u32_le()?;
    let section_page_map_address = reader.read_u64_le()?;
    let section_map_id = reader.read_u32_le()?;
    let _section_page_array_size = reader.read_u32_le()?;
    let _gap_array_size = reader.read_u32_le()?;

    Ok(HeaderData {
        section_page_map_address,
        section_map_id,
    })
}

fn read_system_section(bytes: &[u8], address: u64, expected_signature: u32) -> Result<Vec<u8>> {
    let offset = address as usize;
    if offset + 0x14 > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "system section header out of range",
        ));
    }
    let mut reader = ByteReader::new(&bytes[offset..]);
    let header = SystemSectionHeader {
        signature: reader.read_u32_le()?,
        decompressed_size: reader.read_u32_le()?,
        compressed_size: reader.read_u32_le()?,
        compressed_type: reader.read_u32_le()?,
    };
    let _checksum = reader.read_u32_le()?;
    if header.signature != expected_signature {
        return Err(DwgError::new(
            ErrorKind::Format,
            "unexpected system section signature",
        ));
    }
    let data_offset = offset + 0x14;
    let data_end = data_offset
        .checked_add(header.compressed_size as usize)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "system section size overflow"))?;
    if data_end > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "system section data out of range",
        ));
    }
    let data = &bytes[data_offset..data_end];
    if header.compressed_size == 0 {
        return Ok(Vec::new());
    }
    match header.compressed_type {
        0x02 => decompress_r18(data, header.decompressed_size as usize),
        _ => Err(DwgError::not_implemented(
            "unsupported R2004 system section compression type",
        )),
    }
}

fn read_page_map(bytes: &[u8], header: &HeaderData) -> Result<Vec<PageMapEntry>> {
    let page_map_addr = header
        .section_page_map_address
        .checked_add(0x100)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section page map address overflow"))?;
    let data = read_system_section(bytes, page_map_addr, SECTION_PAGE_MAP_MAGIC)?;
    let mut reader = ByteReader::new(&data);
    let mut page_address: u64 = 0x100;
    let mut entries = Vec::new();

    while reader.remaining() >= 8 {
        let id = reader.read_i32_le()?;
        let size = reader.read_u32_le()?;
        let entry = PageMapEntry {
            id,
            address: page_address,
        };
        page_address = page_address
            .checked_add(size as u64)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "page map address overflow"))?;
        if id < 0 {
            if reader.remaining() < 16 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "page map gap entry truncated",
                ));
            }
            reader.skip(16)?;
        }
        entries.push(entry);
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
        .find(|entry| entry.id == header.section_map_id as i32)
        .ok_or_else(|| {
            DwgError::new(ErrorKind::Format, "section map page not found in page map")
        })?;
    let data = read_system_section(bytes, section_map_page.address, SECTION_MAP_MAGIC)?;
    let mut reader = ByteReader::new(&data);
    if reader.remaining() < 20 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "section map header truncated",
        ));
    }
    let header = SectionMapHeader {
        section_entry_count: reader.read_u32_le()?,
    };
    let _x02 = reader.read_u32_le()?;
    let _x00007400 = reader.read_u32_le()?;
    let _x00 = reader.read_u32_le()?;
    let _unknown = reader.read_u32_le()?;

    let mut sections = Vec::with_capacity(header.section_entry_count as usize);
    for _ in 0..header.section_entry_count {
        if reader.remaining() < 88 {
            return Err(DwgError::new(ErrorKind::Format, "section entry truncated"));
        }
        let size = reader.read_u64_le()?;
        let page_count = reader.read_u32_le()?;
        let max_decompressed_size = reader.read_u32_le()?;
        let _unknown = reader.read_u32_le()?;
        let compressed = reader.read_u32_le()?;
        let _section_id = reader.read_u32_le()?;
        let encrypted = reader.read_u32_le()?;
        let name_bytes = reader.read_bytes(64)?;
        let name = read_cstring(name_bytes);

        let mut pages = Vec::with_capacity(page_count as usize);
        for _ in 0..page_count {
            if reader.remaining() < 16 {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "section page info truncated",
                ));
            }
            let page_id = reader.read_u32_le()?;
            let _data_size = reader.read_u32_le()?;
            let _start_offset = reader.read_u64_le()?;
            pages.push(SectionPageInfo { page_id });
        }

        sections.push(SectionEntry {
            size,
            max_decompressed_size,
            compressed,
            encrypted,
            name,
            pages,
        });
    }

    Ok(sections)
}

fn load_section_data(
    bytes: &[u8],
    section: &SectionEntry,
    page_map: &HashMap<u32, PageMapEntry>,
    config: &ParseConfig,
) -> Result<Vec<u8>> {
    if section.encrypted == 1 {
        return Err(DwgError::not_implemented(
            "encrypted R2004 sections are not supported",
        ));
    }
    let page_size = section.max_decompressed_size as usize;
    let total_size = page_size
        .checked_mul(section.pages.len())
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section size overflow"))?;
    if total_size as u64 > config.max_section_bytes {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section size {} exceeds limit {}",
                total_size, config.max_section_bytes
            ),
        ));
    }
    if total_size == 0 {
        return Ok(Vec::new());
    }
    let mut output = vec![0u8; total_size];

    for (page_idx, page) in section.pages.iter().enumerate() {
        let entry = page_map.get(&page.page_id).ok_or_else(|| {
            DwgError::new(ErrorKind::Format, "section page not found in page map")
        })?;
        let page_offset = entry.address as usize;
        if page_offset + 32 > bytes.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "data section header out of range",
            ));
        }
        let header_bytes =
            decrypt_data_section_header(&bytes[page_offset..page_offset + 32], entry.address)?;
        let header = parse_data_section_header(&header_bytes)?;
        if header.signature != DATA_SECTION_MAGIC {
            return Err(DwgError::new(
                ErrorKind::Format,
                "invalid data section signature",
            ));
        }
        let data_offset = page_offset + 32;
        let data_end = data_offset
            .checked_add(header.compressed_size as usize)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "data section size overflow"))?;
        if data_end > bytes.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                "data section data out of range",
            ));
        }
        let data = &bytes[data_offset..data_end];
        let decompressed = if section.compressed == 2 {
            decompress_r18(data, section.max_decompressed_size as usize)?
        } else {
            data.to_vec()
        };

        let start = page_idx
            .checked_mul(section.max_decompressed_size as usize)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "section page offset overflow"))?;
        if start >= output.len() {
            continue;
        }
        let end = (start + decompressed.len()).min(output.len());
        output[start..end].copy_from_slice(&decompressed[..end - start]);
    }

    Ok(output)
}

fn decrypt_data_section_header(bytes: &[u8], offset: u64) -> Result<[u8; 32]> {
    if bytes.len() < 32 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "data section header truncated",
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[..32]);
    let mask = 0x4164_536B_u32 ^ (offset as u32);
    for chunk in out.chunks_exact_mut(4) {
        let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) ^ mask;
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

fn parse_data_section_header(bytes: &[u8; 32]) -> Result<DataSectionHeader> {
    let mut reader = ByteReader::new(bytes);
    let signature = reader.read_u32_le()?;
    let _data_type = reader.read_u32_le()?;
    let compressed_size = reader.read_u32_le()?;
    let _decompressed_size = reader.read_u32_le()?;
    let _start_offset = reader.read_u32_le()?;
    let _page_header_checksum = reader.read_u32_le()?;
    let _data_checksum = reader.read_u32_le()?;
    let _unknown = reader.read_u32_le()?;
    Ok(DataSectionHeader {
        signature,
        compressed_size,
    })
}

fn record_no_for_name(name: &str) -> u8 {
    match name {
        "AcDb:Header" => 0,
        "AcDb:Classes" => 1,
        "AcDb:Handles" => 2,
        "AcDb:Template" => 4,
        _ => 255,
    }
}

fn read_cstring(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn magic_sequence() -> [u8; HEADER_SIZE] {
    let mut seq = [0u8; HEADER_SIZE];
    let mut randseed: u32 = 1;
    for byte in seq.iter_mut() {
        randseed = randseed.wrapping_mul(0x343fd);
        randseed = randseed.wrapping_add(0x269ec3);
        *byte = (randseed >> 16) as u8;
    }
    seq
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
    let max_class_number = reader.read_bs()?;
    let _zero0 = reader.read_rc()?;
    let _zero1 = reader.read_rc()?;
    let _bit_flag = reader.read_b()?;

    let mut classes = Vec::new();
    while reader.get_pos().0 <= size {
        let class_number = reader.read_bs()?;
        let _proxy_flags = reader.read_bs()?;
        let _app_name = reader.read_tv()?;
        let _cpp_name = reader.read_tv()?;
        let dxf_name = reader.read_tv()?;
        let _was_a_zombie = reader.read_b()?;
        let _item_class_id = reader.read_bs()?;
        let _number_of_objects = reader.read_bl()?;
        let _dwg_version = reader.read_bs()?;
        let _maintenance_version = reader.read_bs()?;
        let _unknown0 = reader.read_bl()?;
        let _unknown1 = reader.read_bl()?;

        classes.push(ClassEntry { dxf_name });

        if class_number == max_class_number {
            break;
        }
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
            let prev_handle = last_handle;
            let prev_offset = last_offset;
            last_handle += read_modular_char(&mut reader)?;
            last_offset += read_modular_char(&mut reader)?;

            if last_handle < 0 || last_offset < 0 {
                if config.strict {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        "AcDb:Handles contains negative handle or offset",
                    )
                    .with_offset(reader.tell()));
                }
                // Keep decoding in permissive mode; corrupted deltas are observed in
                // the wild and pydwg continues scanning the stream.
                last_handle = prev_handle;
                last_offset = prev_offset;
                continue;
            }
            if last_offset > u32::MAX as i64 {
                if config.strict {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        "AcDb:Handles offset exceeds u32 range",
                    )
                    .with_offset(reader.tell()));
                }
                last_handle = prev_handle;
                last_offset = prev_offset;
                continue;
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

fn decompress_r18(src: &[u8], dst_size: usize) -> Result<Vec<u8>> {
    let mut dst = vec![0u8; dst_size];
    let mut dst_idx: usize = 0;
    let mut cursor = Cursor::new(src);

    let (literal_len, mut opcode1) = read_literal_length(&mut cursor)?;
    dst_idx = copy_literal(&mut dst, dst_idx, src, &mut cursor, literal_len)?;

    while cursor.pos < src.len() {
        if opcode1 == 0x00 {
            opcode1 = cursor.read_u8()?;
        }

        let (compressed_bytes, compressed_offset, next_literal_len, next_opcode1) = match opcode1 {
            0x10 => {
                let comp_bytes = read_long_compression_offset(&mut cursor)? + 9;
                let (offset, literal_count) = read_two_byte_offset(&mut cursor)?;
                let offset = offset + 0x3FFF;
                let (literal_len, next_opcode1) = if literal_count == 0 {
                    read_literal_length(&mut cursor)?
                } else {
                    (literal_count, 0x00)
                };
                (comp_bytes, offset, literal_len, next_opcode1)
            }
            0x11 => break,
            0x12..=0x1F => {
                let comp_bytes = (opcode1 & 0x0F) as usize + 2;
                let (offset, literal_count) = read_two_byte_offset(&mut cursor)?;
                let offset = offset + 0x3FFF;
                let (literal_len, next_opcode1) = if literal_count == 0 {
                    read_literal_length(&mut cursor)?
                } else {
                    (literal_count, 0x00)
                };
                (comp_bytes, offset, literal_len, next_opcode1)
            }
            0x20 => {
                let comp_bytes = read_long_compression_offset(&mut cursor)? + 0x21;
                let (offset, literal_count) = read_two_byte_offset(&mut cursor)?;
                let (literal_len, next_opcode1) = if literal_count == 0 {
                    read_literal_length(&mut cursor)?
                } else {
                    (literal_count, 0x00)
                };
                (comp_bytes, offset, literal_len, next_opcode1)
            }
            0x21..=0x3F => {
                let comp_bytes = (opcode1 - 0x1E) as usize;
                let (offset, literal_count) = read_two_byte_offset(&mut cursor)?;
                let (literal_len, next_opcode1) = if literal_count == 0 {
                    read_literal_length(&mut cursor)?
                } else {
                    (literal_count, 0x00)
                };
                (comp_bytes, offset, literal_len, next_opcode1)
            }
            0x40..=0xFF => {
                let comp_bytes = ((opcode1 & 0xF0) >> 4) as usize - 1;
                let opcode2 = cursor.read_u8()? as usize;
                let offset = (opcode2 << 2) | ((opcode1 as usize & 0x0C) >> 2);
                if opcode1 & 0x03 != 0 {
                    let literal_len = (opcode1 & 0x03) as usize;
                    (comp_bytes, offset, literal_len, 0x00)
                } else {
                    let (literal_len, next_opcode1) = read_literal_length(&mut cursor)?;
                    (comp_bytes, offset, literal_len, next_opcode1)
                }
            }
            _ => {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    "invalid R2004 compression opcode",
                ))
            }
        };

        dst_idx = copy_decompressed(&mut dst, dst_idx, compressed_offset + 1, compressed_bytes)?;
        dst_idx = copy_literal(&mut dst, dst_idx, src, &mut cursor, next_literal_len)?;
        opcode1 = next_opcode1;
    }

    if dst.len() > dst_size {
        dst.truncate(dst_size);
    } else if dst.len() < dst_size {
        dst.resize(dst_size, 0);
    }

    Ok(dst)
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(DwgError::new(
                ErrorKind::Decode,
                "unexpected end of compressed stream",
            ));
        }
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }
}

fn read_literal_length(cursor: &mut Cursor<'_>) -> Result<(usize, u8)> {
    let mut opcode1 = 0u8;
    let mut length = 0usize;
    let byte = cursor.read_u8()?;
    if (0x01..=0x0F).contains(&byte) {
        length = byte as usize + 3;
    } else if byte & 0xF0 != 0 {
        opcode1 = byte;
    } else if byte == 0x00 {
        length = 0x0F;
        let mut b = cursor.read_u8()?;
        while b == 0x00 {
            length += 0xFF;
            b = cursor.read_u8()?;
        }
        length += b as usize + 3;
    }
    Ok((length, opcode1))
}

fn read_long_compression_offset(cursor: &mut Cursor<'_>) -> Result<usize> {
    let mut value = 0usize;
    let mut byte = cursor.read_u8()?;
    if byte == 0x00 {
        value = 0xFF;
        byte = cursor.read_u8()?;
        while byte == 0x00 {
            value += 0xFF;
            byte = cursor.read_u8()?;
        }
    }
    Ok(value + byte as usize)
}

fn read_two_byte_offset(cursor: &mut Cursor<'_>) -> Result<(usize, usize)> {
    let byte1 = cursor.read_u8()?;
    let byte2 = cursor.read_u8()?;
    let value = (byte1 as usize >> 2) | ((byte2 as usize) << 6);
    let literal_count = (byte1 & 0x03) as usize;
    Ok((value, literal_count))
}

fn copy_literal(
    dst: &mut Vec<u8>,
    dst_idx: usize,
    src: &[u8],
    cursor: &mut Cursor<'_>,
    length: usize,
) -> Result<usize> {
    if length == 0 {
        return Ok(dst_idx);
    }
    let end = cursor.pos + length;
    if end > src.len() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            "literal run exceeds compressed data",
        ));
    }
    ensure_len(dst, dst_idx + length);
    dst[dst_idx..dst_idx + length].copy_from_slice(&src[cursor.pos..end]);
    cursor.pos = end;
    Ok(dst_idx + length)
}

fn copy_decompressed(
    dst: &mut Vec<u8>,
    dst_idx: usize,
    offset: usize,
    length: usize,
) -> Result<usize> {
    if length == 0 {
        return Ok(dst_idx);
    }

    // Keep behavior permissive for corrupted blocks, matching pydwg's approach.
    if offset > dst_idx {
        ensure_len(dst, dst_idx + length);
        return Ok(dst_idx + length);
    }

    ensure_len(dst, dst_idx + length);
    let mut out = dst_idx;
    for _ in 0..length {
        let src_idx = out - offset;
        let byte = dst[src_idx];
        dst[out] = byte;
        out += 1;
    }
    Ok(out)
}

fn ensure_len(dst: &mut Vec<u8>, len: usize) {
    if dst.len() < len {
        dst.resize(len, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_section_directory_from_sample() {
        let bytes = std::fs::read("test_dwg/line_2004.dwg").expect("sample file");
        let directory =
            parse_section_directory(&bytes, &ParseConfig::default()).expect("directory");
        assert!(directory.record_count > 0);
        assert!(directory
            .records
            .iter()
            .any(|record| record.name.as_deref() == Some("AcDb:Handles")));
    }

    #[test]
    fn builds_object_index_from_handles_section() {
        let bytes = std::fs::read("test_dwg/line_2004.dwg").expect("sample file");
        let index = build_object_index(&bytes, &ParseConfig::default()).expect("object index");
        assert_eq!(index.objects.len(), 199);
    }

    #[test]
    fn parses_object_record_from_acdbobjects() {
        let bytes = std::fs::read("test_dwg/line_2004.dwg").expect("sample file");
        let config = ParseConfig::default();
        let index = build_object_index(&bytes, &config).expect("object index");
        let object = index.objects.first().expect("object");
        let record = parse_object_record(&bytes, object.offset, &config).expect("object record");
        assert!(record.size > 0);
    }

    #[test]
    fn parses_object_headers_from_records() {
        let bytes = std::fs::read("test_dwg/line_2004.dwg").expect("sample file");
        let config = ParseConfig::default();
        let index = build_object_index(&bytes, &config).expect("object index");

        let mut header_count = 0usize;
        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &config).expect("object record");
            let _header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            header_count += 1;
        }
        assert!(header_count > 0);
    }

    #[test]
    fn resolves_basic_entity_type_codes_in_r2004_samples() {
        let config = ParseConfig::default();

        let line_bytes = std::fs::read("test_dwg/line_2004.dwg").expect("line sample");
        let line_index = build_object_index(&line_bytes, &config).expect("line object index");
        let mut line_count = 0usize;
        for object in &line_index.objects {
            let record = parse_object_record(&line_bytes, object.offset, &config)
                .expect("line object record");
            let header = crate::objects::object_header_r2000::parse_from_record(&record)
                .expect("line header");
            if header.type_code == 0x13 {
                line_count += 1;
            }
        }
        assert_eq!(line_count, 1);

        let arc_bytes = std::fs::read("test_dwg/arc_2004.dwg").expect("arc sample");
        let arc_index = build_object_index(&arc_bytes, &config).expect("arc object index");
        let mut arc_count = 0usize;
        for object in &arc_index.objects {
            let record =
                parse_object_record(&arc_bytes, object.offset, &config).expect("arc object record");
            let header = crate::objects::object_header_r2000::parse_from_record(&record)
                .expect("arc header");
            if header.type_code == 0x11 {
                arc_count += 1;
            }
        }
        assert_eq!(arc_count, 1);

        let poly_bytes =
            std::fs::read("test_dwg/polyline2d_line_2004.dwg").expect("polyline sample");
        let poly_index = build_object_index(&poly_bytes, &config).expect("poly object index");
        let mut lw_count = 0usize;
        for object in &poly_index.objects {
            let record = parse_object_record(&poly_bytes, object.offset, &config)
                .expect("poly object record");
            let header = crate::objects::object_header_r2000::parse_from_record(&record)
                .expect("poly header");
            if header.type_code == 0x4D {
                lw_count += 1;
            }
        }
        assert_eq!(lw_count, 1);
    }

    #[test]
    fn decodes_insert_entity_from_r2004_sample() {
        let bytes = std::fs::read("test_dwg/insert_2004.dwg").expect("insert sample");
        let config = ParseConfig::default();
        let index = build_object_index(&bytes, &config).expect("object index");

        let mut insert_count = 0usize;
        let mut decoded_count = 0usize;
        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &config).expect("object record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x07 {
                continue;
            }
            insert_count += 1;

            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_insert(&mut reader).expect("insert entity");
            assert!((entity.position.0 - 100.0).abs() < 1e-9);
            assert!((entity.position.1 - 50.0).abs() < 1e-9);
            decoded_count += 1;
        }

        assert_eq!(insert_count, 1);
        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn legacy_polyline_sample_is_normalized_to_lwpolyline() {
        let bytes = std::fs::read("test_dwg/polyline2d_old_2004.dwg").expect("polyline sample");
        let config = ParseConfig::default();
        let index = build_object_index(&bytes, &config).expect("object index");

        let mut lwpolyline_count = 0usize;
        let mut legacy_polyline_count = 0usize;
        let mut vertex_2d_count = 0usize;
        let mut seqend_count = 0usize;

        for object in &index.objects {
            let record =
                parse_object_record(&bytes, object.offset, &config).expect("object record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            match header.type_code {
                0x4D => lwpolyline_count += 1,
                0x0F => legacy_polyline_count += 1,
                0x0A => vertex_2d_count += 1,
                0x06 => seqend_count += 1,
                _ => {}
            }
        }

        assert_eq!(lwpolyline_count, 1);
        assert_eq!(legacy_polyline_count, 0);
        assert_eq!(vertex_2d_count, 0);
        assert_eq!(seqend_count, 0);
    }

    #[test]
    fn decodes_point_circle_ellipse_from_r2004_samples() {
        let config = ParseConfig::default();

        let point2d_bytes = std::fs::read("test_dwg/point2d_2004.dwg").expect("point2d sample");
        let point2d_index =
            build_object_index(&point2d_bytes, &config).expect("point2d object index");
        let mut point2d_count = 0usize;
        for object in &point2d_index.objects {
            let record = parse_object_record(&point2d_bytes, object.offset, &config)
                .expect("point2d object record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x1B {
                continue;
            }
            point2d_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_point(&mut reader).expect("point");
            assert!((entity.location.0 - 50.0).abs() < 1e-9);
            assert!((entity.location.1 - 50.0).abs() < 1e-9);
        }
        assert_eq!(point2d_count, 1);

        let point3d_bytes = std::fs::read("test_dwg/point3d_2004.dwg").expect("point3d sample");
        let point3d_index =
            build_object_index(&point3d_bytes, &config).expect("point3d object index");
        let mut point3d_count = 0usize;
        for object in &point3d_index.objects {
            let record = parse_object_record(&point3d_bytes, object.offset, &config)
                .expect("point3d object record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x1B {
                continue;
            }
            point3d_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_point(&mut reader).expect("point");
            assert!((entity.location.2 - 50.0).abs() < 1e-9);
        }
        assert_eq!(point3d_count, 1);

        let circle_bytes = std::fs::read("test_dwg/circle_2004.dwg").expect("circle sample");
        let circle_index = build_object_index(&circle_bytes, &config).expect("circle object index");
        let mut circle_count = 0usize;
        for object in &circle_index.objects {
            let record =
                parse_object_record(&circle_bytes, object.offset, &config).expect("circle record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x12 {
                continue;
            }
            circle_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_circle(&mut reader).expect("circle");
            assert!((entity.radius - 50.0).abs() < 1e-9);
        }
        assert_eq!(circle_count, 1);

        let ellipse_bytes = std::fs::read("test_dwg/ellipse_2004.dwg").expect("ellipse sample");
        let ellipse_index =
            build_object_index(&ellipse_bytes, &config).expect("ellipse object index");
        let mut ellipse_count = 0usize;
        for object in &ellipse_index.objects {
            let record = parse_object_record(&ellipse_bytes, object.offset, &config)
                .expect("ellipse record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x23 {
                continue;
            }
            ellipse_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_ellipse(&mut reader).expect("ellipse");
            assert!((entity.center.0 - 100.0).abs() < 1e-9);
            assert!((entity.center.1 - 100.0).abs() < 1e-9);
            assert!((entity.major_axis.0 + 50.0).abs() < 1e-9);
            assert!((entity.major_axis.1 + 50.0).abs() < 1e-9);
            assert!((entity.axis_ratio - 0.4242640687119287).abs() < 1e-12);
        }
        assert_eq!(ellipse_count, 1);
    }

    #[test]
    fn decodes_text_mtext_from_r2004_samples() {
        let config = ParseConfig::default();

        let text_bytes = std::fs::read("test_dwg/text_2004.dwg").expect("text sample");
        let text_index = build_object_index(&text_bytes, &config).expect("text object index");
        let mut text_count = 0usize;
        for object in &text_index.objects {
            let record =
                parse_object_record(&text_bytes, object.offset, &config).expect("text record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x01 {
                continue;
            }
            text_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_text(&mut reader).expect("text");
            assert!(entity.text.contains("Hello"));
            assert!((entity.insertion.0 - 50.0).abs() < 1e-9);
            assert!((entity.insertion.1 - 50.0).abs() < 1e-9);
            assert!((entity.thickness - 0.0).abs() < 1e-9);
            assert!((entity.oblique_angle - 0.0).abs() < 1e-9);
            assert!((entity.height - 5.0).abs() < 1e-9);
            assert!(entity.style_handle.is_some());
        }
        assert_eq!(text_count, 1);

        let mtext_bytes = std::fs::read("test_dwg/mtext_2004.dwg").expect("mtext sample");
        let mtext_index = build_object_index(&mtext_bytes, &config).expect("mtext object index");
        let mut mtext_count = 0usize;
        for object in &mtext_index.objects {
            let record =
                parse_object_record(&mtext_bytes, object.offset, &config).expect("mtext record");
            let header =
                crate::objects::object_header_r2000::parse_from_record(&record).expect("header");
            if header.type_code != 0x2C {
                continue;
            }
            mtext_count += 1;
            let mut reader = record.bit_reader();
            let _type = reader.read_bs().expect("type");
            let entity = crate::entities::decode_mtext(&mut reader).expect("mtext");
            assert!(entity.text.contains("Hello"));
            assert!((entity.insertion.0 - 50.0).abs() < 1e-9);
            assert!((entity.insertion.1 - 50.0).abs() < 1e-9);
            assert!((entity.text_height - 5.0).abs() < 1e-9);
            assert!((entity.rect_width - 100.0).abs() < 1e-9);
        }
        assert_eq!(mtext_count, 1);
    }

    #[test]
    fn parse_object_map_handles_skips_negative_deltas_in_permissive_mode() {
        // One handles block with three entries:
        // 1) (+5, +10) -> valid
        // 2) (-20, +1) -> cumulative handle becomes negative (skip in permissive mode)
        // 3) (+30, +5) -> cumulative values become valid again
        let bytes = vec![
            0x00, 0x08, // section_size = 8 (2 bytes header + 6 bytes payload)
            0x05, 0x0A, // +5, +10
            0x54, 0x01, // -20, +1  (0x40 sign bit on final byte)
            0x1E, 0x05, // +30, +5
            0x00, 0x00, // crc
            0x00, 0x02, // terminator section
        ];
        let index = parse_object_map_handles(&bytes, &ParseConfig::default()).expect("index");
        assert_eq!(index.objects.len(), 2);
        assert_eq!(index.objects[0].handle.0, 5);
        assert_eq!(index.objects[0].offset, 10);
        assert_eq!(index.objects[1].handle.0, 35);
        assert_eq!(index.objects[1].offset, 15);
    }

    #[test]
    fn parse_object_map_handles_rejects_negative_deltas_in_strict_mode() {
        let bytes = vec![
            0x00, 0x06, // section_size = 6 (2 bytes header + 4 bytes payload)
            0x05, 0x0A, // +5, +10
            0x54, 0x01, // -20, +1 -> cumulative handle negative
            0x00, 0x00, // crc
            0x00, 0x02, // terminator section
        ];
        let mut config = ParseConfig::default();
        config.strict = true;
        let err = parse_object_map_handles(&bytes, &config).expect_err("strict error");
        assert!(err
            .to_string()
            .contains("AcDb:Handles contains negative handle or offset"));
    }
}
