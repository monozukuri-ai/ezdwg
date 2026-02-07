use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct ArcEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub center: (f64, f64, f64),
    pub radius: f64,
    pub angle_start: f64,
    pub angle_end: f64,
}

pub fn decode_arc(reader: &mut BitReader<'_>) -> Result<ArcEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_arc_with_header(reader, header, false, false)
}

pub fn decode_arc_r2007(reader: &mut BitReader<'_>) -> Result<ArcEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_arc_with_header(reader, header, true, true)
}

pub fn decode_arc_r2010(reader: &mut BitReader<'_>, object_data_end_bit: u32) -> Result<ArcEntity> {
    let header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    decode_arc_with_header(reader, header, true, true)
}

pub fn decode_arc_r2013(reader: &mut BitReader<'_>, object_data_end_bit: u32) -> Result<ArcEntity> {
    let header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    decode_arc_with_header(reader, header, true, true)
}

fn decode_arc_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<ArcEntity> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let angle_start = reader.read_bd()?;
    let angle_end = reader.read_bd()?;
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

    Ok(ArcEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        radius,
        angle_start,
        angle_end,
    })
}
