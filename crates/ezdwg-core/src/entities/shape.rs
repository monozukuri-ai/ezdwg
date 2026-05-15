use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct ShapeEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub insertion: (f64, f64, f64),
    pub scale: f64,
    pub rotation: f64,
    pub width_factor: f64,
    pub oblique: f64,
    pub thickness: f64,
    pub shape_no: u16,
    pub extrusion: (f64, f64, f64),
    pub shapefile_handle: Option<u64>,
}

pub fn decode_shape(reader: &mut BitReader<'_>) -> Result<ShapeEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_shape_with_header(reader, header, false, false)
}

pub fn decode_shape_r2007(reader: &mut BitReader<'_>) -> Result<ShapeEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_shape_with_header(reader, header, true, true)
}

pub fn decode_shape_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ShapeEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_shape_with_header(reader, header, true, true)
}

pub fn decode_shape_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ShapeEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_shape_with_header(reader, header, true, true)
}

fn decode_shape_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<ShapeEntity> {
    let insertion = reader.read_3bd()?;
    let scale = reader.read_bd()?;
    let rotation = reader.read_bd()?;
    let width_factor = reader.read_bd()?;
    let oblique = reader.read_bd()?;
    let thickness = reader.read_bd()?;
    let shape_no = reader.read_bs()?;
    let extrusion = reader.read_3bd()?;

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

    let shapefile_handle = read_handle_reference(reader, header.handle).ok();

    Ok(ShapeEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        insertion,
        scale,
        rotation,
        width_factor,
        oblique,
        thickness,
        shape_no,
        extrusion,
        shapefile_handle,
    })
}
