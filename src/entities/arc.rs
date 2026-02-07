use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

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

    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let angle_start = reader.read_bd()?;
    let angle_end = reader.read_bd()?;
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(ArcEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        center,
        radius,
        angle_start,
        angle_end,
    })
}
