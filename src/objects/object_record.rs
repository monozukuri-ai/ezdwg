use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct ObjectRecord<'a> {
    pub offset: u32,
    pub size: u32,
    pub body_start: usize,
    pub body_bit_pos: u8,
    pub body: Cow<'a, [u8]>,
    pub raw: Cow<'a, [u8]>,
}

impl<'a> ObjectRecord<'a> {
    pub fn body_range(&self) -> (usize, usize) {
        let start = self.body_start;
        let end = start + self.size as usize;
        (start, end)
    }

    pub fn record_range(&self) -> (usize, usize) {
        let start = self.offset as usize;
        let end = start + self.raw.len();
        (start, end)
    }

    pub fn bit_reader(&self) -> BitReader<'_> {
        let mut reader = BitReader::new(self.body.as_ref());
        reader.set_pos(0, self.body_bit_pos);
        reader
    }
}

pub fn parse_object_record<'a>(bytes: &'a [u8], offset: u32) -> Result<ObjectRecord<'a>> {
    let offset_usize = offset as usize;
    if offset_usize >= bytes.len() {
        return Err(
            DwgError::new(ErrorKind::Format, "object record offset exceeds file size")
                .with_offset(offset as u64),
        );
    }

    let mut reader = BitReader::new(bytes);
    reader.set_pos(offset_usize, 0);

    let size = reader.read_ms()?; // size in bytes excluding CRC
    if size == 0 {
        return Err(
            DwgError::new(ErrorKind::Format, "object record size is zero")
                .with_offset(offset as u64),
        );
    }

    let (body_start, body_bit_pos) = reader.get_pos();
    let end = body_start
        .checked_add(size as usize)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "object size overflow"))?;
    if end + 2 > bytes.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("object record exceeds file size: end {end} + crc"),
        )
        .with_offset(offset as u64));
    }

    let raw_end = end + 2;
    let body = &bytes[body_start..end];
    let raw = &bytes[offset_usize..raw_end];

    Ok(ObjectRecord {
        offset,
        size,
        body_start,
        body_bit_pos,
        body: Cow::Borrowed(body),
        raw: Cow::Borrowed(raw),
    })
}

pub fn parse_object_record_owned(bytes: &[u8], offset: u32) -> Result<ObjectRecord<'static>> {
    let record = parse_object_record(bytes, offset)?;
    Ok(ObjectRecord {
        offset: record.offset,
        size: record.size,
        body_start: record.body_start,
        body_bit_pos: record.body_bit_pos,
        body: Cow::Owned(record.body.as_ref().to_vec()),
        raw: Cow::Owned(record.raw.as_ref().to_vec()),
    })
}
