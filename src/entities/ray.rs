use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct RayEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub start: (f64, f64, f64),
    pub unit_vector: (f64, f64, f64),
}

pub fn decode_ray(reader: &mut BitReader<'_>) -> Result<RayEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_ray_with_header(reader, header, false, false)
}

pub fn decode_ray_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<RayEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_ray_with_header(reader, header, true, false)
}

pub fn decode_ray_r2007(reader: &mut BitReader<'_>) -> Result<RayEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_ray_with_header(reader, header, true, true)
}

pub fn decode_ray_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<RayEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ray_with_header(reader, header, true, true)
}

pub fn decode_ray_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<RayEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ray_with_header(reader, header, true, true)
}

fn decode_ray_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<RayEntity> {
    let start = reader.read_3bd()?;
    let unit_vector = reader.read_3bd()?;

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header)
    } else {
        parse_common_entity_handles(reader, &header).map(|common_handles| common_handles.layer)
    } {
        Ok(layer_handle) => layer_handle,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            0
        }
        Err(err) => return Err(err),
    };

    Ok(RayEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        start,
        unit_vector,
    })
}
