use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct PointEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub location: (f64, f64, f64),
    pub x_axis_angle: f64,
}

pub fn encode_point_entity_payload(input: PointEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x1B, common, |writer| write_point_geometry(writer, input))
}

fn write_point_geometry(writer: &mut BitWriter, input: PointEncodeInput) -> Result<()> {
    writer.write_3bd(input.location.0, input.location.1, input.location.2)?;
    writer.write_bt(input.common.formatting.thickness)?; // thickness
    writer.write_be(
        input.common.formatting.extrusion.0,
        input.common.formatting.extrusion.1,
        input.common.formatting.extrusion.2,
    )?; // extrusion
    writer.write_bd(input.x_axis_angle)?;
    Ok(())
}
