use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::parse_common_entity_header;

#[derive(Debug, Clone)]
pub struct SeqendEntity {
    pub handle: u64,
}

pub fn decode_seqend(reader: &mut BitReader<'_>) -> Result<SeqendEntity> {
    let header = parse_common_entity_header(reader)?;
    Ok(SeqendEntity {
        handle: header.handle,
    })
}
