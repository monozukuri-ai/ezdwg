use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

pub fn encode_ms_value(value: u32) -> Result<Vec<u8>> {
    let mut writer = BitWriter::new();
    writer.write_ms(value)?;
    Ok(writer.into_bytes())
}

pub fn encode_object_record(body: &[u8]) -> Result<Vec<u8>> {
    if body.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "object record body cannot be empty",
        ));
    }
    if body.len() > 0x3FFF_FFFFusize {
        return Err(DwgError::new(
            ErrorKind::Unsupported,
            format!("object record body too large: {}", body.len()),
        ));
    }

    let mut out = encode_ms_value(body.len() as u32)?;
    out.extend_from_slice(body);
    let mut tail = BitWriter::new();
    tail.write_rs(Endian::Little, 0)?;
    out.extend_from_slice(&tail.into_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{encode_ms_value, encode_object_record};
    use crate::objects::parse_object_record;

    #[test]
    fn encodes_ms_values_readable_by_bit_reader() {
        let one = encode_ms_value(1).unwrap();
        let max_single = encode_ms_value(0x7FFF).unwrap();
        let two_word = encode_ms_value(0x1_0000).unwrap();
        assert_eq!(one.len(), 2);
        assert_eq!(max_single.len(), 2);
        assert_eq!(two_word.len(), 4);
    }

    #[test]
    fn encodes_object_record_roundtrip() {
        let body = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01];
        let encoded = encode_object_record(&body).unwrap();
        let record = parse_object_record(&encoded, 0).unwrap();
        assert_eq!(record.size as usize, body.len());
        assert_eq!(record.body.as_ref(), body.as_slice());
        assert_eq!(record.raw.as_ref(), encoded.as_slice());
    }
}
