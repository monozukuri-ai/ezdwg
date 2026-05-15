use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct PointEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub location: (f64, f64, f64),
    pub x_axis_angle: f64,
}

pub fn encode_point_entity_payload(input: PointEncodeInput) -> Result<Vec<u8>> {
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x1B, common, |writer| write_point_geometry(writer, input))
}

fn write_point_geometry(writer: &mut BitWriter, input: PointEncodeInput) -> Result<()> {
    writer.write_3bd(input.location.0, input.location.1, input.location.2)?;
    writer.write_bt(0.0)?; // thickness
    writer.write_be(0.0, 0.0, 1.0)?; // extrusion
    writer.write_bd(input.x_axis_angle)?;
    Ok(())
}
