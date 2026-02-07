use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct LineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub start: (f64, f64, f64),
    pub end: (f64, f64, f64),
}

pub fn decode_line(reader: &mut BitReader<'_>) -> Result<LineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_line_with_header(reader, header, false, false)
}

pub fn decode_line_r2007(reader: &mut BitReader<'_>) -> Result<LineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_line_with_header(reader, header, true, true)
}

pub fn decode_line_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_line_with_header(reader, header, true, true)
}

pub fn decode_line_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_line_with_header(reader, header, true, true)
}

fn decode_line_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<LineEntity> {
    let z_is_zero = reader.read_b()?;
    let x_start = reader.read_rd(Endian::Little)?;
    let x_end = reader.read_dd(x_start)?;
    let y_start = reader.read_rd(Endian::Little)?;
    let y_end = reader.read_dd(y_start)?;

    let (z_start, z_end) = if z_is_zero == 0 {
        let z_start = reader.read_rd(Endian::Little)?;
        let z_end = reader.read_dd(z_start)?;
        (z_start, z_end)
    } else {
        (0.0, 0.0)
    };

    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
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

    Ok(LineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        start: (x_start, y_start, z_start),
        end: (x_end, y_end, z_end),
    })
}
