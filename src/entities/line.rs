use crate::bit::{BitReader, Endian};
use crate::core::result::Result;
use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

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
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(LineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        start: (x_start, y_start, z_start),
        end: (x_end, y_end, z_end),
    })
}
