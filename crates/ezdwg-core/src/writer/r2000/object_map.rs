use super::checksum::crc16;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects::ObjectRef;

pub fn encode_object_map_section(objects: &[ObjectRef]) -> Result<Vec<u8>> {
    let mut ordered = objects.to_vec();
    ordered.sort_by_key(|obj| (obj.handle.0, obj.offset));

    let mut out = Vec::new();
    let mut payload = Vec::new();
    let mut prev_handle = 0i64;
    let mut prev_offset = 0i64;

    for obj in ordered {
        let handle = i64::try_from(obj.handle.0)
            .map_err(|_| DwgError::new(ErrorKind::Unsupported, "object handle exceeds i64"))?;
        let offset = obj.offset as i64;
        let delta_handle = handle - prev_handle;
        let delta_offset = offset - prev_offset;
        if delta_handle < 0 || delta_offset < 0 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!(
                    "object map entries must be monotonic: handle delta={delta_handle} offset delta={delta_offset}"
                ),
            ));
        }
        let handle_bytes = encode_modular_char(delta_handle)?;
        let offset_bytes = encode_modular_char(delta_offset)?;
        if payload.len() + handle_bytes.len() + offset_bytes.len() + 2 > 2032 {
            append_block(&mut out, &payload);
            payload.clear();
            payload.extend_from_slice(&encode_modular_char(handle)?);
            payload.extend_from_slice(&encode_modular_char(offset)?);
        } else {
            payload.extend_from_slice(&handle_bytes);
            payload.extend_from_slice(&offset_bytes);
        }
        prev_handle = handle;
        prev_offset = offset;
    }

    if !payload.is_empty() {
        append_block(&mut out, &payload);
    }
    append_block(&mut out, &[]); // terminator has its own CRC
    Ok(out)
}

fn append_block(out: &mut Vec<u8>, payload: &[u8]) {
    let start = out.len();
    push_u16_be(out, (payload.len() + 2) as u16);
    out.extend_from_slice(payload);
    let crc = crc16(0xC0C1, &out[start..]);
    push_u16_be(out, crc);
}

fn encode_modular_char(value: i64) -> Result<Vec<u8>> {
    let negative = value < 0;
    let mut remaining = value.unsigned_abs();
    let mut out = Vec::with_capacity(4);

    for _ in 0..4 {
        let chunk = (remaining & 0x7F) as u8;
        remaining >>= 7;
        if remaining == 0 && chunk <= 0x3F {
            let mut final_byte = chunk;
            if negative {
                final_byte |= 0x40;
            }
            out.push(final_byte);
            return Ok(out);
        }
        out.push(chunk | 0x80);
    }

    Err(DwgError::new(
        ErrorKind::Unsupported,
        format!("modular char out of range: {value}"),
    ))
}

fn push_u16_be(buf: &mut Vec<u8>, value: u16) {
    buf.push((value >> 8) as u8);
    buf.push((value & 0x00FF) as u8);
}

#[cfg(test)]
mod tests {
    use super::encode_object_map_section;
    use crate::container::{SectionDirectory, SectionLocatorRecord};
    use crate::core::config::ParseConfig;
    use crate::objects::{build_object_index_from_directory, Handle, ObjectRef};

    #[test]
    fn roundtrip_object_map_through_existing_parser() {
        let refs = vec![
            ObjectRef {
                handle: Handle(1),
                offset: 100,
            },
            ObjectRef {
                handle: Handle(3),
                offset: 140,
            },
            ObjectRef {
                handle: Handle(10),
                offset: 220,
            },
        ];
        let bytes = encode_object_map_section(&refs).unwrap();
        let directory = SectionDirectory {
            record_count: 1,
            records: vec![SectionLocatorRecord {
                record_no: 2,
                offset: 0,
                size: bytes.len() as u32,
                name: Some("ObjectMap".to_string()),
            }],
            crc: 0,
            sentinel_ok: true,
        };
        let index = build_object_index_from_directory(&bytes, &directory, &ParseConfig::default())
            .expect("object map should parse");

        assert_eq!(index.len(), 3);
        assert_eq!(index.get(Handle(1)).unwrap().offset, 100);
        assert_eq!(index.get(Handle(3)).unwrap().offset, 140);
        assert_eq!(index.get(Handle(10)).unwrap().offset, 220);
    }

    #[test]
    fn partitions_large_maps_and_resets_page_deltas() {
        let refs: Vec<_> = (0..5000)
            .map(|i| ObjectRef {
                handle: Handle(100 + 3 * i),
                offset: 2000 + 100 * i as u32,
            })
            .collect();
        let bytes = encode_object_map_section(&refs).unwrap();
        let mut offset = 0;
        let mut pages = 0;
        loop {
            let size = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            assert!(size <= 2032);
            let stored = u16::from_be_bytes([bytes[offset + size], bytes[offset + size + 1]]);
            assert_eq!(stored, super::crc16(0xC0C1, &bytes[offset..offset + size]));
            offset += size + 2;
            if size == 2 {
                break;
            }
            pages += 1;
        }
        assert!(pages > 1);
        assert_eq!(offset, bytes.len());
        let directory = SectionDirectory {
            record_count: 1,
            records: vec![SectionLocatorRecord {
                record_no: 2,
                offset: 0,
                size: bytes.len() as u32,
                name: Some("ObjectMap".into()),
            }],
            crc: 0,
            sentinel_ok: true,
        };
        let index =
            build_object_index_from_directory(&bytes, &directory, &ParseConfig::default()).unwrap();
        assert_eq!(index.len(), refs.len());
        for entry in refs {
            assert_eq!(index.get(entry.handle).unwrap().offset, entry.offset);
        }
    }
}
