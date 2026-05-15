use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects::ObjectRef;

pub fn encode_object_map_section(objects: &[ObjectRef]) -> Result<Vec<u8>> {
    let mut ordered = objects.to_vec();
    ordered.sort_by_key(|obj| (obj.handle.0, obj.offset));

    let mut payload = Vec::new();
    let mut prev_handle = 0i64;
    let mut prev_offset = 0i64;

    for obj in ordered {
        let handle = obj.handle.0 as i64;
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
        payload.extend_from_slice(&encode_modular_char(delta_handle)?);
        payload.extend_from_slice(&encode_modular_char(delta_offset)?);
        prev_handle = handle;
        prev_offset = offset;
    }

    let section_size = payload
        .len()
        .checked_add(2)
        .ok_or_else(|| DwgError::new(ErrorKind::Unsupported, "object map section size overflow"))?;
    if section_size > u16::MAX as usize {
        return Err(DwgError::new(
            ErrorKind::Unsupported,
            format!("object map section too large: {section_size}"),
        ));
    }

    let mut out = Vec::with_capacity(section_size + 6);
    push_u16_be(&mut out, section_size as u16);
    out.extend_from_slice(&payload);
    push_u16_be(&mut out, 0); // CRC placeholder
    push_u16_be(&mut out, 2); // terminator block
    Ok(out)
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
}
