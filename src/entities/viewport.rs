use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct ViewportEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
}

pub fn decode_viewport(reader: &mut BitReader<'_>) -> Result<ViewportEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_viewport_with_header(reader, header)
}

pub fn decode_viewport_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_viewport_with_header(reader, header)
}

pub fn decode_viewport_r2007(reader: &mut BitReader<'_>) -> Result<ViewportEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_viewport_with_header(reader, header)
}

pub fn decode_viewport_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_viewport_with_header(reader, header)
}

pub fn decode_viewport_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ViewportEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_viewport_with_header(reader, header)
}

fn decode_viewport_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
) -> Result<ViewportEntity> {
    // VIEWPORT body payload is still TODO. We can still expose style/layer metadata by
    // jumping to the handle stream and decoding common handles.
    reader.set_bit_pos(header.obj_size);
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(ViewportEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
    })
}
