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
    codepage: Option<u16>,
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
        let mut reader = BitReader::new_with_codepage(self.body.as_ref(), self.codepage);
        reader.set_pos(0, self.body_bit_pos);
        reader
    }

    pub fn with_codepage(mut self, codepage: Option<u16>) -> Self {
        self.codepage = codepage;
        self
    }
}

pub fn parse_object_record<'a>(bytes: &'a [u8], offset: u32) -> Result<ObjectRecord<'a>> {
    parse_object_record_impl(bytes, offset, false)
}

/// R2010+ object records carry an extra MC "handle stream size" field between
/// the MS size and the object data, and the MS size does **not** include that
/// field. Slicing `size` bytes from right after the MS (the pre-R2010 layout)
/// therefore drops the last MC-width bytes of the object — the tail of the
/// handle stream — and puts every "size * 8 - handle_stream_size" position
/// off by the MC width. This variant measures the MC field and extends the
/// body accordingly (`body` still starts at the MC field, `size` stays the MS
/// value), so `body_bits - handle_stream_size_bits` is the exact data end.
pub fn parse_object_record_r2010<'a>(bytes: &'a [u8], offset: u32) -> Result<ObjectRecord<'a>> {
    parse_object_record_impl(bytes, offset, true)
}

fn parse_object_record_impl<'a>(
    bytes: &'a [u8],
    offset: u32,
    size_excludes_handle_size_field: bool,
) -> Result<ObjectRecord<'a>> {
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
    let handle_size_field_bytes = if size_excludes_handle_size_field {
        let mut probe = reader.clone();
        let before = probe.tell_bits();
        probe.read_umc()?;
        ((probe.tell_bits() - before) / 8) as usize
    } else {
        0
    };
    let end = body_start
        .checked_add(size as usize)
        .and_then(|value| value.checked_add(handle_size_field_bytes))
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
        codepage: None,
    })
}

pub fn parse_object_record_owned(bytes: &[u8], offset: u32) -> Result<ObjectRecord<'static>> {
    let record = parse_object_record(bytes, offset)?;
    Ok(owned_copy(&record))
}

/// Owned variant of [`parse_object_record_r2010`].
pub fn parse_object_record_owned_r2010(bytes: &[u8], offset: u32) -> Result<ObjectRecord<'static>> {
    let record = parse_object_record_r2010(bytes, offset)?;
    Ok(owned_copy(&record))
}

fn owned_copy(record: &ObjectRecord<'_>) -> ObjectRecord<'static> {
    ObjectRecord {
        offset: record.offset,
        size: record.size,
        body_start: record.body_start,
        body_bit_pos: record.body_bit_pos,
        body: Cow::Owned(record.body.as_ref().to_vec()),
        raw: Cow::Owned(record.raw.as_ref().to_vec()),
        codepage: record.codepage,
    }
}
