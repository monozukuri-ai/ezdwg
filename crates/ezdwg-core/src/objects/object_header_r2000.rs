use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects::object_record::{parse_object_record, ObjectRecord};
use crate::objects::ObjectRef;

#[derive(Debug, Clone, Copy)]
pub struct ObjectHeaderR2000 {
    pub offset: u32,
    pub data_size: u32,
    pub body_start: usize,
    pub body_bit_pos: u8,
    pub type_code: u16,
}

impl ObjectHeaderR2000 {
    pub fn total_size(&self) -> u32 {
        let header_bytes = self.body_start as u32 - self.offset;
        header_bytes + self.data_size + 2
    }

    pub fn record_range(&self) -> (usize, usize) {
        let start = self.offset as usize;
        let end = self.body_start + self.data_size as usize + 2;
        (start, end)
    }

    pub fn data_range(&self) -> (usize, usize) {
        let start = self.body_start;
        let end = start + self.data_size as usize;
        (start, end)
    }

    pub fn body_bit_pos(&self) -> u8 {
        self.body_bit_pos
    }
}

pub fn parse_at(bytes: &[u8], offset: u32) -> Result<ObjectHeaderR2000> {
    let record = parse_object_record(bytes, offset)?;
    parse_from_record(&record)
}

pub fn parse_for_object(bytes: &[u8], object: ObjectRef) -> Result<ObjectHeaderR2000> {
    parse_at(bytes, object.offset)
}

pub fn parse_from_record(record: &ObjectRecord<'_>) -> Result<ObjectHeaderR2000> {
    let mut reader = BitReader::new(record.body.as_ref());
    reader.set_pos(0, record.body_bit_pos);
    let mut type_code = reader.read_bs()?;
    if type_code == 0 {
        reader.set_pos(0, record.body_bit_pos);
        let _handle_stream_size = reader.read_umc()?;
        type_code = reader.read_ot_r2010()?;
        if type_code == 0 {
            return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
        }
    }

    Ok(ObjectHeaderR2000 {
        offset: record.offset,
        data_size: record.size,
        body_start: record.body_start,
        body_bit_pos: record.body_bit_pos,
        type_code,
    })
}
