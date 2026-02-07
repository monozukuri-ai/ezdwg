use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

#[derive(Debug, Clone)]
pub struct CircleEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub center: (f64, f64, f64),
    pub radius: f64,
}

pub fn decode_circle(reader: &mut BitReader<'_>) -> Result<CircleEntity> {
    let header = parse_common_entity_header(reader)?;

    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(CircleEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        center,
        radius,
    })
}
