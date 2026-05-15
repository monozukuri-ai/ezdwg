use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects::object_record::{parse_object_record, ObjectRecord};
use crate::objects::ObjectRef;

#[derive(Debug, Clone, Copy)]
pub struct ObjectHeaderR2010 {
    pub offset: u32,
    pub data_size: u32,
    pub body_start: usize,
    pub body_bit_pos: u8,
    pub handle_stream_size_bits: u32,
    pub type_code: u16,
}

impl ObjectHeaderR2010 {
    pub fn body_bit_pos(&self) -> u8 {
        self.body_bit_pos
    }
}

pub fn parse_at(bytes: &[u8], offset: u32) -> Result<ObjectHeaderR2010> {
    let record = parse_object_record(bytes, offset)?;
    parse_from_record(&record)
}

pub fn parse_for_object(bytes: &[u8], object: ObjectRef) -> Result<ObjectHeaderR2010> {
    parse_at(bytes, object.offset)
}

pub fn parse_from_record(record: &ObjectRecord<'_>) -> Result<ObjectHeaderR2010> {
    let mut reader = BitReader::new(record.body.as_ref());
    reader.set_pos(0, record.body_bit_pos);

    let handle_stream_size_bits = reader.read_umc()?;
    let type_code = reader.read_ot_r2010()?;
    if type_code == 0 {
        return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
    }

    Ok(ObjectHeaderR2010 {
        offset: record.offset,
        data_size: record.size,
        body_start: record.body_start,
        body_bit_pos: record.body_bit_pos,
        handle_stream_size_bits,
        type_code,
    })
}
