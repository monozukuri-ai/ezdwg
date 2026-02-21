use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct OleFrameEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
}

pub fn decode_oleframe(reader: &mut BitReader<'_>) -> Result<OleFrameEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_ole_entity_with_header(reader, header)
}

pub fn decode_oleframe_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_ole_entity_with_header(reader, header)
}

pub fn decode_oleframe_r2007(reader: &mut BitReader<'_>) -> Result<OleFrameEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_ole_entity_with_header(reader, header)
}

pub fn decode_oleframe_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ole_entity_with_header(reader, header)
}

pub fn decode_oleframe_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ole_entity_with_header(reader, header)
}

pub fn decode_ole2frame(reader: &mut BitReader<'_>) -> Result<OleFrameEntity> {
    decode_oleframe(reader)
}

pub fn decode_ole2frame_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    decode_oleframe_r14(reader, object_handle)
}

pub fn decode_ole2frame_r2007(reader: &mut BitReader<'_>) -> Result<OleFrameEntity> {
    decode_oleframe_r2007(reader)
}

pub fn decode_ole2frame_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    decode_oleframe_r2010(reader, object_data_end_bit, object_handle)
}

pub fn decode_ole2frame_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<OleFrameEntity> {
    decode_oleframe_r2013(reader, object_data_end_bit, object_handle)
}

fn decode_ole_entity_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
) -> Result<OleFrameEntity> {
    // OLEFRAME/OLE2FRAME payload decoding is pending; expose style/layer metadata by
    // decoding the common handle stream.
    reader.set_bit_pos(header.obj_size);
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(OleFrameEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
    })
}
