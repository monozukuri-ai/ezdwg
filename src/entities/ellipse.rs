use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct EllipseEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub center: (f64, f64, f64),
    pub major_axis: (f64, f64, f64),
    pub extrusion: (f64, f64, f64),
    pub axis_ratio: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

pub fn decode_ellipse(reader: &mut BitReader<'_>) -> Result<EllipseEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_ellipse_with_header(reader, header, false, false)
}

pub fn decode_ellipse_r2007(reader: &mut BitReader<'_>) -> Result<EllipseEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_ellipse_with_header(reader, header, true, true)
}

pub fn decode_ellipse_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<EllipseEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ellipse_with_header(reader, header, true, true)
}

pub fn decode_ellipse_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<EllipseEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ellipse_with_header(reader, header, true, true)
}

fn decode_ellipse_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<EllipseEntity> {
    let center = reader.read_3bd()?;
    let major_axis = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let axis_ratio = reader.read_bd()?;
    let start_angle = reader.read_bd()?;
    let end_angle = reader.read_bd()?;
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

    Ok(EllipseEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    })
}
