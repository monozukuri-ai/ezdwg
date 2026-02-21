use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, read_additional_entity_handles, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct LongTransactionEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub owner_handle: Option<u64>,
    pub reactor_handles: Vec<u64>,
    pub xdic_obj_handle: Option<u64>,
    pub ltype_handle: Option<u64>,
    pub plotstyle_handle: Option<u64>,
    pub material_handle: Option<u64>,
    pub extra_handles: Vec<u64>,
}

pub fn decode_long_transaction(reader: &mut BitReader<'_>) -> Result<LongTransactionEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_long_transaction_with_header(reader, header)
}

pub fn decode_long_transaction_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<LongTransactionEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_long_transaction_with_header(reader, header)
}

pub fn decode_long_transaction_r2007(reader: &mut BitReader<'_>) -> Result<LongTransactionEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_long_transaction_with_header(reader, header)
}

pub fn decode_long_transaction_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LongTransactionEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_long_transaction_with_header(reader, header)
}

pub fn decode_long_transaction_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LongTransactionEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_long_transaction_with_header(reader, header)
}

fn decode_long_transaction_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
) -> Result<LongTransactionEntity> {
    // LONG_TRANSACTION body payload is still TODO. Decode the full common handle
    // stream so callers can inspect ownership/reactor topology.
    reader.set_bit_pos(header.obj_size);
    let common_handles = parse_common_entity_handles(reader, &header)?;
    let extra_handles = read_additional_entity_handles(reader, header.handle, 8);

    Ok(LongTransactionEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        owner_handle: common_handles.owner_ref,
        reactor_handles: common_handles.reactors,
        xdic_obj_handle: common_handles.xdic_obj,
        ltype_handle: common_handles.ltype,
        plotstyle_handle: common_handles.plotstyle,
        material_handle: common_handles.material,
        extra_handles,
    })
}
